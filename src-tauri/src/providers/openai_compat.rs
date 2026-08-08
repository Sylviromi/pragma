use super::types::{
    AgentEvent, ContentBlock, ModelInfo, ProviderInfo, ProviderSource, ToolDefinition,
};
use futures_util::StreamExt;
use tauri::ipc::Channel;

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
    #[allow(clippy::too_many_arguments)]
    pub async fn call(
        &self,
        messages: &[serde_json::Value],
        model: &str,
        api_key: &str,
        tools: &[ToolDefinition],
        channel: &Channel<AgentEvent>,
    ) -> Result<Vec<ContentBlock>, String> {
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
            .json(&body);
        if !api_key.is_empty() {
            req = req.bearer_auth(api_key);
        }

        let resp = req.send().await.map_err(|e| e.to_string())?;
        if !resp.status().is_success() {
            let msg = resp.text().await.unwrap_or_default();
            channel.send(AgentEvent::Error { message: msg.clone() }).ok();
            return Err(msg);
        }

        let mut full_text = String::new();
        // tool_calls by index: (id, name, accumulated_arguments)
        let mut tool_calls: std::collections::BTreeMap<usize, (String, String, String)> =
            std::collections::BTreeMap::new();

        let mut stream = resp.bytes_stream();
        // SSE lines can span multiple HTTP chunks — buffer and process complete lines only.
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
                if data.trim() == "[DONE]" {
                    break 'outer;
                }
                let Ok(ev) = serde_json::from_str::<serde_json::Value>(data) else {
                    continue;
                };
                let delta = &ev["choices"][0]["delta"];

                // Text tokens
                if let Some(t) = delta["content"].as_str() {
                    if !t.is_empty() {
                        full_text.push_str(t);
                        channel.send(AgentEvent::Token { content: t.to_string() }).ok();
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
        if !full_text.is_empty() {
            blocks.push(ContentBlock::Text { text: full_text });
        }
        for (_, (id, name, args)) in tool_calls {
            let input = serde_json::from_str(&args).unwrap_or(serde_json::Value::Object(Default::default()));
            blocks.push(ContentBlock::ToolUse { id, name, input });
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
                    // Anthropic content-block array — split into text + tool_calls
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
                    if tcs.is_empty() {
                        out.push(serde_json::json!({ "role": "assistant", "content": text }));
                    } else {
                        out.push(serde_json::json!({
                            "role": "assistant",
                            "content": if text.is_empty() { serde_json::Value::Null } else { serde_json::Value::String(text) },
                            "tool_calls": tcs,
                        }));
                    }
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
    out
}

pub fn deepseek() -> OpenAICompatProvider {
    OpenAICompatProvider {
        id: "deepseek".into(),
        name: "DeepSeek".into(),
        base_url: "https://api.deepseek.com/v1".into(),
        source: ProviderSource::Builtin,
        models: vec![ModelInfo {
            id: "deepseek-chat".into(),
            name: "DeepSeek V3".into(),
            context_window: Some(64_000),
            is_free: false,
            is_vision: false,
        }],
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
