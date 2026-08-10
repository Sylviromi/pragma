pub mod types;
pub mod anthropic;
pub mod catalog;
pub mod openai_compat;
pub mod sse;

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tauri::Manager;

use types::{ContentBlock, ModelInfo, ProviderInfo, ToolDefinition, Usage};
use anthropic::AnthropicProvider;
use openai_compat::OpenAICompatProvider;

/// A callable provider handle. Kept as an enum (rather than `dyn Provider` with an
/// async method) so the agentic loop can await a concrete `call()` per provider —
/// native async fn in traits is not object-safe.
#[derive(Clone)]
pub enum ProviderHandle {
    Anthropic(AnthropicProvider),
    OpenAICompat(OpenAICompatProvider),
}

impl ProviderHandle {
    pub fn info(&self) -> ProviderInfo {
        match self {
            ProviderHandle::Anthropic(p) => p.info(),
            ProviderHandle::OpenAICompat(p) => p.info(),
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
        match self {
            ProviderHandle::Anthropic(p) => {
                p.call(messages, model, api_key, tools, thinking, app_handle, session_id).await
            }
            ProviderHandle::OpenAICompat(p) => {
                p.call(messages, model, api_key, tools, thinking, app_handle, session_id).await
            }
        }
    }
}

#[derive(Default, Clone)]
pub struct ProviderRegistry {
    inner: Arc<RwLock<HashMap<String, ProviderHandle>>>,
}

impl ProviderRegistry {
    pub fn register<P: Into<ProviderHandle> + 'static>(&self, p: P) {
        let handle: ProviderHandle = p.into();
        let info = handle.info();
        self.inner
            .write()
            .unwrap()
            .insert(info.id.clone(), handle);
    }
    pub fn get(&self, id: &str) -> Option<ProviderHandle> {
        self.inner.read().unwrap().get(id).cloned()
    }
    pub fn list_infos(&self) -> Vec<ProviderInfo> {
        self.inner
            .read()
            .unwrap()
            .values()
            .map(|p| p.info())
            .collect()
    }

    /// Remove a provider by id (used when a plugin is disabled or uninstalled).
    pub fn unregister(&self, id: &str) {
        self.inner.write().unwrap().remove(id);
    }

    /// Replace the model list of an existing provider (used by /v1/models discovery).
    pub fn upsert_models(&self, id: &str, models: Vec<ModelInfo>) {
        let mut inner = self.inner.write().unwrap();
        if let Some(handle) = inner.get_mut(id) {
            match handle {
                ProviderHandle::Anthropic(p) => p.models = models,
                ProviderHandle::OpenAICompat(p) => p.models = models,
            }
        }
    }

    /// Re-run model discovery for one provider (e.g. right after an API key was
    /// saved). No-op when the provider is unknown or has no key set.
    pub fn trigger_discovery(&self, id: &str, app: &tauri::AppHandle) {
        let Some(handle) = self.inner.read().unwrap().get(id).cloned() else {
            return;
        };
        match handle {
            ProviderHandle::Anthropic(_) => {
                let key = app
                    .try_state::<crate::AppState>()
                    .map(|s| s.settings.read().unwrap().api_keys.get("anthropic").cloned())
                    .flatten();
                anthropic::spawn_discovery(app.clone(), key);
            }
            ProviderHandle::OpenAICompat(p) => {
                if let Some(key) = app
                    .try_state::<crate::AppState>()
                    .map(|s| s.settings.read().unwrap().api_keys.get(&p.id).cloned())
                    .flatten()
                {
                    spawn_model_discovery(&p.id, &p.base_url, key, app.clone());
                }
            }
        }
    }
}

/// Fetch a provider's `/v1/models` list and merge it with catalog knowledge
/// (context windows, vision, free tiers). Returns the raw discovered models;
/// `Vec::new()` means the endpoint wasn't reachable or returned nothing useful.
pub async fn fetch_openai_models(base_url: &str, api_key: &str) -> Vec<ModelInfo> {
    let url = format!("{}/models", base_url.trim_end_matches('/'));
    let Ok(resp) = reqwest::Client::new()
        .get(&url)
        .timeout(std::time::Duration::from_secs(6))
        .bearer_auth(api_key)
        .send()
        .await
    else {
        return Vec::new();
    };
    if !resp.status().is_success() {
        return Vec::new();
    }
    let Ok(body) = resp.json::<serde_json::Value>().await else {
        return Vec::new();
    };

    let Some(arr) = body["data"].as_array() else {
        return Vec::new();
    };
    let mut out: Vec<ModelInfo> = Vec::with_capacity(arr.len());
    for m in arr {
        let Some(id) = m["id"].as_str() else {
            continue;
        };
        // Context window: providers disagree on the field name. OpenRouter
        // sends context_length; vLLM/SGLang and various local runtimes send
        // max_model_len / max_context_length / ctx_len.
        let ctx = ["context_length", "context_window", "max_model_len", "max_context_length", "ctx_len"]
            .iter()
            .find_map(|k| m.get(*k).and_then(|v| v.as_u64()).map(|n| n as u32));
        // Humanized name when the server provides one (OpenRouter), else id.
        let name = m["metadata"]["humanized_name"]
            .as_str()
            .or_else(|| m["name"].as_str())
            .unwrap_or(id)
            .to_string();
        // Free tier: OpenRouter prices prompt tokens as "0".
        let free = m["pricing"]["prompt"]
            .as_str()
            .map(|s| s.parse::<f64>().map(|v| v == 0.0).unwrap_or(false))
            .or_else(|| m["pricing"]["prompt"].as_f64().map(|v| v == 0.0));
        // Thinking capability: servers disagree on the field. Accept booleans
        // and "true"/"false" strings, top-level or nested; falls back to the
        // family catalog when the server says nothing.
        let thinking = ["supports_thinking", "supportsThinking", "thinking", "reasoning"]
            .iter()
            .find_map(|k| m.get(*k).and_then(boolish))
            .or_else(|| {
                m.get("metadata").and_then(|md| {
                    ["supports_thinking", "supportsThinking", "thinking", "reasoning"]
                        .iter()
                        .find_map(|k| md.get(*k).and_then(boolish))
                })
            })
            .or_else(|| {
                m.get("capabilities")
                    .and_then(|c| c.get("thinking"))
                    .and_then(boolish)
            });
        // Optional thinking budget + output ceiling from the same sources.
        let thinking_budget = m["thinking_budget"]
            .as_u64()
            .or_else(|| m["thinkingBudget"].as_u64())
            .map(|n| n as u32);
        let max_tokens = m["max_tokens"]
            .as_u64()
            .or_else(|| m["maxTokens"].as_u64())
            .map(|n| n as u32);
        let mut info = catalog::enrich(id, name, ctx, None, free, thinking);
        info.thinking_budget = thinking_budget;
        info.max_tokens = max_tokens;
        info.reasoning_effort = m["reasoning_effort"]
            .as_str()
            .map(|s| s.to_string())
            .or_else(|| {
                m.get("metadata")
                    .and_then(|md| md["reasoning_effort"].as_str())
                    .map(|s| s.to_string())
            });
        out.push(info);
    }
    out
}

/// Best-effort boolean parse for capability fields that may arrive as JSON
/// booleans or "true"/"false" strings.
fn boolish(v: &serde_json::Value) -> Option<bool> {
    v.as_bool().or_else(|| v.as_str().and_then(|s| s.parse().ok()))
}

/// Run an async closure on a detached thread with its own current-thread
/// tokio runtime. Used for best-effort model-discovery fetches that must not
/// block or outlive the main runtime.
pub fn run_detached<F, Fut>(f: F)
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: std::future::Future<Output = ()> + Send + 'static,
{
    std::thread::spawn(move || {
        let rt = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(_) => return,
        };
        rt.block_on(f());
    });
}

/// Detached best-effort `/v1/models` fetch; enriches with catalog knowledge and
/// upserts the results into the registry, then tells the UI to refresh.
pub fn spawn_model_discovery(
    provider_id: &str,
    base_url: &str,
    api_key: String,
    app: tauri::AppHandle,
) {
    use tauri::Emitter;
    let provider_id = provider_id.to_string();
    let base_url = base_url.to_string();
    run_detached(move || async move {
        let models = fetch_openai_models(&base_url, &api_key).await;
        if models.is_empty() {
            return;
        }
        if let Some(registry) = app.try_state::<crate::AppState>() {
            registry.registry.upsert_models(&provider_id, models);
            let _ = app.emit("providers:changed", (provider_id,));
        }
    });
}

/// Kick off discovery for the built-in providers at startup (and whenever an
/// API key changes). Fetches real model lists where possible and replaces the
/// static fallbacks.
pub fn spawn_builtin_discovery(app: &tauri::AppHandle) {
    let keys = { app.state::<crate::AppState>().settings.read().unwrap().api_keys.clone() };

    // Anthropic: real /v1/models with the API key.
    anthropic::spawn_discovery(app.clone(), keys.get("anthropic").cloned());

    for (id, base_url) in [
        ("deepseek", "https://api.deepseek.com/v1"),
        ("opencode", "http://localhost:4000/v1"),
    ] {
        let key = keys.get(id).cloned().unwrap_or_default();
        spawn_model_discovery(id, base_url, key, app.clone());
    }
}

impl From<AnthropicProvider> for ProviderHandle {
    fn from(p: AnthropicProvider) -> Self {
        ProviderHandle::Anthropic(p)
    }
}

impl From<OpenAICompatProvider> for ProviderHandle {
    fn from(p: OpenAICompatProvider) -> Self {
        ProviderHandle::OpenAICompat(p)
    }
}