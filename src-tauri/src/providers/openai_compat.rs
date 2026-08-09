use super::types::{
    AgentEvent, ContentBlock, ModelInfo, ProviderInfo, ProviderSource, ToolDefinition,
};
use futures_util::StreamExt;
use tauri::Emitter;

#[derive(Clone)]
pub struct OpenAICompatProvider {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub source: ProviderSource,
    pub models: Vec<ModelInfo>,
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
}

impl OpenAICompatProvider {
    pub async fn call(
        &self,
        messages: &[serde_json::Value],
        model: &str,
        api_key: &str,
        tools: &[ToolDefinition],
        app_handle: &tauri::AppHandle,
        session_id: &str,
    ) -> Result<Vec<ContentBlock>, String> {
        let event_name = format!("agent:{session_id}");
        let client = reqwest::Client::new();

        // Convert history to OpenAI format (handles Anthropic-style tool_use/tool_result blocks)
        let oai_messages = to_openai_messages(messages);

        let mut body = serde_json::json!({
            "model": model,
            "stream": true,
            "messages": oai_messages,
        });

        if !tools.is_empty() {
            body["tools"] = serde_json::to_value(
                tools.iter().map(|t| serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.input_schema,
                    }
                })).collect::<Vec<_>>(),
            ).unwrap();
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
                let msg = format!("Failed to reach {}", self.id);
                let msg = format!("{msg} at {}: {e}", self.base_url);
                app_handle.emit(&event_name, AgentEvent::Error { message: msg.clone() }).ok();
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
            app_handle.emit(&event_name, AgentEvent::Error { message: msg.clone() }).ok();
            return Err(msg);
        }

        let mut full_text = String::new();
        let mut reasoning_buf = String::new();
        // tool_calls by index: (id, name, accumulated_arguments)
        let mut tool_calls: std::collections::BTreeMap<usize, (String, String, String)> =
            std::collections::BTreeMap::new();

        let mut stream = resp.bytes_stream();
        // Raw byte buffer — avoids corrupting multi-byte UTF-8 chars split across HTTP chunks.
        let mut buf: Vec<u8> = Vec::new();
        // Whether the stream ended cleanly with [DONE]; false means we hit an
        // EOF/error mid-stream, in which case the answer may be incomplete.
        let mut saw_done = false;
        let mut stream_broken = false;
        'outer: loop {
            // Idle protection: if the relay stops sending bytes for 60s the
            // connection is dead — surface it instead of hanging forever.
            let chunk = tokio::time::timeout(std::time::Duration::from_secs(60), stream.next()).await;
            let bytes = match chunk {
                Err(_) => {
                    stream_broken = true; // no data for 60s
                    break 'outer;
                }
                Ok(None) => break 'outer, // clean EOF — [DONE] handling below
                Ok(Some(Err(_))) => {
                    stream_broken = true;
                    break 'outer;
                }
                Ok(Some(Ok(b))) => b,
            };
            buf.extend_from_slice(&bytes);
            loop {
                let Some(nl) = buf.iter().position(|&b| b == b'\n') else { break };
                let line_bytes = &buf[..nl];
                let line_bytes = line_bytes.strip_suffix(b"\r").unwrap_or(line_bytes);
                let line = String::from_utf8_lossy(line_bytes).into_owned();
                buf.drain(..=nl);
                let Some(data) = line.strip_prefix("data: ") else {
                    continue;
                };
                if data.trim() == "[DONE]" {
                    saw_done = true;
                    break 'outer;
                }
                let Ok(ev) = serde_json::from_str::<serde_json::Value>(data) else {
                    continue;
                };
                let delta = &ev["choices"][0]["delta"];

                // Reasoning tokens (DeepSeek and compatible models)
                if let Some(r) = delta["reasoning_content"].as_str() {
                    if !r.is_empty() {
                        reasoning_buf.push_str(r);
                        app_handle
                            .emit(&event_name, AgentEvent::Reasoning { content: r.to_string() })
                            .ok();
                    }
                }

                // Text tokens
                if let Some(t) = delta["content"].as_str() {
                    if !t.is_empty() {
                        full_text.push_str(t);
                        app_handle.emit(&event_name, AgentEvent::Token { content: t.to_string() }).ok();
                    }
                }

                // Tool call deltas
                if let Some(tcs) = delta["tool_calls"].as_array() {
                    for tc in tcs {
                        let idx = tc["index"].as_u64().unwrap_or(0) as usize;
                        let entry = tool_calls.entry(idx).or_insert_with(|| {
                            (String::new(), String::new(), String::new())
                        });
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
        }

        let mut blocks: Vec<ContentBlock> = Vec::new();
        if !reasoning_buf.is_empty() {
            blocks.push(ContentBlock::Reasoning { content: reasoning_buf });
        }
        if !full_text.is_empty() {
            blocks.push(ContentBlock::Text { text: full_text });
        }
        for (_, (id, name, args)) in tool_calls {
            let input = serde_json::from_str(&args).unwrap_or(serde_json::Value::Object(Default::default()));
            blocks.push(ContentBlock::ToolUse { id, name, input });
        }

        let cut_off = stream_broken || !saw_done;
        if blocks.is_empty() {
            // An empty body that wasn't cut off is still a dead turn — never
            // end the loop with zero content (that looks like the model dying).
            return Err(if cut_off {
                "Connection interrupted before any content arrived — the model service may be down.".into()
            } else {
                "The model returned an empty response — the service may be overloaded; try again.".into()
            });
        }
        if cut_off && !blocks.iter().any(|b| matches!(b, ContentBlock::ToolUse { .. })) {
            // Partial answer arrived but the stream broke: surface it. (When
            // tool calls are pending we stay quiet so approvals keep flowing.)
            app_handle
                .emit(&event_name, AgentEvent::Error {
                    message: "Connection interrupted — the response may be incomplete.".into(),
                })
                .ok();
        }

        Ok(blocks)
    }
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
                    let reasoning: String = blocks.iter()
                        .filter(|b| b["type"] == "reasoning")
                        .filter_map(|b| b["content"].as_str())
                        .collect::<Vec<_>>()
                        .join("");
                    let text: String = blocks.iter()
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
                            let mut kept = out[..i]
                                .iter()
                                .filter(|m| m["role"].as_str() == Some("system"))
                                .cloned()
                                .collect::<Vec<_>>();
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
            super::catalog::enrich("deepseek-chat", "DeepSeek Chat".into(), None, None, None),
            super::catalog::enrich("deepseek-reasoner", "DeepSeek Reasoner".into(), None, None, None),
        ],
    }
}

pub fn opencode_local() -> OpenAICompatProvider {
    OpenAICompatProvider {
        id: "opencode".into(),
        name: "OpenCode".into(),
        base_url: "http://localhost:4000/v1".into(),
        source: ProviderSource::Builtin,
        models: vec![],
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
        assert!(texts.contains(&"unrelated follow-up"), "newest prompt must survive: {out:?}");
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
}
