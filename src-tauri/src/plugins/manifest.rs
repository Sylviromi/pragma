use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: String,
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
    fn defaults_for_missing_contributes() {
        let m = parse_manifest(r#"{"id":"demo","name":"Demo","version":"0.1.0"}"#).unwrap();
        assert_eq!(m.description, "");
        assert!(m.contributes.providers.is_empty());
        assert!(m.contributes.slots.is_empty());
        assert!(m.contributes.commands.is_empty());
        assert!(m.contributes.skills.is_empty());
    }

    #[test]
    fn rejects_bad_id() {
        assert!(parse_manifest(r#"{"id":"Has Space","name":"x","version":"1"}"#).is_err());
        assert!(parse_manifest(r#"{"id":"a/b","name":"x","version":"1"}"#).is_err());
        assert!(parse_manifest(r#"{"id":"OKID","name":"x","version":"1"}"#).is_err()); // uppercase
    }

    #[test]
    fn rejects_missing_fields() {
        assert!(parse_manifest(r#"{"id":"demo","name":"x"}"#).is_err());
        assert!(parse_manifest("not json").is_err());
    }
}