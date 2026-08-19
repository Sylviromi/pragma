use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: String,
    /// Who made it (shown in Settings → Plugins; also used by the future
    /// plugin registry for attribution).
    #[serde(default)]
    pub author: String,
    /// Publisher identity — namespaces plugins without touching the flat id
    /// scheme: first-party pragma plugins use "pragma" (badged in the UI),
    /// third parties use their own handle. The install SOURCE (npm scope,
    /// git owner) provides ecosystem-level namespacing on top.
    #[serde(default)]
    pub publisher: String,
    /// Source repo (GitHub etc.) — where the source and issues live.
    #[serde(default)]
    pub repository: String,
    /// Project page for humans (docs, screenshots).
    #[serde(default)]
    pub homepage: String,
    /// URL that serves the LATEST packaged .zip of this plugin. The update
    /// machinery (planned: version poll → re-install through the same
    /// install path as manual zips) will use it; ignored for now.
    #[serde(default, rename = "updateUrl")]
    pub update_url: String,
    /// Development-only plugin — loads only in debug builds (`tauri dev` /
    /// `cargo run`), never in a shipped release app. Used for first-party
    /// dev tooling (pragma-dev) that must not ship to users.
    #[serde(default)]
    pub dev: bool,
    #[serde(default)]
    pub contributes: Contributes,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct Contributes {
    #[serde(default)]
    pub providers: Vec<String>,
    #[serde(default)]
    pub slots: Vec<String>,
    #[serde(default)]
    pub commands: Vec<String>,
    #[serde(default)]
    pub skills: Vec<String>,
}

pub fn parse_manifest(text: &str) -> Result<PluginManifest, String> {
    let m: PluginManifest = serde_json::from_str(text).map_err(|e| e.to_string())?;
    if m.id.is_empty() || m.name.is_empty() || m.version.is_empty() {
        return Err("plugin manifest needs id, name and version".into());
    }
    let lowered = m.id.trim().to_lowercase();
    if m.id != lowered || m.id.contains([' ', '/', '\\']) {
        return Err(format!("invalid plugin id: {}", m.id));
    }
    Ok(m)
}

pub fn load_manifest_from_dir(dir: &Path) -> Result<PluginManifest, String> {
    let p = dir.join("manifest.json");
    if !p.exists() {
        return Err(format!("no manifest.json in {}", dir.display()));
    }
    let text = std::fs::read_to_string(&p).map_err(|e| e.to_string())?;
    parse_manifest(&text)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FULL: &str = r#"{
        "id": "zen-warp",
        "name": "Zen WARP Proxy",
        "version": "1.0.0",
        "description": "Routes through Zerox proxy",
        "contributes": {
            "providers": ["zen-warp"],
            "slots": ["input-toolbar", "chat-toolbar-extra"],
            "commands": ["/zen"],
            "skills": ["skill.md"]
        }
    }"#;

    #[test]
    fn parses_full_manifest() {
        let m = parse_manifest(FULL).unwrap();
        assert_eq!(m.id, "zen-warp");
        assert_eq!(m.name, "Zen WARP Proxy");
        assert_eq!(m.version, "1.0.0");
        assert_eq!(m.contributes.providers, vec!["zen-warp"]);
        assert_eq!(m.contributes.slots.len(), 2);
        assert_eq!(m.contributes.commands, vec!["/zen"]);
        assert_eq!(m.contributes.skills, vec!["skill.md"]);
    }

    #[test]
    fn parses_optional_metadata_fields() {
        let m = parse_manifest(
            r#"{"id":"demo","name":"Demo","version":"1.2.3","author":"Ada","publisher":"pragma","repository":"https://github.com/ada/demo","homepage":"https://ada.dev/demo","updateUrl":"https://ada.dev/demo/latest.zip"}"#,
        )
        .unwrap();
        assert_eq!(m.author, "Ada");
        assert_eq!(m.publisher, "pragma");
        assert_eq!(m.repository, "https://github.com/ada/demo");
        assert_eq!(m.homepage, "https://ada.dev/demo");
        assert_eq!(m.update_url, "https://ada.dev/demo/latest.zip");
    }

    #[test]
    fn defaults_for_missing_contributes() {
        let m = parse_manifest(r#"{"id":"demo","name":"Demo","version":"0.1.0"}"#).unwrap();
        assert_eq!(m.description, "");
        assert!(m.author.is_empty());
        assert!(m.publisher.is_empty());
        assert!(m.repository.is_empty());
        assert!(m.homepage.is_empty());
        assert!(m.update_url.is_empty());
        assert!(m.contributes.providers.is_empty());
        assert!(m.contributes.slots.is_empty());
        assert!(m.contributes.commands.is_empty());
        assert!(m.contributes.skills.is_empty());
    }

    #[test]
    fn rejects_bad_id() {
        assert!(parse_manifest(r#"{"id":"Has Space","name":"x","version":"1"}"#).is_err());
        assert!(parse_manifest(r#"{"id":"a/b","name":"x","version":"1"}"#).is_err());
        assert!(parse_manifest(r#"{"id":"OKID","name":"x","version":"1"}"#).is_err());
        // uppercase
    }

    #[test]
    fn rejects_missing_fields() {
        assert!(parse_manifest(r#"{"id":"demo","name":"x"}"#).is_err());
        assert!(parse_manifest("not json").is_err());
    }
}
