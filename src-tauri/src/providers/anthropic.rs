use super::types::{
    AgentEvent, ContentBlock, ModelInfo, ProviderInfo, ProviderSource, ToolDefinition,
};
use futures_util::StreamExt;
use tauri::ipc::Channel;

#[derive(Clone)]
pub struct AnthropicProvider;

impl AnthropicProvider {
    pub fn info(&self) -> ProviderInfo {
        ProviderInfo {
            id: "anthropic".into(),
            name: "Anthropic".into(),
            source: ProviderSource::Builtin,
            models: vec![
                ModelInfo {
                    id: "claude-sonnet-4-6".into(),
                    name: "Claude Sonnet 4.6".into(),
                    context_window: Some(200_000),
                    is_free: false,
                    is_vision: true,
                },
                ModelInfo {
                    id: "claude-opus-4-6".into(),
                    name: "Claude Opus 4.6".into(),
                    context_window: Some(200_000),
                    is_free: false,
                    is_vision: true,
                },
                ModelInfo {
                    id: "claude-haiku-4-5-20251001".into(),
                    name: "Claude Haiku 4.5".into(),
                    context_window: Some(200_000),
                    is_free: false,
                    is_vision: true,
                },
            ],
        }
    }
}

impl AnthropicProvider {
    pub async fn call(
        messages: &[serde_json::Value],
        model: &str,
        api_key: &str,
        tools: &[ToolDefinition],
        channel: &Channel<AgentEvent>,
    ) -> Result<Vec<ContentBlock>, String> {
        let client = reqwest::Client::new();

        // Anthropic requires system content in a top-level "system" field,
        // not as a {"role":"system"} entry inside messages.
        let system_text: String = messages
            .iter()
            .filter(|m| m["role"].as_str() == Some("system"))
            .filter_map(|m| m["content"].as_str())
            .collect::<Vec<_>>()
            .join("\n\n");
        let chat_messages: Vec<&serde_json::Value> = messages
            .iter()
            .filter(|m| m["role"].as_str() != Some("system"))
            .collect();

        let mut body = serde_json::json!({
            "model": model,
            "max_tokens": 8096,
            "stream": true,
            "messages": chat_messages,
        });
        if !system_text.is_empty() {
            body["system"] = serde_json::Value::String(system_text);
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

        let resp = client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !resp.status().is_success() {
            let msg = resp.text().await.unwrap_or_default();
            channel.send(AgentEvent::Error { message: msg.clone() }).ok();
            return Err(msg);
        }

        // Parse SSE stream, collect content blocks.
        // SSE lines can span multiple HTTP chunks — buffer and process complete lines only.
        let mut blocks: Vec<ContentBlock> = Vec::new();
        let mut current_text = String::new();
        let mut current_tool: Option<(String, String, String)> = None; // (id, name, input_json)
        let mut stream = resp.bytes_stream();
        let mut buf = String::new();

        'outer: while let Some(chunk) = stream.next().await {
            let bytes = chunk.map_err(|e| e.to_string())?;
            buf.push_str(&String::from_utf8_lossy(&bytes));
            loop {
                let Some(nl) = buf.find('\n') else { break };
                let line = buf[..nl].trim_end_matches('\r').to_string();
                buf.drain(..=nl);
                let Some(data) = line.strip_prefix("data: ") else {
                    continue;
                };
                if data == "[DONE]" {
                    break 'outer;
                }
                let Ok(ev) = serde_json::from_str::<serde_json::Value>(data) else {
                    continue;
                };
                match ev["type"].as_str().unwrap_or("") {
                    "content_block_start" => {
                        let bt = ev["content_block"]["type"].as_str().unwrap_or("");
                        if bt == "tool_use" {
                            let id = ev["content_block"]["id"].as_str().unwrap_or("").to_string();
                            let name = ev["content_block"]["name"].as_str().unwrap_or("").to_string();
                            current_tool = Some((id, name, String::new()));
                        }
                    }
                    "content_block_delta" => {
                        let dt = ev["delta"]["type"].as_str().unwrap_or("");
                        if dt == "text_delta" {
                            let t = ev["delta"]["text"].as_str().unwrap_or("");
                            current_text.push_str(t);
                            channel.send(AgentEvent::Token { content: t.to_string() }).ok();
                        } else if dt == "input_json_delta" {
                            if let Some((_, _, ref mut json_buf)) = current_tool {
                                json_buf.push_str(ev["delta"]["partial_json"].as_str().unwrap_or(""));
                            }
                        }
                    }
                    "content_block_stop" => {
                        if let Some((id, name, json_buf)) = current_tool.take() {
                            let input: serde_json::Value =
                                serde_json::from_str(&json_buf).unwrap_or(serde_json::Value::Null);
                            blocks.push(ContentBlock::ToolUse { id, name, input });
                        } else if !current_text.is_empty() {
                            blocks.push(ContentBlock::Text {
                                text: std::mem::take(&mut current_text),
                            });
                        }
                    }
                    _ => {}
                }
            }
        }

        Ok(blocks)
    }
}