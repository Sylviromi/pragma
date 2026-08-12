pub mod manifest;
pub mod provider;
pub mod watcher;

use manifest::{load_manifest_from_dir, Contributes, PluginManifest};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::RwLock;

/// Built-in skills shipped with the app. Their docs live in assets/skills/<id>/skill.md
/// and their tools are dispatched by the Rust ToolExecutor (see tools::definitions::skill_tools).
/// (id, one-line description shown in the skill index).
pub const NATIVE_SKILLS: [(&str, &str); 2] = [
    ("git", "Version control: git status, diff, log, add, commit — load when the task touches git, a repo's history, or staging"),
    ("web", "Web: search the web and fetch URLs for external documentation — load when you need docs or information outside the project"),
];

/// One usable skill: a native built-in, a plugin's legacy top-level skill.md,
/// or one folder under a plugin's skills/ directory.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillInfo {
    /// "git" | "web" | "demo" (legacy) | "demo/frontend-dev" (skills/ subfolder)
    pub id: String,
    /// Display name (folder name for subfolder skills, plugin name for legacy).
    pub name: String,
    pub description: String,
    /// "native" | "plugin" | "plugin-skill" — where the skill comes from.
    pub source: String,
    pub plugin_id: Option<String>,
    /// True when the skill contributes tools (native skills always do;
    /// plugin skills when a tools.json rides along).
    pub has_tools: bool,
    /// True when the skill has a sidecar handler.js (plugin skills only).
    pub has_handler: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub enabled: bool,
    pub contributes: Contributes,
    pub has_component: bool,
    pub has_skill: bool,
    pub skills: Vec<SkillInfo>,
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

    /// Every discoverable skill, enabled only: native built-ins, then each
    /// enabled plugin's `skills/<id>/skill.md` subfolders, then legacy
    /// top-level `skill.md` (skill id = plugin id).
    pub fn skills(&self) -> Vec<SkillInfo> {
        let mut out: Vec<SkillInfo> = NATIVE_SKILLS
            .iter()
            .map(|(id, desc)| SkillInfo {
                id: (*id).to_string(),
                name: (*id).to_string(),
                description: (*desc).to_string(),
                source: "native".into(),
                plugin_id: None,
                has_tools: true,
                has_handler: false,
            })
            .collect();
        for p in self.list() {
            if p.enabled {
                out.extend(self.skills_for_plugin(&p));
            }
        }
        out
    }

    /// The skills contributed by one plugin (subfolder + legacy top-level).
    fn skills_for_plugin(&self, p: &PluginInfo) -> Vec<SkillInfo> {
        let mut out = Vec::new();
        let pdir = self.plugin_dir(&p.id);
        // skills/<id>/ folders — the new layout: each folder is a skill with
        // skill.md (+ optional tools.json / handler.js).
        let skills_dir = pdir.join("skills");
        if let Ok(entries) = std::fs::read_dir(&skills_dir) {
            let mut ids: Vec<String> = entries
                .filter_map(|e| e.ok())
                .filter(|e| {
                    std::fs::metadata(e.path())
                        .map(|m| m.is_dir())
                        .unwrap_or(false)
                })
                .filter_map(|e| e.file_name().into_string().ok())
                .collect();
            ids.sort();
            for sid in ids {
                let sdir = skills_dir.join(&sid);
                if !sdir.join("skill.md").is_file() {
                    continue;
                }
                out.push(SkillInfo {
                    id: format!("{}/{}", p.id, sid),
                    name: sid.clone(),
                    description: p.description.clone(),
                    source: "plugin-skill".into(),
                    plugin_id: Some(p.id.clone()),
                    has_tools: sdir.join("tools.json").is_file(),
                    has_handler: sdir.join("handler.js").is_file(),
                });
            }
        }
        // Legacy: a single top-level skill.md — skill id = plugin id.
        if pdir.join("skill.md").is_file() {
            out.push(SkillInfo {
                id: p.id.clone(),
                name: p.name.clone(),
                description: p.description.clone(),
                source: "plugin".into(),
                plugin_id: Some(p.id.clone()),
                has_tools: false,
                has_handler: false,
            });
        }
        out
    }

    /// (id, description) pairs for the system prompt's skill index.
    pub fn skill_index(&self) -> Vec<(String, String)> {
        self.skills()
            .into_iter()
            .map(|s| (s.id, s.description))
            .collect()
    }

    /// Resolve a skill's full document: native skills from embedded assets,
    /// plugin skills from their skill.md (subfolder or legacy top-level).
    pub fn skill_doc(&self, skill_id: &str) -> Result<String, String> {
        if let Some((_, desc)) = NATIVE_SKILLS.iter().find(|(id, _)| *id == skill_id) {
            let _ = desc;
            let text = match skill_id {
                "git" => include_str!("../../assets/skills/git/skill.md"),
                "web" => include_str!("../../assets/skills/web/skill.md"),
                _ => unreachable!(),
            };
            return Ok(text.to_string());
        }
        if let Some((plugin_id, sid)) = skill_id.split_once('/') {
            let p = self
                .plugin_dir(plugin_id)
                .join("skills")
                .join(sid)
                .join("skill.md");
            if p.is_file() {
                return std::fs::read_to_string(&p).map_err(|e| e.to_string());
            }
            return Err(format!("skill not found: {skill_id}"));
        }
        self.skill_md(skill_id)
    }

    /// Look up a skill from its id (None when missing/disabled).
    pub fn skill(&self, skill_id: &str) -> Option<SkillInfo> {
        self.skills().into_iter().find(|s| s.id == skill_id)
    }

    /// Path to a plugin skill's sidecar handler.js, if it has one.
    pub fn skill_handler_path(&self, skill_id: &str) -> Option<PathBuf> {
        match skill_id.split_once('/') {
            Some((plugin_id, sid)) => {
                let p = self
                    .plugin_dir(plugin_id)
                    .join("skills")
                    .join(sid)
                    .join("handler.js");
                p.is_file().then_some(p)
            }
            None => None,
        }
    }

    /// Parse a plugin skill's tools.json into tool definitions. Empty when the
    /// skill declares none.
    pub fn skill_tools_json(
        &self,
        skill_id: &str,
    ) -> Result<Vec<crate::providers::types::ToolDefinition>, String> {
        let Some(skill) = self.skill(skill_id) else {
            return Ok(Vec::new());
        };
        if !skill.has_tools {
            return Ok(Vec::new());
        }
        let (plugin_id, sid) = skill_id
            .split_once('/')
            .ok_or("native skills have no tools.json")?;
        let p = self
            .plugin_dir(plugin_id)
            .join("skills")
            .join(sid)
            .join("tools.json");
        let text = std::fs::read_to_string(&p).map_err(|e| e.to_string())?;
        crate::tools::definitions::parse_plugin_tools(&text)
    }

    /// All plugins that have a valid manifest, in directory order.
    pub fn list(&self) -> Vec<PluginInfo> {
        let mut out = Vec::new();
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return out;
        };
        let mut ids: Vec<String> = entries
            .filter_map(|e| e.ok())
            .filter(|e| {
                std::fs::metadata(e.path())
                    .map(|m| m.is_dir())
                    .unwrap_or(false)
            })
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
                skills: Vec::new(), // filled below without recursion
                folder: dir.display().to_string(),
            });
        }
        // Second pass: fill each plugin's skill list (skills_for_plugin does
        // not recurse into list()).
        for p in out.iter_mut() {
            p.skills = self.skills_for_plugin(p);
        }
        out
    }

    /// Real folders behind symlinked plugin entries in the plugins dir
    /// (dev workflow: the plugin lives in an untracked folder, linked in
    /// manually with `ln -s`). The file watcher watches these explicitly
    /// because notify does not follow symlinked directories by default.
    pub fn symlink_targets(&self) -> Vec<PathBuf> {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        entries
            .filter_map(|e| e.ok())
            .filter(|e| {
                std::fs::symlink_metadata(e.path())
                    .map(|m| m.file_type().is_symlink())
                    .unwrap_or(false)
            })
            .filter_map(|e| std::fs::read_link(e.path()).ok())
            .collect()
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
        assert!(demo
            .contributes
            .slots
            .contains(&"chat-toolbar-extra".to_string()));
        let hidden = list.iter().find(|p| p.id == ".hidden").unwrap();
        assert!(!hidden.has_component);
    }

    #[test]
    fn lists_only_valid_manifests() {
        let (m, _dir) = mk_manager(vec![
            ("broken/manifest.json", "not json"),
            (
                "demo/manifest.json",
                r#"{"id":"demo","name":"Demo","version":"0.1"}"#,
            ),
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
    fn skill_index() {
        let (m, _dir) = mk_manager(vec![
            (
                "demo/manifest.json",
                r#"{"id":"demo","name":"Demo","version":"0.1","description":"A demo skill"}"#,
            ),
            ("demo/skill.md", "always be concise"),
        ]);
        let index = m.skill_index();
        // native git + web always present, plus the demo plugin's legacy skill
        assert_eq!(index.len(), 3);
        assert!(index.iter().any(|(id, _)| id == "git"));
        assert!(index.iter().any(|(id, _)| id == "web"));
        let demo = index.iter().find(|(id, _)| id == "demo").unwrap();
        assert_eq!(demo.1, "A demo skill");
    }

    #[test]
    fn discovers_subfolder_skills_with_tools_and_handler() {
        let (m, _dir) = mk_manager(vec![
            (
                "fe/manifest.json",
                r#"{"id":"fe","name":"Frontend","version":"1","description":"Frontend stack"}"#,
            ),
            ("fe/skills/frontend-dev/skill.md", "frontend skill doc"),
            (
                "fe/skills/frontend-dev/tools.json",
                r#"[{"name":"fe_status","description":"status","inputSchema":{"type":"object","properties":{}}}]"#,
            ),
            ("fe/skills/frontend-dev/handler.js", "console.log(1)"),
            ("fe/skills/a11y/skill.md", "a11y doc"),
        ]);
        let fe_skills = m
            .skills()
            .into_iter()
            .filter(|s| s.plugin_id.as_deref() == Some("fe"))
            .collect::<Vec<_>>();
        assert_eq!(fe_skills.len(), 2);
        let dev = fe_skills
            .iter()
            .find(|s| s.id == "fe/frontend-dev")
            .unwrap();
        assert_eq!(dev.source, "plugin-skill");
        assert!(dev.has_tools);
        assert!(dev.has_handler);
        let a11y = fe_skills.iter().find(|s| s.id == "fe/a11y").unwrap();
        assert!(!a11y.has_tools);
        assert!(!a11y.has_handler);
        // doc + handler path + tools resolve
        assert_eq!(
            m.skill_doc("fe/frontend-dev").unwrap(),
            "frontend skill doc"
        );
        assert!(m
            .skill_handler_path("fe/frontend-dev")
            .unwrap()
            .ends_with("handler.js"));
        assert_eq!(m.skill_tools_json("fe/frontend-dev").unwrap().len(), 1);
        assert_eq!(m.skill_tools_json("fe/a11y").unwrap().len(), 0);
        // unknown id → error
        assert!(m.skill_doc("fe/unknown").is_err());
    }

    #[test]
    fn native_skill_docs_resolve() {
        let (m, _dir) = mk_manager(vec![]);
        assert!(m.skill_doc("git").unwrap().contains("git_status"));
        assert!(m.skill_doc("web").unwrap().contains("web_search"));
        assert_eq!(m.skill("git").unwrap().source, "native");
        assert!(m.skill("git").unwrap().has_tools);
    }

    #[test]
    fn list_follows_symlinked_plugin_dirs() {
        let real = tempfile::tempdir().unwrap();
        std::fs::write(
            real.path().join("manifest.json"),
            r#"{"id":"linked","name":"Linked","version":"1"}"#,
        )
        .unwrap();
        let (m, _dir) = mk_manager(vec![(
            "real/manifest.json",
            r#"{"id":"real","name":"Real","version":"1"}"#,
        )]);
        let link = m.plugin_dir("linked");
        #[cfg(unix)]
        {
            if std::os::unix::fs::symlink(real.path(), &link).is_err() {
                return; // no symlink support in this environment — skip
            }
        }
        #[cfg(not(unix))]
        {
            std::fs::create_dir_all(&link).unwrap();
            std::fs::write(
                link.join("manifest.json"),
                r#"{"id":"linked","name":"Linked","version":"1"}"#,
            )
            .unwrap();
        }
        let list = m.list();
        assert!(
            list.iter().any(|p| p.id == "linked"),
            "symlinked plugin must be listed: {:?}",
            list.iter().map(|p| &p.id).collect::<Vec<_>>()
        );
        assert!(list.iter().any(|p| p.id == "real"));
    }
}
