pub mod manifest;
pub mod provider;
pub mod watcher;

use manifest::{load_manifest_from_dir, Contributes, PluginManifest};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
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
    /// Who made it (Settings → Plugins shows it; the registry uses it for attribution).
    pub author: String,
    /// Publisher identity — "pragma" for first-party plugins (badged in the UI).
    pub publisher: String,
    /// Source repo URL, when declared in the manifest.
    pub repository: String,
    /// Project homepage URL, when declared in the manifest.
    pub homepage: String,
    /// Manifest `updateUrl` — polled by the future update system (unused today).
    pub update_url: String,
    pub enabled: bool,
    pub contributes: Contributes,
    pub has_component: bool,
    pub has_skill: bool,
    pub skills: Vec<SkillInfo>,
    pub folder: String,
    /// Recorded install source (`npm:…` / local path) — None for manually
    /// dropped-in folders. Used by the marketplace to mark installed plugins.
    pub source: Option<String>,
}

/// One installed plugin's update status, as reported by `check_updates`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginUpdateInfo {
    pub id: String,
    /// Recorded install source (`git:…` / `npm:…` / path) — None for folders
    /// dropped in manually.
    pub source: Option<String>,
    /// Installed version (from the local manifest).
    pub current: String,
    /// Latest version known from the source — None when it can't be
    /// determined (local paths are live symlinks; git repos with no semver
    /// tags; unreadable remotes).
    pub latest: Option<String>,
    pub update_available: bool,
    /// Version-pinned source (`git:…@tag` / `npm:pkg@ver`) — updates are
    /// deliberately skipped, matching pi's pinned-ref semantics.
    pub pinned: bool,
}

pub struct PluginManager {
    pub dir: PathBuf,
    disabled: RwLock<Vec<String>>,
    /// Plugin ids that have been loaded before (persisted in settings.json as
    /// `seen_plugins`). A plugin whose id is NOT in this set has never been
    /// seen — it starts DISABLED (see `register_new_plugins`).
    seen: RwLock<HashSet<String>>,
    /// Where each installed plugin came from: id → source spec (`git:…`,
    /// `npm:…`, local path). Persisted to `plugin_sources.json` next to
    /// settings — the origin record that a future update checker polls.
    sources: RwLock<HashMap<String, String>>,
}

impl PluginManager {
    /// `seen` = plugin ids already known to the app (persisted). Existing
    /// on-disk plugins are automatically treated as seen (upgrade migration:
    /// they keep their current enabled state); only plugins whose folders
    /// appear AFTER this point start disabled.
    pub fn new(dir: PathBuf, disabled: Vec<String>, seen: Vec<String>) -> Self {
        let sources = Self::load_sources(&dir);
        let m = Self {
            dir,
            disabled: RwLock::new(disabled),
            seen: RwLock::new(seen.into_iter().collect()),
            sources: RwLock::new(sources),
        };
        m.seed_existing();
        m
    }

    /// Path of the installed-plugin-sources record (`plugin_sources.json` in
    /// the app config dir — the plugins dir's parent, so it never sits inside
    /// the watched plugins tree).
    fn sources_path(&self) -> PathBuf {
        self.dir
            .parent()
            .map(|p| p.join("plugin_sources.json"))
            .unwrap_or_else(|| self.dir.join("plugin_sources.json"))
    }

    fn load_sources(dir: &Path) -> HashMap<String, String> {
        let path = dir
            .parent()
            .map(|p| p.join("plugin_sources.json"))
            .unwrap_or_else(|| dir.join("plugin_sources.json"));
        std::fs::read_to_string(path)
            .ok()
            .and_then(|t| serde_json::from_str::<HashMap<String, String>>(&t).ok())
            .unwrap_or_default()
    }

    fn persist_sources(&self) {
        let path = self.sources_path();
        let json = serde_json::to_string_pretty(&*self.sources.read().unwrap()).unwrap_or_default();
        let _ = std::fs::write(path, json);
    }

    /// The recorded source spec for a plugin (None for manually dropped-in
    /// folders / pre-install).
    pub fn source_of(&self, id: &str) -> Option<String> {
        self.sources.read().unwrap().get(id).cloned()
    }

    /// Mark everything currently on disk as seen WITHOUT touching enabled
    /// state — the upgrade migration. A brand-new install has nothing to
    /// seed; a pre-existing plugins dir keeps its plugins enabled as before.
    fn seed_existing(&self) {
        for id in self.scan_ids() {
            self.seen.write().unwrap().insert(id);
        }
    }

    /// First-time plugins start DISABLED. Every discoverable plugin id that
    /// is not in `seen` is recorded and added to the disabled list — the user
    /// explicitly enables it in Settings → Plugins. Returns true when
    /// anything changed (the caller persists settings). Idempotent.
    pub fn register_new_plugins(&self) -> bool {
        let mut changed = false;
        let mut seen = self.seen.write().unwrap();
        let mut disabled = self.disabled.write().unwrap();
        for id in self.scan_ids() {
            if seen.insert(id.clone()) {
                if !disabled.contains(&id) {
                    disabled.push(id);
                }
                changed = true;
            }
        }
        changed
    }

    /// Plugin ids that have been seen at least once (for settings.json).
    pub fn seen_ids(&self) -> Vec<String> {
        let mut v: Vec<String> = self.seen.read().unwrap().iter().cloned().collect();
        v.sort();
        v
    }

    /// Discoverable plugin ids: folder names with a valid manifest whose
    /// declared id matches the folder name (mirrors `list()`'s filtering).
    fn scan_ids(&self) -> Vec<String> {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        let mut ids: Vec<String> = entries
            .filter_map(|e| e.ok())
            .filter(|e| {
                std::fs::metadata(e.path())
                    .map(|m| m.is_dir())
                    .unwrap_or(false)
            })
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|id| {
                load_manifest_from_dir(&self.dir.join(id))
                    .map(|m| m.id == *id)
                    .unwrap_or(false)
            })
            .collect();
        ids.sort();
        ids
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
                author: m.author.clone(),
                publisher: m.publisher.clone(),
                repository: m.repository.clone(),
                homepage: m.homepage.clone(),
                update_url: m.update_url.clone(),
                enabled: self.is_enabled(&m.id),
                contributes: m.contributes.clone(),
                has_component: dir.join("component.js").is_file(),
                has_skill: dir.join("skill.md").is_file(),
                skills: Vec::new(), // filled below without recursion
                folder: dir.display().to_string(),
                source: self.sources.read().unwrap().get(&m.id).cloned(),
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

    // ── Install / uninstall ──────────────────────────────────────────────────

    /// Install a plugin from a **source spec**:
    /// - `npm:name[@version]` — installed with a real `npm install` into the
    ///   shared npm prefix, so plugin sidecars can `require` dependencies.
    ///   This is the distribution layer (one plugin = one npm package).
    /// - a local **folder** path — symlinked, so edits hot-reload live (the
    ///   dev workflow).
    ///
    /// The package is validated before anything touches the plugins dir, then
    /// `plugins/<id>` becomes a symlink to it (npm packages stay in place so
    /// their `node_modules` resolves from the sidecar's cwd). Every install
    /// records its source (`plugin_sources.json`) and enables the plugin.
    pub fn install_from_source(&self, spec: &str) -> Result<PluginInfo, String> {
        let source = PluginSource::parse(spec)?;
        std::fs::create_dir_all(&self.dir)
            .map_err(|e| format!("cannot create plugins dir: {e}"))?;
        let staging = self.dir.join(format!(".install-{}", uuid::Uuid::new_v4()));

        let prepared = (|| -> Result<PathBuf, String> {
            match &source {
                PluginSource::Npm { package } => install_npm(&self.npm_dir(), package),
                PluginSource::Local(path) => {
                    let p = Path::new(path);
                    if !p.is_dir() {
                        return Err(format!("local source is not a folder: {path}"));
                    }
                    let root = locate_plugin_root(p)?;
                    symlink_dir(&root, &staging)?;
                    Ok(staging.clone())
                }
            }
        })();
        let pkg = match prepared {
            Ok(p) => p,
            Err(e) => {
                let _ = std::fs::remove_dir_all(&staging);
                return Err(e);
            }
        };
        self.install_package_dir(&pkg, Some(spec))
    }

    /// Shared npm prefix (config_dir/npm) — npm packages install here with
    /// their node_modules, mirroring pi's `~/.pi/agent/npm/`.
    fn npm_dir(&self) -> PathBuf {
        self.dir
            .parent()
            .map(|p| p.join("npm"))
            .unwrap_or_else(|| self.dir.join("npm"))
    }

    /// Validate a prepared package folder and register it: locate the plugin
    /// root (manifest at the package root or in one subfolder), validate, then
    /// symlink `plugins/<id>` → the root (canonicalized, so npm packages keep
    /// their real location and local dev folders point at their true path).
    fn install_package_dir(&self, pkg: &Path, source: Option<&str>) -> Result<PluginInfo, String> {
        std::fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
        let found = locate_plugin_root(pkg).map_err(|e| format!("invalid plugin package: {e}"))?;
        let root = std::fs::canonicalize(&found).map_err(|e| e.to_string())?;
        let manifest =
            load_manifest_from_dir(&root).map_err(|e| format!("invalid plugin package: {e}"))?;
        let id = manifest.id.clone();
        let target = self.plugin_dir(&id);

        // Swap: remove any existing entry, then symlink in the new one.
        if let Ok(meta) = std::fs::symlink_metadata(&target) {
            if meta.file_type().is_symlink() {
                std::fs::remove_file(&target).map_err(|e| e.to_string())?;
            } else if meta.is_dir() {
                std::fs::remove_dir_all(&target).map_err(|e| e.to_string())?;
            } else {
                std::fs::remove_file(&target).map_err(|e| e.to_string())?;
            }
        }
        symlink_dir(&root, &target)?;

        // Consent: record the source, mark seen, enable.
        if let Some(spec) = source {
            self.sources
                .write()
                .unwrap()
                .insert(id.clone(), spec.to_string());
            self.persist_sources();
        }
        self.seen.write().unwrap().insert(id.clone());
        self.set_disabled(&id, false);

        self.list()
            .into_iter()
            .find(|p| p.id == id)
            .ok_or_else(|| format!("plugin '{id}' installed but not discoverable"))
    }

    /// Remove a plugin. Dev-symlinked / npm plugins remove just the link (and
    /// the npm package from the shared prefix); the source record is dropped.
    pub fn uninstall(&self, id: &str) -> Result<(), String> {
        let dir = self.plugin_dir(id);
        let meta =
            std::fs::symlink_metadata(&dir).map_err(|e| format!("plugin '{id}' not found: {e}"))?;
        if meta.file_type().is_symlink() {
            std::fs::remove_file(&dir).map_err(|e| format!("cannot remove symlink: {e}"))?;
        } else if meta.is_dir() {
            std::fs::remove_dir_all(&dir)
                .map_err(|e| format!("cannot remove plugin folder: {e}"))?;
        } else {
            std::fs::remove_file(&dir).map_err(|e| format!("cannot remove plugin: {e}"))?;
        }
        // npm-installed plugins: also drop the package from the shared prefix.
        if let Some(spec) = self.sources.read().unwrap().get(id).cloned() {
            if let Ok(PluginSource::Npm { package }) = PluginSource::parse(&spec) {
                let (name, _) = npm_name_version(&package);
                let pkg_dir = self.npm_dir().join("node_modules").join(name);
                if pkg_dir.exists() {
                    let _ = std::fs::remove_dir_all(pkg_dir);
                }
            }
        }
        if self.sources.write().unwrap().remove(id).is_some() {
            self.persist_sources();
        }
        Ok(())
    }

    // ── Updates ──────────────────────────────────────────────────────────────

    /// Poll every installed plugin's recorded source for a newer version
    /// (npm registry `latest` via `npm view`). Local-path plugins are live
    /// symlinks (always current); version-pinned sources (`npm:pkg@ver`)
    /// deliberately don't move (pi semantics). No recorded source → skipped.
    pub fn check_updates(&self) -> Vec<PluginUpdateInfo> {
        let sources = self.sources.read().unwrap().clone();
        let mut out = Vec::new();
        for (id, spec) in sources {
            let current = self
                .manifest(&id)
                .map(|m| m.version.clone())
                .unwrap_or_default();
            let mut info = PluginUpdateInfo {
                id: id.clone(),
                source: Some(spec.clone()),
                current,
                latest: None,
                update_available: false,
                pinned: false,
            };
            match PluginSource::parse(&spec) {
                Ok(PluginSource::Local(_)) => {} // live symlink — always current
                Ok(PluginSource::Npm { package }) => {
                    let (name, version) = npm_name_version(&package);
                    if version.is_some() {
                        info.pinned = true;
                    } else if let Ok(Some(latest)) = npm_latest_version(&name) {
                        info.latest = Some(latest.clone());
                        info.update_available = compare_versions(&latest, &info.current) > 0;
                    }
                }
                Err(_) => {} // unparsable recorded source — nothing to poll
            }
            out.push(info);
        }
        out.sort_by(|a, b| a.id.cmp(&b.id));
        out
    }

    /// Update one installed plugin to the latest version from its recorded
    /// source — re-installs through the exact same path as a fresh install
    /// (`npm install` fetches `latest`). Pinned and local-path sources are
    /// left alone.
    pub fn update_plugin(&self, id: &str) -> Result<PluginInfo, String> {
        let spec = self.sources.read().unwrap().get(id).cloned().ok_or_else(|| {
            format!(
                "plugin '{id}' has no recorded source — dropped-in folders update by editing the folder"
            )
        })?;
        let source = PluginSource::parse(&spec)?;
        match &source {
            PluginSource::Local(_) => {
                return Err(
                    "local-path plugins are live symlinks — edit the source folder".into(),
                )
            }
            PluginSource::Npm { package } if npm_name_version(package).1.is_some() => {
                return Err(
                    "this plugin is version-pinned (npm:pkg@ver) — drop the @version to receive updates"
                        .into(),
                )
            }
            _ => {}
        }
        self.install_from_source(&spec)
    }
}

/// A plugin source spec — where an installed plugin came from. npm packages
/// are the distribution layer; local folders are the dev workflow. Git and
/// zip sources were removed with the move to npm-only.
#[derive(Debug, Clone, PartialEq)]
pub enum PluginSource {
    /// `npm:name` / `npm:name@version` / `npm:@scope/name`
    Npm { package: String },
    /// Local folder path (symlinked — the dev workflow).
    Local(String),
}

impl PluginSource {
    /// Parse a source spec string:
    /// - `npm:name` / `npm:name@version` / `npm:@scope/name`
    /// - `file:path` or a plain local folder path that exists on disk
    pub fn parse(spec: &str) -> Result<Self, String> {
        let s = spec.trim();
        if let Some(pkg) = s.strip_prefix("npm:") {
            if pkg.is_empty() {
                return Err("npm source needs a package name (npm:my-plugin)".into());
            }
            return Ok(PluginSource::Npm {
                package: pkg.to_string(),
            });
        }
        if let Some(path) = s.strip_prefix("file:") {
            if path.is_empty() {
                return Err("file source needs a path".into());
            }
            return Ok(PluginSource::Local(path.to_string()));
        }
        if Path::new(s).exists() {
            return Ok(PluginSource::Local(s.to_string()));
        }
        if s.starts_with("git:")
            || s.starts_with("http://")
            || s.starts_with("https://")
            || s.starts_with("ssh://")
            || s.starts_with("git://")
        {
            return Err(
                "git sources are no longer supported — plugins are distributed via npm (npm:<package>) or installed from a local folder"
                    .into(),
            );
        }
        Err(format!(
            "unknown plugin source: {spec:?} — use npm:<package>[@version] or a local folder path"
        ))
    }
}

/// Find the plugin root inside a prepared package: `manifest.json` at the
/// root or inside at most a few levels down (npm packages may nest the
/// plugin). Requires EXACTLY one candidate — a package must contain one
/// plugin.
fn locate_plugin_root(pkg: &Path) -> Result<PathBuf, String> {
    let mut found = Vec::new();
    walk_for_manifest(pkg, 0, &mut found);
    match found.len() {
        1 => Ok(found.remove(0)),
        0 => Err(
            "no manifest.json found — not a Pragma plugin (expected manifest.json at the package root or in one subfolder)".into(),
        ),
        _ => Err("multiple plugins in one package — publish each plugin separately".into()),
    }
}

/// Depth-bounded search for a folder containing `manifest.json`. Stops at the
/// first manifest found (a plugin folder may contain nested `skills/` folders
/// that look similar — never descend INTO a plugin).
fn walk_for_manifest(dir: &Path, depth: usize, found: &mut Vec<PathBuf>) {
    if depth > 3 {
        return;
    }
    if dir.join("manifest.json").is_file() {
        found.push(dir.to_path_buf());
        return;
    }
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            let p = e.path();
            if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                walk_for_manifest(&p, depth + 1, found);
            }
        }
    }
}

/// `npm` resolves on Windows as `npm.cmd` — `Command::new` doesn't pick up
/// .cmd shims, so use the platform-exact executable name.
#[cfg(windows)]
const NPM_BIN: &str = "npm.cmd";
#[cfg(not(windows))]
const NPM_BIN: &str = "npm";

/// Install an npm package with a real `npm install` into the shared npm prefix
/// (config_dir/npm) — `node_modules` lands next to the package, so plugin
/// sidecars can `require` dependencies. Returns the installed package folder.
fn install_npm(npm_dir: &Path, package: &str) -> Result<PathBuf, String> {
    std::fs::create_dir_all(npm_dir).map_err(|e| e.to_string())?;
    // A minimal package.json keeps npm from mutating a shared manifest; the
    // prefix holds every npm-installed plugin (like pi's ~/.pi/agent/npm/).
    let manifest_path = npm_dir.join("package.json");
    if !manifest_path.exists() {
        std::fs::write(&manifest_path, "{}\n").map_err(|e| e.to_string())?;
    }
    let out = std::process::Command::new(NPM_BIN)
        .args(["install", "--no-audit", "--no-fund", "--prefix"])
        .arg(npm_dir)
        .arg(package)
        .output()
        .map_err(|e| format!("npm is not available on PATH: {e}"))?;
    if !out.status.success() {
        let msg = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(if msg.is_empty() {
            format!("npm install of {package} failed")
        } else {
            msg
        });
    }
    Ok(npm_dir
        .join("node_modules")
        .join(resolve_package_name(package)?))
}

/// The package folder name for an npm spec: local path specs carry their own
/// package.json (read its `name`); plain specs split `name[@version]`.
fn resolve_package_name(spec: &str) -> Result<String, String> {
    let path = spec.strip_prefix("file:").unwrap_or(spec);
    if Path::new(path).is_dir() {
        let text = std::fs::read_to_string(Path::new(path).join("package.json"))
            .map_err(|e| format!("cannot read {path}/package.json: {e}"))?;
        let v: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| format!("invalid package.json: {e}"))?;
        return v["name"]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| format!("{path}/package.json has no name"));
    }
    let (name, _) = npm_name_version(spec);
    Ok(name)
}

/// Split an npm package spec into `(name, version)`. `@scope/pkg` keeps its
/// scope; `pkg@1.2.3` / `@scope/pkg@1.2.3` split the trailing version (npm
/// versions are numeric semver — a non-numeric suffix is not a version).
fn npm_name_version(package: &str) -> (String, Option<String>) {
    match package.rfind('@') {
        Some(0) => (package.to_string(), None), // @scope/pkg — no version
        Some(i) => {
            let (name, ver) = (&package[..i], &package[i + 1..]);
            if !ver.is_empty() && ver.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                (name.to_string(), Some(ver.to_string()))
            } else {
                (package.to_string(), None)
            }
        }
        None => (package.to_string(), None),
    }
}

/// Latest published version of an npm package via `npm view <name> version`.
fn npm_latest_version(name: &str) -> Result<Option<String>, String> {
    let out = std::process::Command::new(NPM_BIN)
        .args(["view", name, "version"])
        .output()
        .map_err(|e| format!("npm is not available on PATH: {e}"))?;
    if !out.status.success() {
        let msg = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(if msg.is_empty() {
            format!("npm view {name} version failed")
        } else {
            msg
        });
    }
    let v = String::from_utf8_lossy(&out.stdout).trim().to_string();
    Ok((!v.is_empty()).then_some(v))
}

/// Compare two version strings ("1.2.3", "v1.2.3", "1.2.3-beta.1").
/// Dotted numeric segments compare numerically (padded to equal length); a
/// pre-release suffix sorts below the same version without one. Returns <0,
/// 0, or >0.
fn compare_versions(a: &str, b: &str) -> i32 {
    fn parse(v: &str) -> (Vec<u64>, Option<String>) {
        let v = v.trim_start_matches('v').trim();
        match v.split_once('-') {
            Some((core, pre)) => (segments(core), Some(pre.to_string())),
            None => (segments(v), None),
        }
    }
    fn segments(core: &str) -> Vec<u64> {
        core.split('.')
            .map(|s| s.trim().parse::<u64>().unwrap_or(0))
            .collect()
    }
    let (mut ac, ap) = parse(a);
    let (mut bc, bp) = parse(b);
    let n = ac.len().max(bc.len());
    ac.resize(n, 0);
    bc.resize(n, 0);
    for (x, y) in ac.iter().zip(bc.iter()) {
        if x != y {
            return if x > y { 1 } else { -1 };
        }
    }
    match (&ap, &bp) {
        (None, None) => 0,
        (None, Some(_)) => 1, // release > prerelease
        (Some(_), None) => -1,
        (Some(x), Some(y)) => x.cmp(y) as i32,
    }
}

/// Create a directory symlink (Unix); copy fallback on Windows (junctions
/// aren't exposed by std).
#[cfg(unix)]
fn symlink_dir(target: &Path, link: &Path) -> Result<(), String> {
    std::os::unix::fs::symlink(target, link).map_err(|e| e.to_string())
}

#[cfg(not(unix))]
fn symlink_dir(target: &Path, link: &Path) -> Result<(), String> {
    copy_dir(target, link)
}

#[cfg(not(unix))]
fn copy_dir(src: &Path, dst: &Path) -> Result<(), String> {
    fn cp(from: &Path, to: &Path) -> Result<(), String> {
        std::fs::create_dir_all(to).map_err(|e| e.to_string())?;
        for e in std::fs::read_dir(from).map_err(|e| e.to_string())? {
            let e = e.map_err(|e| e.to_string())?;
            let f = e.path();
            let t = to.join(e.file_name());
            if e.file_type().map_err(|e| e.to_string())?.is_dir() {
                cp(&f, &t)?;
            } else {
                std::fs::copy(&f, &t).map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }
    cp(src, dst)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk_manager(files: Vec<(&str, &str)>) -> (PluginManager, tempfile::TempDir) {
        // The plugins dir is NESTED inside the tempdir (tempdir/plugins) so the
        // plugin_sources.json record lands in the tempdir, not /tmp — parallel
        // tests would otherwise race on a shared /tmp/plugin_sources.json.
        let dir = tempfile::tempdir().unwrap();
        let plugins = dir.path().join("plugins");
        for (name, content) in files {
            let path = plugins.join(name);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(path, content).unwrap();
        }
        (PluginManager::new(plugins, vec![], vec![]), dir)
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

    #[test]
    fn first_time_plugins_start_disabled() {
        let dir = tempfile::tempdir().unwrap();
        let plugins = dir.path().join("plugins");
        let m = PluginManager::new(plugins.clone(), vec![], vec![]);
        // A plugin folder appears AFTER startup (installed while running).
        std::fs::create_dir_all(plugins.join("brand-new")).unwrap();
        std::fs::write(
            plugins.join("brand-new/manifest.json"),
            r#"{"id":"brand-new","name":"BN","version":"1"}"#,
        )
        .unwrap();
        assert!(
            m.register_new_plugins(),
            "first-time plugin registers as new"
        );
        let p = m.list().into_iter().find(|p| p.id == "brand-new").unwrap();
        assert!(!p.enabled, "first-time plugin starts DISABLED");
        assert!(m.seen_ids().contains(&"brand-new".to_string()));
        assert!(m.disabled_ids().contains(&"brand-new".to_string()));
        // Idempotent: a second sweep changes nothing.
        assert!(!m.register_new_plugins());
    }

    #[test]
    fn existing_plugins_keep_their_state_after_upgrade() {
        // Upgrade run: empty seen list, plugins already on disk. Construction
        // seeds them as seen, so they keep their current enabled state.
        let (m, _dir) = mk_manager(vec![(
            "demo/manifest.json",
            r#"{"id":"demo","name":"Demo","version":"1"}"#,
        )]);
        assert!(
            !m.register_new_plugins(),
            "on-disk plugins were seeded — nothing new to disable"
        );
        assert!(m.list()[0].enabled, "existing plugin stays enabled");
        assert!(m.seen_ids().contains(&"demo".to_string()));
    }

    #[test]
    fn explicitly_enabled_plugin_is_not_re_disabled() {
        // User enabled a first-time-disabled plugin; a later sweep must not
        // flip it back off.
        let dir = tempfile::tempdir().unwrap();
        let plugins = dir.path().join("plugins");
        let m = PluginManager::new(plugins.clone(), vec![], vec![]);
        std::fs::create_dir_all(plugins.join("p1")).unwrap();
        std::fs::write(
            plugins.join("p1/manifest.json"),
            r#"{"id":"p1","name":"P1","version":"1"}"#,
        )
        .unwrap();
        assert!(m.register_new_plugins());
        assert!(!m.list()[0].enabled);
        // User enables it (removes from disabled). seen still contains p1.
        m.set_disabled("p1", false);
        assert!(
            !m.register_new_plugins(),
            "no NEW plugins — p1 already seen"
        );
        assert!(
            m.list()[0].enabled,
            "explicitly enabled plugin stays enabled"
        );
    }

    #[test]
    fn disabled_list_from_settings_is_respected() {
        // A plugin that was already disabled before the change stays disabled.
        let (m, _dir) = mk_manager(vec![(
            "demo/manifest.json",
            r#"{"id":"demo","name":"Demo","version":"1"}"#,
        )]);
        let m = PluginManager::new(m.dir.clone(), vec!["demo".to_string()], vec![]);
        assert!(!m.list()[0].enabled);
        assert!(!m.register_new_plugins());
        assert!(!m.list()[0].enabled);
    }

    // ── install_from_zip / uninstall ────────────────────────────────────────

    #[test]
    fn uninstall_removes_folder_and_keeps_seen_state() {
        let (m, _dir) = mk_manager(vec![(
            "demo/manifest.json",
            r#"{"id":"demo","name":"Demo","version":"1"}"#,
        )]);
        assert!(m.plugin_dir("demo").exists());
        m.uninstall("demo").unwrap();
        assert!(!m.plugin_dir("demo").exists());
        assert!(m.list().is_empty());
        // Re-installing the same id is a normal install, not a first-time drop.
        m.set_disabled("demo", true);
        m.seen.write().unwrap().insert("demo".into());
        assert!(!m.register_new_plugins());
        assert!(m.list().is_empty());
    }
    #[test]
    fn uninstall_of_missing_plugin_errors() {
        let (m, _dir) = mk_manager(vec![]);
        assert!(m.uninstall("nope").is_err());
    }
    #[test]
    fn parses_npm_sources() {
        match PluginSource::parse("npm:my-plugin").unwrap() {
            PluginSource::Npm { package } => assert_eq!(package, "my-plugin"),
            other => panic!("expected Npm, got {other:?}"),
        }
        match PluginSource::parse("npm:@scope/pkg@1.2.3").unwrap() {
            PluginSource::Npm { package } => assert_eq!(package, "@scope/pkg@1.2.3"),
            other => panic!("expected Npm, got {other:?}"),
        }
    }
    #[test]
    fn rejects_unknown_sources() {
        assert!(PluginSource::parse("not-a-real-source").is_err());
        assert!(PluginSource::parse("npm:").is_err());
    }
    #[test]
    fn installs_local_folder_as_symlink() {
        // Local folder source: the plugin dir must be a symlink to the source,
        // so edits to the source folder hot-reload live.
        let source = tempfile::tempdir().unwrap();
        std::fs::write(
            source.path().join("manifest.json"),
            r#"{"id":"local","name":"Local","version":"1","author":"Me"}"#,
        )
        .unwrap();
        std::fs::write(
            source.path().join("component.js"),
            "export default class {}",
        )
        .unwrap();

        let (m, _dir) = mk_manager(vec![]);
        let spec = source.path().display().to_string();
        let info = m.install_from_source(&spec).unwrap();
        assert_eq!(info.id, "local");
        assert!(info.enabled);
        assert!(info.has_component);

        let installed = m.plugin_dir("local");
        #[cfg(unix)]
        assert!(
            std::fs::symlink_metadata(&installed)
                .map(|md| md.file_type().is_symlink())
                .unwrap_or(false),
            "local installs must be symlinks (dev workflow)"
        );
        // Source recorded. On Unix the plugin dir is a live symlink, so edits
        // to the source folder show through it; on Windows (no symlink
        // privilege) it's a copy — the live-link assertions are Unix-only.
        assert_eq!(m.source_of("local").as_deref(), Some(spec.as_str()));
        #[cfg(unix)]
        {
            std::fs::write(source.path().join("new-file.txt"), "hi").unwrap();
            assert!(installed.join("new-file.txt").is_file());
        }
        // Uninstall removes the plugin dir (the link on Unix, the copy on
        // Windows), never the source folder.
        m.uninstall("local").unwrap();
        assert!(!installed.exists());
        assert!(source.path().join("manifest.json").is_file());
        assert_eq!(m.source_of("local"), None);
    }
    #[test]
    fn installs_local_folder_inside_subfolder() {
        // A local folder whose plugin lives one level down (repo-style layout).
        let source = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(source.path().join("plugins/my-plugin")).unwrap();
        std::fs::write(
            source.path().join("plugins/my-plugin/manifest.json"),
            r#"{"id":"my-plugin","name":"My","version":"1"}"#,
        )
        .unwrap();
        let (m, _dir) = mk_manager(vec![]);
        m.install_from_source(&source.path().display().to_string())
            .unwrap();
        assert!(m.plugin_dir("my-plugin").join("manifest.json").is_file());
    }
    #[test]
    fn compares_versions() {
        assert_eq!(compare_versions("1.0.0", "1.0.0"), 0);
        assert!(compare_versions("1.2.0", "1.1.9") > 0);
        assert!(compare_versions("1.0.0", "1.0.1") < 0);
        assert!(compare_versions("v2.0.0", "v1.99.99") > 0);
        assert!(
            compare_versions("1.0.0", "1.0.0-beta.1") > 0,
            "release > prerelease"
        );
        assert!(compare_versions("1.0.0-alpha", "1.0.0-beta") < 0);
        assert!(
            compare_versions("10.0", "9.0.0") > 0,
            "short versions pad with zeros"
        );
    }
    #[test]
    fn splits_npm_name_and_version() {
        assert_eq!(npm_name_version("my-plugin"), ("my-plugin".into(), None));
        assert_eq!(
            npm_name_version("my-plugin@1.2.3"),
            ("my-plugin".into(), Some("1.2.3".into()))
        );
        assert_eq!(npm_name_version("@scope/pkg"), ("@scope/pkg".into(), None));
        assert_eq!(
            npm_name_version("@scope/pkg@1.2.3"),
            ("@scope/pkg".into(), Some("1.2.3".into()))
        );
    }

    #[test]
    fn sources_survive_manager_reload() {
        // A fresh manager over the same dir sees the recorded source.
        let pkg = tempfile::tempdir().unwrap();
        std::fs::write(
            pkg.path().join("package.json"),
            r#"{"name":"pragma-plugin-reload","version":"1.0.0"}"#,
        )
        .unwrap();
        std::fs::write(
            pkg.path().join("manifest.json"),
            r#"{"id":"reload","name":"Reload","version":"1.0.0"}"#,
        )
        .unwrap();
        let (m, _dir) = mk_manager(vec![]);
        let installed = install_npm(&m.npm_dir(), &pkg.path().display().to_string()).unwrap();
        m.install_package_dir(&installed, Some("npm:pragma-plugin-reload"))
            .unwrap();
        let m2 = PluginManager::new(m.dir.clone(), vec![], vec![]);
        assert_eq!(
            m2.source_of("reload").as_deref(),
            Some("npm:pragma-plugin-reload")
        );
    }

    // ── npm-only install / update ────────────────────────────────────────────

    /// Build a local folder shaped like a published plugin package.
    fn make_pkg(version: &str) -> tempfile::TempDir {
        let pkg = tempfile::tempdir().unwrap();
        std::fs::write(
            pkg.path().join("package.json"),
            format!(
                r#"{{"name":"pragma-plugin-npmtest","version":"{version}","keywords":["pragma-plugin"]}}"#
            ),
        )
        .unwrap();
        std::fs::write(
            pkg.path().join("manifest.json"),
            format!(r#"{{"id":"npmtest","name":"Npm Test","version":"{version}"}}"#),
        )
        .unwrap();
        pkg
    }

    #[test]
    fn installs_npm_package_with_real_npm_install() {
        // npm install from a local folder (offline) — the package lands in the
        // shared npm prefix with node_modules, and plugins/<id> becomes a
        // symlink to it.
        let pkg = make_pkg("1.0.0");
        let (m, _dir) = mk_manager(vec![]);
        let installed = install_npm(&m.npm_dir(), &pkg.path().display().to_string()).unwrap();
        assert!(installed.join("manifest.json").is_file());
        assert!(
            installed.join("node_modules").exists() || !pkg.path().join("node_modules").exists()
        );

        let info = m
            .install_package_dir(&installed, Some("npm:pragma-plugin-npmtest"))
            .unwrap();
        assert_eq!(info.id, "npmtest");
        assert!(info.enabled);
        assert_eq!(info.source.as_deref(), Some("npm:pragma-plugin-npmtest"));
        assert_eq!(m.list()[0].version, "1.0.0");

        let link = m.plugin_dir("npmtest");
        assert!(link.join("manifest.json").is_file());
        #[cfg(unix)]
        assert!(std::fs::symlink_metadata(&link)
            .map(|md| md.file_type().is_symlink())
            .unwrap_or(false));
        // Uninstall drops the symlink AND the package from the npm prefix.
        m.uninstall("npmtest").unwrap();
        assert!(!link.exists());
        assert!(!installed.exists());
        assert_eq!(m.source_of("npmtest"), None);
    }

    #[test]
    fn reinstalling_npm_package_replaces_the_previous_version() {
        let (m, _dir) = mk_manager(vec![]);
        // v1
        let pkg = make_pkg("1.0.0");
        let installed = install_npm(&m.npm_dir(), &pkg.path().display().to_string()).unwrap();
        m.install_package_dir(&installed, Some("npm:pragma-plugin-npmtest"))
            .unwrap();
        assert_eq!(m.list()[0].version, "1.0.0");
        // bump to v2 and reinstall — same id, newer version, single entry
        let pkg2 = make_pkg("2.0.0");
        let installed2 = install_npm(&m.npm_dir(), &pkg2.path().display().to_string()).unwrap();
        m.install_package_dir(&installed2, Some("npm:pragma-plugin-npmtest"))
            .unwrap();
        assert_eq!(m.list().len(), 1, "reinstall must not duplicate");
        assert_eq!(m.list()[0].version, "2.0.0");
        assert!(m.list()[0].enabled);
    }

    #[test]
    fn pinned_and_local_sources_are_handled_by_updates() {
        let (m, _dir) = mk_manager(vec![(
            "demo/manifest.json",
            r#"{"id":"demo","name":"Demo","version":"1.0.0"}"#,
        )]);
        // Pinned npm source: reported as pinned, update refuses.
        m.sources
            .write()
            .unwrap()
            .insert("demo".into(), "npm:demo@1.0.0".into());
        m.persist_sources();
        let u = m.check_updates();
        assert_eq!(u.len(), 1);
        assert!(u[0].pinned);
        assert!(!u[0].update_available);
        assert!(m.update_plugin("demo").is_err());
        // Local path source: live symlink, never an update target.
        m.sources
            .write()
            .unwrap()
            .insert("demo".into(), "/some/dev/folder".into());
        m.persist_sources();
        let u = m.check_updates();
        assert!(!u[0].pinned);
        assert!(!u[0].update_available);
        assert!(m.update_plugin("demo").is_err());
    }

    #[test]
    fn parse_rejects_git_sources_with_guidance() {
        let err = PluginSource::parse("git:https://github.com/user/repo").unwrap_err();
        assert!(err.contains("npm"), "error should point at npm: {err}");
        assert!(PluginSource::parse("https://github.com/user/repo").is_err());
        // npm + local still parse
        assert!(matches!(
            PluginSource::parse("npm:pragma-plugin-x").unwrap(),
            PluginSource::Npm { .. }
        ));
    }
}
