use crate::plugins::PluginManager;
use crate::providers::openai_compat::OpenAICompatProvider;
use crate::providers::types::{ModelInfo, ProviderSource};
use crate::providers::ProviderRegistry;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderDef {
    pub id: String,
    pub name: String,
    pub base_url: String,
    /// "api-key" (send as Bearer from settings[provider_id]) or "none"
    #[serde(default = "default_auth")]
    pub auth: String,
    /// When true, fetch {base_url}/models at load time and replace the static list.
    #[serde(default)]
    pub discover: bool,
    #[serde(default)]
    pub models: Vec<ModelInfo>,
}

fn default_auth() -> String {
    "api-key".into()
}

pub fn load_provider_def(dir: &Path) -> Result<ProviderDef, String> {
    let p = dir.join("provider.json");
    if !p.exists() {
        return Err(format!("no provider.json in {}", dir.display()));
    }
    let text = std::fs::read_to_string(&p).map_err(|e| e.to_string())?;
    parse_provider_def(&text)
}

pub fn parse_provider_def(text: &str) -> Result<ProviderDef, String> {
    let def: ProviderDef = serde_json::from_str(text).map_err(|e| e.to_string())?;
    if def.id.is_empty() || def.name.is_empty() || def.base_url.is_empty() {
        return Err("provider.json needs id, name and base_url".into());
    }
    if !looks_like_http_url(&def.base_url) {
        return Err(format!("base_url must be http(s), got: {}", def.base_url));
    }
    if def.auth != "api-key" && def.auth != "none" {
        return Err(format!("auth must be \"api-key\" or \"none\", got: {}", def.auth));
    }
    Ok(def)
}

fn looks_like_http_url(s: &str) -> bool {
    s.starts_with("http://") || s.starts_with("https://")
}

/// Register every enabled plugin that ships a provider.json into the registry.
/// Plugin providers are tagged `ProviderSource::Plugin` so the UI can label them.
/// Returns how many providers were registered.
pub fn register_plugin_providers(
    registry: &ProviderRegistry,
    manager: &PluginManager,
    api_keys: &std::collections::HashMap<String, String>,
    app: &tauri::AppHandle,
) -> usize {
    let mut registered = 0;
    for info in manager.list() {
        if !info.enabled {
            continue;
        }
        let dir = manager.plugin_dir(&info.id);
        let Ok(def) = load_provider_def(&dir) else {
            continue;
        };
        // Only register if the manifest actually contributes this provider id
        let Some(manifest) = manager.manifest(&info.id) else {
            continue;
        };
        if !manifest.contributes.providers.contains(&def.id) {
            eprintln!(
                "[plugin:{}] provider.json declares id \"{}\" but manifest contributes.providers does not include it",
                info.id, def.id
            );
            continue;
        }

        let provider = OpenAICompatProvider {
            id: def.id.clone(),
            name: def.name.clone(),
            base_url: def.base_url.clone(),
            source: ProviderSource::Plugin {
                plugin_id: info.id.clone(),
            },
            models: def.models.clone(),
        };
        registry.register(provider);
        registered += 1;

        // Best-effort model discovery: GET {base_url}/models with a bearer key.
        if def.discover {
            let key = api_keys.get(&def.id).cloned().unwrap_or_default();
            crate::providers::spawn_model_discovery(&def.id, &def.base_url, key, app.clone());
        }
    }
    registered
}

/// Remove providers contributed by plugins (before re-registering on hot reload).
pub fn unregister_plugin_providers(registry: &ProviderRegistry, manager: &PluginManager) {
    for info in manager.list() {
        let dir = manager.plugin_dir(&info.id);
        if let Ok(def) = load_provider_def(&dir) {
            registry.unregister(&def.id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::manifest::{parse_manifest, PluginManifest};

    #[test]
    fn parses_full_provider_def() {
        let def = parse_provider_def(
            r#"{"id":"zen-warp","name":"Zen","base_url":"https://api.zen.example/v1","auth":"none","models":[{"id":"gpt-mega","name":"GPT Mega"}]}"#,
        )
        .unwrap();
        assert_eq!(def.id, "zen-warp");
        assert_eq!(def.auth, "none");
        assert_eq!(def.models.len(), 1);
        assert_eq!(def.models[0].id, "gpt-mega");
    }

    #[test]
    fn rejects_bad_provider_def() {
        assert!(parse_provider_def(r#"{"id":"x","name":"y"}"#).is_err()); // no base_url
        assert!(parse_provider_def(r#"{"id":"x","name":"y","base_url":"ftp://nope"}"#).is_err());
        assert!(parse_provider_def(r#"{"id":"x","name":"y","base_url":"http://ok","auth":"magic"}"#).is_err());
    }

    #[test]
    fn registration_requires_manifest_match() {
        let dir = tempfile::tempdir().unwrap();
        // plugin with manifest declaring provider "warp" but provider.json id "other"
        std::fs::write(
            dir.path().join("manifest.json"),
            r#"{"id":"zen","name":"Zen","version":"1","contributes":{"providers":["warp"]}}"#,
        )
        .unwrap();
        std::fs::write(
            dir.path().join("provider.json"),
            r#"{"id":"other","name":"Other","base_url":"https://x.example/v1"}"#,
        )
        .unwrap();
        let m = PluginManifest {
            id: "zen".into(),
            name: "Zen".into(),
            version: "1".into(),
            description: String::new(),
            contributes: parse_manifest(
                r#"{"id":"zen","name":"Zen","version":"1","contributes":{"providers":["warp"]}}"#,
            )
            .unwrap()
            .contributes,
        };
        // dir-based flow: use manager-free path — load_provider_def + matching check manually
        let def = load_provider_def(dir.path()).unwrap();
        assert!(!m.contributes.providers.contains(&def.id));
    }
}
