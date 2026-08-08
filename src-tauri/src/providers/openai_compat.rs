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
        _tools: &[ToolDefinition], // tool calling varies by provider, basic streaming only for now
        channel: &Channel<AgentEvent>,
    ) -> Result<Vec<ContentBlock>, String> {
        let client = reqwest::Client::new();
        let body = serde_json::json!({ "model": model, "stream": true, "messages": messages });
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
        let mut stream = resp.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let bytes = chunk.map_err(|e| e.to_string())?;
            for line in String::from_utf8_lossy(&bytes).lines() {
                let Some(data) = line.strip_prefix("data: ") else {
                    continue;
                };
                if data.trim() == "[DONE]" {
                    break;
                }
                let Ok(ev) = serde_json::from_str::<serde_json::Value>(data) else {
                    continue;
                };
                if let Some(t) = ev["choices"][0]["delta"]["content"].as_str() {
                    if !t.is_empty() {
                        full_text.push_str(t);
                        channel
                            .send(AgentEvent::Token {
                                content: t.to_string(),
                            })
                            .ok();
                    }
                }
            }
        }

        Ok(vec![ContentBlock::Text { text: full_text }])
    }
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