use super::sse;
use super::types::{
    estimate_usage, AgentEvent, ContentBlock, ModelInfo, ProviderInfo, ProviderSource,
    ThinkingPref, ToolDefinition, Usage,
};
use tauri::Emitter;

#[derive(Clone)]
pub struct OpenAICompatProvider {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub source: ProviderSource,
    pub models: Vec<ModelInfo>,
    /// Provider-level reasoning effort, applied to every model unless a model
    /// declares its own `reasoningEffort`. None = never send the field.
    pub reasoning_effort: Option<String>,
}

impl OpenAICompatProvider {
    pub fn info(&self) -> ProviderInfo {
        ProviderInfo {
            id: self.id.clone(),
            name: self.name.clone(),
            source: self.source.clone(),
            models: self.models.clone(),
        }
    }

    pub async fn call(
        &self,
        messages: &[serde_json::Value],
        model: &str,
        api_key: &str,
        tools: &[ToolDefinition],
        thinking: ThinkingPref,
        app_handle: &tauri::AppHandle,
        session_id: &str,
    ) -> Result<(Vec<ContentBlock>, Usage), String> {
        let event_name = format!("agent:{session_id}");
        let client = reqwest::Client::new();

        // Convert history to OpenAI format (handles Anthropic-style tool_use/tool_result blocks)
        let oai_messages = to_openai_messages(messages);

        // Per-model settings (provider.json): `maxTokens` ceiling and optional
        // `reasoningEffort` (e.g. "low"/"medium"/"high" for gpt-5, Qwen3,
        // GLM-z1 and other reasoning-capable OpenAI-compat endpoints). The
        // provider-level reasoning_effort is the fallback when the model has
        // none. deepseek-reasoner needs no field (it reasons by default);
        // opaque upstream servers that reject unknown params stay safe because
        // the field is only sent when explicitly configured.
        let (max_tokens, reasoning_effort) =
            model_request_options(&self.models, model, self.reasoning_effort.as_deref());
        // The user's thinking pref decides how much reasoning to request:
        // off = never send reasoning_effort (cost control); auto = the
        // configured effort (provider.json / model entry); low/medium/high =
        // an explicit override of whatever was configured.
        let reasoning_effort = match thinking {
            ThinkingPref::Off => None,
            ThinkingPref::Auto => reasoning_effort,
            ThinkingPref::Low | ThinkingPref::Medium | ThinkingPref::High => {
                thinking.effort().map(|e| e.to_string())
            }
        };

        let mut body = serde_json::json!({
            "model": model,
            "stream": true,
            "temperature": 0.0,
            "max_tokens": max_tokens.unwrap_or(16_384),
            "messages": oai_messages,
        });
        if let Some(effort) = reasoning_effort {
            body["reasoning_effort"] = serde_json::Value::String(effort.to_string());
        }

        if !tools.is_empty() {
            body["tools"] = serde_json::to_value(
                tools
                    .iter()
                    .map(|t| {
                        serde_json::json!({
                            "type": "function",
                            "function": {
                                "name": t.name,
                                "description": t.description,
                                "parameters": t.input_schema,
                            }
                        })
                    })
                    .collect::<Vec<_>>(),
            )
            .unwrap();
        }

        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));
        let mut req = client
            .post(&url)
            .header("content-type", "application/json")
            .header("accept-encoding", "identity")
            .json(&body);
        if !api_key.is_empty() {
            req = req.bearer_auth(api_key);
        }

        let resp = req
            .timeout(std::time::Duration::from_secs(180))
            .send()
            .await
            .map_err(|e| {
                let msg = format!("Failed to reach {} at {}: {e}", self.id, self.base_url);
                app_handle
                    .emit(
                        &event_name,
                        AgentEvent::Error {
                            message: msg.clone(),
                        },
                    )
                    .ok();
                msg
            })?;
        if !resp.status().is_success() {
            let status = resp.status();
            let msg = match resp.text().await {
                Ok(t) if !t.trim().is_empty() => t,
                _ => format!(
                    "{} returned HTTP {status} with no error details — the service may be down, slow, or over quota.",
                    self.id
                ),
            };
            app_handle
                .emit(
                    &event_name,
                    AgentEvent::Error {
                        message: msg.clone(),
                    },
                )
                .ok();
            return Err(msg);
        }

        let mut full_text = String::new();
        // tool_calls by index: (id, name, accumulated_arguments)
        let mut tool_calls: std::collections::BTreeMap<usize, (String, String, String)> =
            std::collections::BTreeMap::new();

        // Set when the stream ended cleanly with [DONE]; false means we hit an
        // EOF/error mid-stream, in which case the answer may be incomplete.
        let mut saw_done = false;
        let mut usage = Usage::default();
        let mut reasoning_buf = String::new();
        let mut reasoning_end_emitted = false;
        let mut stream = sse::SseStream::new(resp);
        while let Some(data) = stream.next_data().await {
            if data.trim() == "[DONE]" {
                saw_done = true;
                break;
            }
            let Ok(ev) = serde_json::from_str::<serde_json::Value>(&data) else {
                continue;
            };
            // Usage arrives on the final chunk (OpenAI stream_options.include_usage;
            // DeepSeek sends it too). Capture whenever present.
            if let Some(u) = ev["usage"].as_object() {
                if let Some(n) = u["prompt_tokens"].as_u64() {
                    usage.input_tokens = n;
                }
                if let Some(n) = u["completion_tokens"].as_u64() {
                    usage.output_tokens = n;
                }
                if usage.input_tokens == 0 {
                    if let Some(n) = u["input_tokens"].as_u64() {
                        usage.input_tokens = n;
                    }
                }
                if usage.output_tokens == 0 {
                    if let Some(n) = u["output_tokens"].as_u64() {
                        usage.output_tokens = n;
                    }
                }
            }
            let delta = &ev["choices"][0]["delta"];

            // Reasoning tokens (DeepSeek and compatible models)
            if let Some(r) = delta["reasoning_content"].as_str() {
                if !r.is_empty() {
                    reasoning_buf.push_str(r);
                    app_handle
                        .emit(
                            &event_name,
                            AgentEvent::Reasoning {
                                content: r.to_string(),
                            },
                        )
                        .ok();
                }
            }

            // Text tokens
            if let Some(t) = delta["content"].as_str() {
                if !t.is_empty() {
                    // DeepSeek streams reasoning first, then content — the
                    // reasoning block is done the moment content begins.
                    if !reasoning_buf.is_empty() && !reasoning_end_emitted {
                        reasoning_end_emitted = true;
                        app_handle.emit(&event_name, AgentEvent::ThinkingEnd).ok();
                    }
                    full_text.push_str(t);
                    app_handle
                        .emit(
                            &event_name,
                            AgentEvent::Token {
                                content: t.to_string(),
                            },
                        )
                        .ok();
                }
            }

            // Tool call deltas
            if let Some(tcs) = delta["tool_calls"].as_array() {
                for tc in tcs {
                    let idx = tc["index"].as_u64().unwrap_or(0) as usize;
                    let entry = tool_calls
                        .entry(idx)
                        .or_insert_with(|| (String::new(), String::new(), String::new()));
                    if let Some(id) = tc["id"].as_str() {
                        entry.0 = id.to_string();
                    }
                    if let Some(name) = tc["function"]["name"].as_str() {
                        entry.1 = name.to_string();
                    }
                    if let Some(args) = tc["function"]["arguments"].as_str() {
                        entry.2.push_str(args);
                    }
                }
            }
        }

        // Reasoning-only turn (no text followed it): seal the thought block.
        if !reasoning_buf.is_empty() && !reasoning_end_emitted {
            app_handle.emit(&event_name, AgentEvent::ThinkingEnd).ok();
        }

        let mut blocks: Vec<ContentBlock> = Vec::new();
        if !reasoning_buf.is_empty() {
            blocks.push(ContentBlock::Reasoning {
                content: reasoning_buf,
            });
        }
        if !full_text.trim().is_empty() {
            blocks.push(ContentBlock::Text { text: full_text });
        }
        for (_, (id, name, args)) in tool_calls {
            let input = serde_json::from_str(&args)
                .unwrap_or(serde_json::Value::Object(Default::default()));
            blocks.push(ContentBlock::ToolUse { id, name, input });
        }

        let cut_off = stream.broken() || !saw_done;
        if blocks.is_empty() {
            // An empty body that wasn't cut off is still a dead turn — never
            // end the loop with zero content (that looks like the model dying).
            return Err(if cut_off {
                "Connection interrupted before any content arrived — the model service may be down."
                    .into()
            } else {
                "The model returned an empty response — the service may be overloaded; try again."
                    .into()
            });
        }
        if cut_off
            && !blocks
                .iter()
                .any(|b| matches!(b, ContentBlock::ToolUse { .. }))
        {
            // Partial answer arrived but the stream broke: surface it. (When
            // tool calls are pending we stay quiet so approvals keep flowing.)
            app_handle
                .emit(
                    &event_name,
                    AgentEvent::Error {
                        message: "Connection interrupted — the response may be incomplete.".into(),
                    },
                )
                .ok();
        }

        // Fallback estimate when usage events were absent.
        if usage.total() == 0 {
            usage.input_tokens =
                estimate_usage(&serde_json::to_string(&oai_messages).unwrap_or_default());
            usage.output_tokens =
                estimate_usage(&serde_json::to_string(&blocks).unwrap_or_default());
        }

        Ok((blocks, usage))
    }
}

/// Resolve per-model request options for an OpenAI-compat call:
/// - `max_tokens`: the model's own `maxTokens` when configured, else None
///   (caller falls back to the provider default).
/// - `reasoning_effort`: the model's own `reasoningEffort`, else the
///   provider-level fallback, else None (field not sent).
fn model_request_options(
    models: &[ModelInfo],
    model_id: &str,
    provider_effort: Option<&str>,
) -> (Option<u32>, Option<String>) {
    let cfg = models.iter().find(|m| m.id == model_id);
    let max_tokens = cfg.and_then(|m| m.max_tokens);
    let effort = cfg
        .and_then(|m| m.reasoning_effort.as_deref())
        .or(provider_effort)
        .map(|s| s.to_string());
    (max_tokens, effort)
}

/// Convert a history array (which may contain Anthropic-style content blocks) into
/// standard OpenAI message format.
fn to_openai_messages(messages: &[serde_json::Value]) -> Vec<serde_json::Value> {
    let mut out = Vec::new();
    for msg in messages {
        let role = msg["role"].as_str().unwrap_or("");
        let content = &msg["content"];

        match role {
            "assistant" => {
                if let Some(blocks) = content.as_array() {
                    // Content-block array — split into reasoning + text + tool_calls
                    let reasoning: String = blocks
                        .iter()
                        .filter(|b| b["type"] == "reasoning")
                        .filter_map(|b| b["content"].as_str())
                        .collect::<Vec<_>>()
                        .join("");
                    let text: String = blocks
                        .iter()
                        .filter(|b| b["type"] == "text")
                        .filter_map(|b| b["text"].as_str())
                        .collect::<Vec<_>>()
                        .join("");
                    let tcs: Vec<serde_json::Value> = blocks.iter()
                        .filter(|b| b["type"] == "tool_use")
                        .map(|b| serde_json::json!({
                            "id": b["id"],
                            "type": "function",
                            "function": {
                                "name": b["name"],
                                "arguments": serde_json::to_string(&b["input"]).unwrap_or_else(|_| "{}".into())
                            }
                        }))
                        .collect();
                    let mut m = if tcs.is_empty() {
                        serde_json::json!({ "role": "assistant", "content": text })
                    } else {
                        serde_json::json!({
                            "role": "assistant",
                            "content": if text.is_empty() { serde_json::Value::Null } else { serde_json::Value::String(text) },
                            "tool_calls": tcs,
                        })
                    };
                    if !reasoning.is_empty() {
                        m["reasoning_content"] = serde_json::Value::String(reasoning);
                    }
                    out.push(m);
                } else {
                    out.push(msg.clone());
                }
            }
            "user" => {
                // Check for Anthropic tool_result blocks — convert each to role:tool
                if let Some(blocks) = content.as_array() {
                    if blocks.iter().any(|b| b["type"] == "tool_result") {
                        for block in blocks {
                            if block["type"] == "tool_result" {
                                out.push(serde_json::json!({
                                    "role": "tool",
                                    "tool_call_id": block["tool_use_id"],
                                    "content": block["content"].as_str().unwrap_or(""),
                                }));
                            }
                        }
                        continue;
                    }
                }
                out.push(msg.clone());
            }
            _ => out.push(msg.clone()),
        }
    }
    // Sanitize: remove orphaned tool_calls sequences (e.g., session crashed
    // mid-loop). If an assistant message has N tool_calls but fewer than N
    // role:tool messages follow it, the exchange is corrupted. Drop only the
    // corrupt span and everything up to the latest real user message — but
    // ALWAYS keep the newest user prompt (and the system message) so the model
    // still sees what the user actually asked instead of re-running old tool
    // calls.
    let mut i = 0;
    while i < out.len() {
        if out[i]["role"].as_str() == Some("assistant") {
            if let Some(tcs) = out[i]["tool_calls"].as_array() {
                let n_calls = tcs.len();
                if n_calls > 0 {
                    let mut j = i + 1;
                    while j < out.len() && out[j]["role"].as_str() == Some("tool") {
                        j += 1;
                    }
                    if (j - i - 1) < n_calls {
                        // Find the newest real user message (role:user — tool
                        // results are already role:tool here).
                        let mut cutoff = i;
                        for k in (i..out.len()).rev() {
                            if out[k]["role"].as_str() == Some("user") {
                                cutoff = k;
                                break;
                            }
                        }
                        if cutoff <= i {
                            // No user prompt after the dangling call: drop the
                            // dangling assistant message itself.
                            out.remove(i);
                        } else {
                            // Keep everything before the corrupted exchange,
                            // skip the dangling assistant+tool span, then
                            // append from the latest user message onward.
                            let mut kept = out[..i].to_vec();
                            kept.extend_from_slice(&out[cutoff..]);
                            out = kept;
                        }
                        break;
                    }
                    i = j;
                    continue;
                }
            }
        }
        i += 1;
    }

    out
}

pub fn deepseek() -> OpenAICompatProvider {
    OpenAICompatProvider {
        id: "deepseek".into(),
        name: "DeepSeek".into(),
        base_url: "https://api.deepseek.com/v1".into(),
        source: ProviderSource::Builtin,
        models: vec![
            super::catalog::enrich(
                "deepseek-chat",
                "DeepSeek Chat".into(),
                None,
                None,
                None,
                None,
                None,
            ),
            super::catalog::enrich(
                "deepseek-reasoner",
                "DeepSeek Reasoner".into(),
                None,
                None,
                None,
                None,
                None,
            ),
        ],
        reasoning_effort: None,
    }
}

pub fn opencode_local() -> OpenAICompatProvider {
    OpenAICompatProvider {
        id: "opencode".into(),
        name: "OpenCode".into(),
        base_url: "http://localhost:4000/v1".into(),
        source: ProviderSource::Builtin,
        models: vec![],
        reasoning_effort: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizer_keeps_newest_user_prompt_with_dangling_tool_call() {
        // Exactly the corruption seen in production: assistant tool_call with
        // no result, then the user's brand-new prompt.
        let messages = vec![
            serde_json::json!({ "role": "system", "content": "sys" }),
            serde_json::json!({ "role": "user", "content": "old task" }),
            assistant_with_blocks(vec![
                serde_json::json!({ "type": "tool_use", "id": "c1", "name": "x", "input": {} }),
            ]),
            serde_json::json!({ "role": "user", "content": "unrelated follow-up" }),
        ];
        let out = to_openai_messages(&messages);
        let texts: Vec<&str> = out
            .iter()
            .filter(|m| m["role"].as_str() == Some("user"))
            .filter_map(|m| m["content"].as_str())
            .collect();
        assert!(
            texts.contains(&"unrelated follow-up"),
            "newest prompt must survive: {out:?}"
        );
    }

    fn assistant_with_blocks(blocks: Vec<serde_json::Value>) -> serde_json::Value {
        serde_json::json!({ "role": "assistant", "content": blocks })
    }

    #[test]
    fn sanitizer_keeps_well_formed_exchange() {
        let messages = vec![
            serde_json::json!({ "role": "user", "content": "hi" }),
            assistant_with_blocks(vec![
                serde_json::json!({ "type": "tool_use", "id": "c1", "name": "read_file", "input": {"path": "a"} }),
            ]),
            serde_json::json!({ "role": "user", "content": [
                serde_json::json!({ "type": "tool_result", "tool_use_id": "c1", "content": "ok", "is_error": false })
            ]}),
        ];
        let out = to_openai_messages(&messages);
        assert_eq!(out.len(), 3);
        assert_eq!(out[1]["tool_calls"].as_array().unwrap().len(), 1);
        assert_eq!(out[2]["role"], "tool");
    }

    #[test]
    fn request_options_resolution() {
        let models = vec![
            crate::providers::catalog::enrich(
                "plain",
                "Plain".into(),
                None,
                None,
                None,
                None,
                None,
            ),
            crate::providers::types::ModelInfo {
                id: "reasoner".into(),
                name: "Reasoner".into(),
                context_window: None,
                is_free: false,
                is_vision: false,
                max_tokens: Some(32_000),
                reasoning_effort: Some("high".into()),
                supports_thinking: true,
                supports_thinking_levels: true,
                thinking_budget: None,
            },
        ];
        // Model-level settings win.
        let (mt, eff) = model_request_options(&models, "reasoner", Some("medium"));
        assert_eq!(mt, Some(32_000));
        assert_eq!(eff.as_deref(), Some("high"));
        // Provider-level fallback when the model has none.
        let (mt, eff) = model_request_options(&models, "plain", Some("medium"));
        assert_eq!(mt, None);
        assert_eq!(eff.as_deref(), Some("medium"));
        // Nothing configured → no reasoning_effort field.
        let (mt, eff) = model_request_options(&models, "plain", None);
        assert_eq!(mt, None);
        assert_eq!(eff, None);
        // Unknown model → provider fallback still applies.
        let (mt, eff) = model_request_options(&models, "ghost", Some("low"));
        assert_eq!(mt, None);
        assert_eq!(eff.as_deref(), Some("low"));
    }
}
