use super::catalog;
use super::run_detached;
use super::sse;
use super::types::{
    AgentEvent, ContentBlock, ModelInfo, ProviderInfo, ProviderSource, ToolDefinition, Usage,
    estimate_usage,
};
use tauri::Emitter;
use tauri::Manager;

#[derive(Clone)]
pub struct AnthropicProvider {
    pub models: Vec<ModelInfo>,
}

/// Budget for extended thinking, in tokens — how much the model may deliberate
/// per turn before producing output. Only sent for models that support it.
const THINKING_BUDGET_TOKENS: u32 = 8_192;
/// max_tokens when thinking is enabled: the API requires max_tokens to exceed
/// the thinking budget so there is room left for text/tool calls after thinking.
const THINKING_MAX_TOKENS: u32 = 25_600;

/// The thinking budget to use for a model, driven by its declared capability
/// (provider model list → /v1/models metadata → family catalog fallback)
/// rather than hardcoded id matching. Returns the budget when thinking should
/// be enabled for this call, None otherwise.
fn thinking_cfg(models: &[ModelInfo], model: &str) -> Option<u32> {
    let info = models.iter().find(|m| m.id == model);
    match info {
        Some(m) if m.supports_thinking => Some(m.thinking_budget.unwrap_or(THINKING_BUDGET_TOKENS)),
        Some(_) => None,
        // Model missing from the provider's list (e.g. selected before
        // discovery landed) — fall back to the capability catalog.
        None => super::catalog::thinking_for(model).then_some(THINKING_BUDGET_TOKENS),
    }
}

/// Curated fallback list — real Anthropic API model ids. Shown until (or when)
/// the live /v1/models fetch succeeds (which needs an API key).
pub fn fallback_models() -> Vec<ModelInfo> {
    [
        "claude-sonnet-4-5-20250929",
        "claude-opus-4-1-20250805",
        "claude-haiku-4-5-20251001",
        "claude-3-7-sonnet-20250219",
        "claude-3-5-haiku-20241022",
    ]
    .into_iter()
    .map(|id| {
        let family = id
            .split('-') // claude-sonnet-4-5-20250929 → [claude, sonnet, 4, 5, 20250929]
            .nth(1)
            .map(|w| {
                let mut c = w.chars();
                match c.next() {
                    Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                    None => w.to_string(),
                }
            })
            .unwrap_or_else(|| id.to_string());
        catalog::enrich(id, format!("Claude {family}"), None, None, None, None)
    })
    .collect()
}

impl Default for AnthropicProvider {
    fn default() -> Self {
        Self {
            models: fallback_models(),
        }
    }
}

impl AnthropicProvider {
    pub fn info(&self) -> ProviderInfo {
        ProviderInfo {
            id: "anthropic".into(),
            name: "Anthropic".into(),
            source: ProviderSource::Builtin,
            models: self.models.clone(),
        }
    }

    pub async fn call(
        &self,
        messages: &[serde_json::Value],
        model: &str,
        api_key: &str,
        tools: &[ToolDefinition],
        thinking: bool,
        app_handle: &tauri::AppHandle,
        session_id: &str,
    ) -> Result<(Vec<ContentBlock>, Usage), String> {
        let event_name = format!("agent:{session_id}");
        let client = reqwest::Client::new();

        // Anthropic requires system content in a top-level "system" field,
        // not as a {"role":"system"} entry inside messages.
        let system_text: String = messages
            .iter()
            .filter(|m| m["role"].as_str() == Some("system"))
            .filter_map(|m| m["content"].as_str())
            .collect::<Vec<_>>()
            .join("\n\n");
        // Extended thinking is driven by the model's declared capability (model
        // list → discovery metadata → family catalog), NOT hardcoded ids, and
        // can be force-disabled by the user (thinking==false) to save money.
        // When enabled, thinking blocks (with their signatures) must be passed
        // back verbatim in history — the API requires it. When disabled, any
        // thinking blocks in history are stripped so the API doesn't reject
        // the request (it rejects them unless thinking is enabled).
        let thinking_budget = if thinking {
            thinking_cfg(&self.models, model)
        } else {
            None
        };
        let strip_thinking = thinking_budget.is_none();
        let stripped_messages: Vec<serde_json::Value> = messages
            .iter()
            .filter(|m| m["role"].as_str() != Some("system"))
            .map(|m| {
                if strip_thinking {
                    if m["role"].as_str() == Some("assistant") {
                        if let Some(blocks) = m["content"].as_array() {
                            let clean: Vec<_> = blocks
                                .iter()
                                .filter(|b| b["type"].as_str() != Some("thinking"))
                                .cloned()
                                .collect();
                            return serde_json::json!({ "role": "assistant", "content": clean });
                        }
                    }
                }
                m.clone()
            })
            .collect();
        let chat_messages: Vec<&serde_json::Value> = stripped_messages.iter().collect();

        // Extended thinking requires temperature == 1 and max_tokens > budget.
        let mut body = serde_json::json!({
            "model": model,
            "max_tokens": thinking_budget.map_or(16384, |_| THINKING_MAX_TOKENS),
            "temperature": thinking_budget.map_or(0.0, |_| 1.0),
            "stream": true,
            "messages": chat_messages,
        });
        if let Some(budget) = thinking_budget {
            body["thinking"] = serde_json::json!({
                "type": "enabled",
                "budget_tokens": budget,
            });
        }
        if !system_text.is_empty() {
            body["system"] = serde_json::Value::String(system_text.clone());
        }
        if !tools.is_empty() {
            body["tools"] = serde_json::to_value(
                tools
                    .iter()
                    .map(|t| {
                        serde_json::json!({
                            "name": t.name,
                            "description": t.description,
                            "input_schema": t.input_schema,
                        })
                    })
                    .collect::<Vec<_>>(),
            )
            .unwrap();
        }

        // Prompt caching: the system prompt + tool definitions are byte-identical
        // across every call of a conversation, so mark the end of that prefix with
        // an ephemeral cache breakpoint — the API re-sends only the messages after
        // it each turn, saving ~2k+ tokens of input per call. The breakpoint is
        // only added when the cached prefix is long enough: Anthropic rejects
        // cache_control on a prefix under its minimum cacheable length (~1024
        // tokens; estimate_usage ≈ chars/4).
        let tools_value = body.get("tools").cloned();
        let cacheable_tokens = estimate_usage(&system_text)
            + tools_value
                .as_ref()
                .map_or(0, |t| estimate_usage(&t.to_string()));
        if cacheable_tokens >= 1024 {
            if let Some(arr) = body["tools"].as_array_mut() {
                // Cache breakpoint at the END of the tools prefix (single
                // breakpoint covering system + tools).
                if let Some(last) = arr.last_mut() {
                    last["cache_control"] = serde_json::json!({ "type": "ephemeral" });
                }
            } else if !system_text.is_empty() {
                // No tools (rare) — cache the system prompt alone.
                body["system"] = serde_json::json!([{
                    "type": "text",
                    "text": system_text,
                    "cache_control": { "type": "ephemeral" },
                }]);
            }
        }

        let resp = client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .header("accept-encoding", "identity")
            .json(&body)
            .timeout(std::time::Duration::from_secs(180))
            .send()
            .await
            .map_err(|e| {
                let msg = format!("Failed to reach Anthropic API: {e}");
                app_handle.emit(&event_name, AgentEvent::Error { message: msg.clone() }).ok();
                msg
            })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let msg = match resp.text().await {
                Ok(t) if !t.trim().is_empty() => t,
                _ => format!(
                    "Anthropic API returned HTTP {status} with no error details — the service may be down or the key invalid."
                ),
            };
            app_handle.emit(&event_name, AgentEvent::Error { message: msg.clone() }).ok();
            return Err(msg);
        }

        // Parse SSE stream, collect content blocks.
        let mut blocks: Vec<ContentBlock> = Vec::new();
        let mut current_text = String::new();
        let mut current_tool: Option<(String, String, String)> = None; // (id, name, input_json)
        let mut current_thinking: Option<(String, String)> = None; // (thinking, signature)

        // Whether the stream finished cleanly (message_stop/[DONE] seen); false
        // means an EOF/error mid-stream, in which case the answer may be incomplete.
        let mut saw_stop = false;
        let mut usage = Usage::default();
        let mut stream = sse::SseStream::new(resp);
        while let Some(data) = stream.next_data().await {
            if data == "[DONE]" {
                saw_stop = true;
                break;
            }
            let Ok(ev) = serde_json::from_str::<serde_json::Value>(&data) else {
                continue;
            };
            // Usage events: message_start carries input_tokens, message_delta carries
            // output_tokens (and cumulative input_tokens on final delta).
            match ev["type"].as_str().unwrap_or("") {
                "message_start" => {
                    if let Some(n) = ev["message"]["usage"]["input_tokens"].as_u64() {
                        usage.input_tokens = n;
                    }
                }
                "message_delta" => {
                    if let Some(n) = ev["usage"]["output_tokens"].as_u64() {
                        usage.output_tokens = n;
                    }
                    if let Some(n) = ev["usage"]["input_tokens"].as_u64() {
                        usage.input_tokens = n;
                    }
                }
                _ => {}
            }
            if ev["type"].as_str() == Some("message_stop") {
                saw_stop = true;
                break;
            }
            match ev["type"].as_str().unwrap_or("") {
                "content_block_start" => {
                    // Flush any accumulated text before starting a non-text block,
                    // so text written before a tool call isn't silently lost.
                    if !current_text.is_empty() {
                        blocks.push(ContentBlock::Text {
                            text: std::mem::take(&mut current_text),
                        });
                    }
                    let bt = ev["content_block"]["type"].as_str().unwrap_or("");
                    if bt == "tool_use" {
                        let id = ev["content_block"]["id"].as_str().unwrap_or("").to_string();
                        let name = ev["content_block"]["name"].as_str().unwrap_or("").to_string();
                        current_tool = Some((id, name, String::new()));
                    } else if bt == "thinking" {
                        current_thinking = Some((String::new(), String::new()));
                    }
                }
                "content_block_delta" => {
                    let dt = ev["delta"]["type"].as_str().unwrap_or("");
                    if dt == "text_delta" {
                        let t = ev["delta"]["text"].as_str().unwrap_or("");
                        current_text.push_str(t);
                        app_handle.emit(&event_name, AgentEvent::Token { content: t.to_string() }).ok();
                    } else if dt == "input_json_delta" {
                        if let Some((_, _, ref mut json_buf)) = current_tool {
                            json_buf.push_str(ev["delta"]["partial_json"].as_str().unwrap_or(""));
                        }
                    } else if dt == "thinking_delta" {
                        if let Some((ref mut thinking, _)) = current_thinking {
                            let t = ev["delta"]["thinking"].as_str().unwrap_or("");
                            thinking.push_str(t);
                            if !t.is_empty() {
                                app_handle
                                    .emit(&event_name, AgentEvent::Reasoning { content: t.to_string() })
                                    .ok();
                            }
                        }
                    } else if dt == "signature_delta" {
                        if let Some((_, ref mut signature)) = current_thinking {
                            signature.push_str(ev["delta"]["signature"].as_str().unwrap_or(""));
                        }
                    }
                }
                "content_block_stop" => {
                    if let Some((id, name, json_buf)) = current_tool.take() {
                        let input: serde_json::Value =
                            serde_json::from_str(&json_buf).unwrap_or(serde_json::Value::Null);
                        blocks.push(ContentBlock::ToolUse { id, name, input });
                    } else if let Some((thinking, signature)) = current_thinking.take() {
                        blocks.push(ContentBlock::Thinking { thinking, signature });
                        // Seal the live thought block so the frontend separates
                        // it from the next one (streamed UI == persisted UI).
                        app_handle
                            .emit(&event_name, AgentEvent::ThinkingEnd)
                            .ok();
                    } else if !current_text.is_empty() {
                        blocks.push(ContentBlock::Text {
                            text: std::mem::take(&mut current_text),
                        });
                    }
                }
                _ => {}
            }
        }

        // Flush any in-progress text block (stream may have ended without content_block_stop)
        if !current_text.is_empty() {
            blocks.push(ContentBlock::Text { text: current_text });
        }

        let cut_off = stream.broken() || !saw_stop;
        // Fallback estimate when usage events were absent (older API / proxy).
        if usage.total() == 0 {
            let input_est: String = [system_text.as_str(), &serde_json::to_string(messages).unwrap_or_default()].concat();
            usage.input_tokens = estimate_usage(&input_est);
            usage.output_tokens = estimate_usage(&serde_json::to_string(&blocks).unwrap_or_default());
        }
        if blocks.is_empty() {
            return Err(if cut_off {
                "Connection interrupted before any content arrived — the Anthropic service may be down.".into()
            } else {
                "The model returned an empty response — try again.".into()
            });
        }
        if cut_off && !blocks.iter().any(|b| matches!(b, ContentBlock::ToolUse { .. })) {
            app_handle
                .emit(&event_name, AgentEvent::Error {
                    message: "Connection interrupted — the response may be incomplete.".into(),
                })
                .ok();
        }

        Ok((blocks, usage))
    }
}

/// Fetch the real model list from https://api.anthropic.com/v1/models.
/// Returns None when there's no key or the request fails;
/// `Some(Vec::new())` when the response carried no models.
pub async fn fetch_models(api_key: Option<String>) -> Option<Vec<ModelInfo>> {
    let Some(key) = api_key else {
        return None;
    };
    if key.trim().is_empty() {
        return None;
    }
    let Ok(resp) = reqwest::Client::new()
        .get("https://api.anthropic.com/v1/models")
        .header("x-api-key", &key)
        .header("anthropic-version", "2023-06-01")
        .timeout(std::time::Duration::from_secs(6))
        .send()
        .await
    else {
        return None;
    };
    if !resp.status().is_success() {
        return None;
    }
    let Ok(body) = resp.json::<serde_json::Value>().await else {
        return None;
    };
    let Some(arr) = body["data"].as_array() else {
        return Some(Vec::new());
    };
    let models: Vec<ModelInfo> = arr
        .iter()
        .filter_map(|m| {
            let id = m["id"].as_str()?;
            let name = m["display_name"].as_str().unwrap_or(id).to_string();
            let ctx = m["context_window"].as_u64().map(|n| n as u32);
            Some(catalog::enrich(id, name, ctx, Some(true), Some(false), None))
        })
        .collect();
    Some(models)
}

/// Detached best-effort /v1/models fetch; on success replaces the fallback
/// list and tells the UI. No key means we keep the curated fallback.
pub fn spawn_discovery(app: tauri::AppHandle, api_key: Option<String>) {
    run_detached(move || async move {
        let Some(models) = fetch_models(api_key).await else {
            return;
        };
        if models.is_empty() {
            return;
        }
        if let Some(state) = app.try_state::<crate::AppState>() {
            state.registry.upsert_models("anthropic", models);
            let _ = app.emit("providers:changed", ("anthropic",));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thinking_only_for_capable_models() {
        // No model list → capability catalog fallback.
        for m in [
            "claude-sonnet-4-5-20250929",
            "claude-opus-4-1-20250805",
            "claude-sonnet-4-6-20251022",
            "claude-3-7-sonnet-20250219",
        ] {
            assert!(thinking_cfg(&[], m).is_some(), "{m} should support thinking");
        }
        // Base Sonnet 4 (pre-4.5) does not support extended thinking.
        assert_eq!(thinking_cfg(&[], "claude-sonnet-4-20250514"), None);
        // Haiku family never: no thinking support, small output cap.
        for m in [
            "claude-haiku-4-5-20251001",
            "claude-3-5-haiku-20241022",
        ] {
            assert!(thinking_cfg(&[], m).is_none(), "{m} must not get thinking");
        }
        // Unknown ids: conservative — no thinking.
        assert_eq!(thinking_cfg(&[], "totally-unknown-model"), None);
    }

    #[test]
    fn thinking_respects_model_declared_capability() {
        // A custom/plugin model declaring support (provider.json or discovery)
        // gets thinking even though the catalog doesn't know the id — this is
        // what makes thinking work with custom providers.
        let models = vec![
            crate::providers::types::ModelInfo {
                id: "my-custom-reasoner".into(),
                name: "Custom".into(),
                context_window: None,
                is_free: false,
                is_vision: false,
                max_tokens: None,
                reasoning_effort: None,
                supports_thinking: true,
                thinking_budget: Some(20_000),
            },
            crate::providers::catalog::enrich("plain-model", "Plain".into(), None, None, None, None),
        ];
        // Declared budget wins.
        assert_eq!(thinking_cfg(&models, "my-custom-reasoner"), Some(20_000));
        // Declared NOT capable → no thinking, even for a known family id.
        assert_eq!(thinking_cfg(&models, "plain-model"), None);
        // Catalog fallback for ids absent from the list.
        assert!(thinking_cfg(&models, "claude-sonnet-4-5-20250929").is_some());
    }

    #[test]
    fn thinking_max_tokens_exceeds_budget() {
        assert!(THINKING_MAX_TOKENS > THINKING_BUDGET_TOKENS);
        // Leaves room for output after the full budget is spent.
        assert!(THINKING_MAX_TOKENS - THINKING_BUDGET_TOKENS >= 8_000);
    }
}