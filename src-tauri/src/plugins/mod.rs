pub mod manifest;
pub mod provider;
pub mod watcher;

use manifest::{load_manifest_from_dir, Contributes, PluginManifest};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::RwLock;

#[derive(Debug, Clone, Serialize)]
pub struct PluginInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub enabled: bool,
    pub contributes: Contributes,
    pub has_component: bool,
    pub has_skill: bool,
    pub folder: String,
}

pub struct PluginManager {
    pub dir: PathBuf,
    disabled: RwLock<Vec<String>>,
}

impl PluginManager {
    pub fn new(dir: PathBuf, disabled: Vec<String>) -> Self {
        Self {
            dir,
            disabled: RwLock::new(disabled),
        }
    }

    pub fn plugin_dir(&self, id: &str) -> PathBuf {
        self.dir.join(id)
    }

    pub fn is_enabled(&self, id: &str) -> bool {
        !self.disabled.read().unwrap().contains(&id.to_string())
    }

    pub fn set_disabled(&self, id: &str, disabled: bool) {
        let mut list = self.disabled.write().unwrap();
        if disabled {
            if !list.contains(&id.to_string()) {
                list.push(id.to_string());
            }
        } else {
            list.retain(|x| x != id);
        }
    }

    pub fn disabled_ids(&self) -> Vec<String> {
        self.disabled.read().unwrap().clone()
    }

    pub fn manifest(&self, id: &str) -> Option<PluginManifest> {
        load_manifest_from_dir(&self.plugin_dir(id)).ok()
    }

    pub fn has_component(&self, id: &str) -> bool {
        self.plugin_dir(id).join("component.js").is_file()
    }

    pub fn has_skill(&self, id: &str) -> bool {
        self.plugin_dir(id).join("skill.md").is_file()
    }

    pub fn component_code(&self, id: &str) -> Result<String, String> {
        let p = self.plugin_dir(id).join("component.js");
        std::fs::read_to_string(&p).map_err(|e| e.to_string())
    }

    pub fn skill_md(&self, id: &str) -> Result<String, String> {
        let p = self.plugin_dir(id).join("skill.md");
        std::fs::read_to_string(&p).map_err(|e| e.to_string())
    }

    /// All plugins that have a valid manifest, in directory order.
    pub fn list(&self) -> Vec<PluginInfo> {
        let mut out = Vec::new();
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return out;
        };
        let mut ids: Vec<String> = entries
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
            .filter_map(|e| e.file_name().into_string().ok())
            .collect();
        ids.sort();
        for id in ids {
            let dir = self.dir.join(&id);
            let Ok(m) = load_manifest_from_dir(&dir) else {
                continue;
            };
            if m.id != id {
                eprintln!("[plugin] folder \"{id}\" manifest id \"{}\" mismatch", m.id);
                continue;
            }
            out.push(PluginInfo {
                id: m.id.clone(),
                name: m.name.clone(),
                version: m.version.clone(),
                description: m.description.clone(),
                enabled: self.is_enabled(&m.id),
                contributes: m.contributes.clone(),
                has_component: dir.join("component.js").is_file(),
                has_skill: dir.join("skill.md").is_file(),
                folder: dir.display().to_string(),
            });
        }
        out
    }

    /// Full text of every enabled plugin's skill.md, prefixed with `# <id>`, blank-line separated.
    pub fn collect_skill_markdown(&self) -> String {
        let mut parts = Vec::new();
        for info in self.list().sort_enabled() {
            if let Ok(md) = self.skill_md(&info.id) {
                if !md.trim().is_empty() {
                    parts.push(format!("# Plugin skill: {}\n\n{}", info.id, md.trim()));
                }
            }
        }
        parts.join("\n\n---\n\n")
    }

    /// Create the demo plugin folder if the plugins dir has no plugin folders at all.
    /// Returns true if a plugin was written.
    pub fn ensure_demo_plugin(&self) -> bool {
        if !self.dir.exists() {
            let _ = std::fs::create_dir_all(&self.dir);
        }
        let has_plugins = std::fs::read_dir(&self.dir)
            .map(|mut it| it.any(|e| e.ok().map(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false)).unwrap_or(false)))
            .unwrap_or(false);
        if has_plugins {
            return false;
        }
        let demo = self.plugin_dir("demo");
        let _ = std::fs::create_dir_all(&demo);
        std::fs::write(
            demo.join("manifest.json"),
            include_str!("../../assets/demo-plugin/manifest.json"),
        )
        .ok();
        std::fs::write(
            demo.join("component.js"),
            include_str!("../../assets/demo-plugin/component.js"),
        )
        .ok();
        std::fs::write(
            demo.join("skill.md"),
            include_str!("../../assets/demo-plugin/skill.md"),
        )
        .ok();
        std::fs::write(
            demo.join("provider.json"),
            include_str!("../../assets/demo-plugin/provider.json"),
        )
        .ok();
        true
    }
}

/// tiny helper: keep sort by id but enabled first — used by collect_skill_markdown
trait SortEnabled {
    fn sort_enabled(self) -> Vec<PluginInfo>;
}
impl SortEnabled for Vec<PluginInfo> {
    fn sort_enabled(self) -> Vec<PluginInfo> {
        let mut v = self;
        v.sort_by_key(|p| (!p.enabled, p.id.clone()));
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk_manager(files: Vec<(&str, &str)>) -> (PluginManager, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        for (name, content) in files {
            let path = dir.path().join(name);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(path, content).unwrap();
        }
        (PluginManager::new(dir.path().to_path_buf(), vec![]), dir)
    }

    #[test]
    fn lists_plugins_and_flags() {
        let (m, _dir) = mk_manager(vec![
            (
                "demo/manifest.json",
                r#"{"id":"demo","name":"Demo","version":"1","contributes":{"slots":["chat-toolbar-extra"]}}"#,
            ),
            ("demo/component.js", "export default class {}"),
            (
                ".hidden/manifest.json",
                r#"{"id":".hidden","name":"H","version":"1"}"#,
            ),
        ]);
        let list = m.list();
        assert_eq!(list.len(), 2);
        let demo = list.iter().find(|p| p.id == "demo").unwrap();
        assert!(demo.has_component);
        assert!(!demo.has_skill);
        assert!(demo.enabled);
        assert!(demo.contributes.slots.contains(&"chat-toolbar-extra".to_string()));
        let hidden = list.iter().find(|p| p.id == ".hidden").unwrap();
        assert!(!hidden.has_component);
    }

    #[test]
    fn lists_only_valid_manifests() {
        let (m, _dir) = mk_manager(vec![
            ("broken/manifest.json", "not json"),
            ("demo/manifest.json", r#"{"id":"demo","name":"Demo","version":"0.1"}"#),
        ]);
        let list = m.list();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, "demo");
    }

    #[test]
    fn disabled_state() {
        let (m, _dir) = mk_manager(vec![(
            "demo/manifest.json",
            r#"{"id":"demo","name":"Demo","version":"0.1"}"#,
        )]);
        assert!(m.is_enabled("demo"));
        m.set_disabled("demo", true);
        assert!(!m.is_enabled("demo"));
        let list = m.list();
        assert!(!list[0].enabled);
        m.set_disabled("demo", false);
        assert!(m.is_enabled("demo"));
    }

    #[test]
    fn skills_markdown() {
        let (m, _dir) = mk_manager(vec![
            ("demo/manifest.json", r#"{"id":"demo","name":"Demo","version":"0.1"}"#),
            ("demo/skill.md", "always be concise"),
        ]);
        let md = m.collect_skill_markdown();
        assert!(md.contains("# Plugin skill: demo"));
        assert!(md.contains("always be concise"));
    }
}