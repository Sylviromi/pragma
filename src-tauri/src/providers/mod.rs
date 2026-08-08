pub mod types;
pub mod anthropic;
pub mod openai_compat;

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use types::{ContentBlock, ModelInfo, ProviderInfo, ToolDefinition};
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
        app_handle: &tauri::AppHandle,
        session_id: &str,
    ) -> Result<Vec<ContentBlock>, String> {
        match self {
            ProviderHandle::Anthropic(_) => {
                AnthropicProvider::call(messages, model, api_key, tools, app_handle, session_id).await
            }
            ProviderHandle::OpenAICompat(p) => {
                p.call(messages, model, api_key, tools, app_handle, session_id).await
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
                ProviderHandle::OpenAICompat(p) => p.models = models,
                ProviderHandle::Anthropic(p) => {
                    let _ = p;
                }
            }
        }
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