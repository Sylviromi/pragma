use crate::providers::types::ToolDefinition;

/// Core tools every run gets, regardless of which skills are loaded. Loading a
/// skill ADDS its tools (see `tools_for_skill`) — the model never sees git or
/// web tools unless the matching skill is loaded for the session.
pub fn core_tools() -> Vec<ToolDefinition> {
    vec![
        // ── File operations ──────────────────────────────────────────────
        ToolDefinition {
            name: "read_file".into(),
            description: "Read a file's contents. Lines are 1-indexed and prefixed like `cat -n`, so line numbers match your editor and stack traces. A read is bounded by three ceilings: at most 2000 lines, at most 128 KB of the file (a larger file returns only its head), and at most 2000 characters per line (a minified bundle is clamped, never returned whole). When a read is cut short by a ceiling the result names the recovery, e.g. `offset=2001 to read the rest` — follow it to page through. A file that exceeds the byte budget has no resume offset (re-reading returns the same head) — use search_files to locate content beyond it. Empty/missing files, past-EOF offsets and special devices (/dev/zero etc.) return a one-line fact instead of an error. If the exact path is missing, visually-identical spellings are retried (narrow no-break space, NFD/NFC, curly quotes) and the result notes the corrected name — use it in future calls; when no repair matches, the result suggests close names (did you mean…). Binary files (PNG/PDF/ZIP/etc.) return a one-line mime fact with a pdftotext hint for PDFs. Use offset (1-based start line) / limit (max lines) to read a targeted range; for large files prefer locating lines with search_files first.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File path relative to working directory. Aliases: filePath, file_path, absolutePath, target, filename." },
                    "offset": { "type": ["integer", "string"], "description": "1-based start line (default 1). Pass an integer or a numeric string like \"2000\"; non-numeric strings and fractions are rejected." },
                    "limit": { "type": ["integer", "string"], "description": "Maximum lines to return (default: all, up to the 2000-line window)." }
                },
                "required": ["path"]
            }),
        },
        ToolDefinition {
            name: "write_file".into(),
            description: "Write content to a file, creating it and any parent directories if needed. Overwrites existing files.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "content": { "type": "string" }
                },
                "required": ["path", "content"]
            }),
        },
        ToolDefinition {
            name: "edit_file".into(),
            description: "Replace a line range in a file. Hash-anchored: pass the `hash` from the latest read_file header — the edit is REJECTED if the file changed since that read (stale-anchor protection), so a partial read of a big file is fine; you only re-read what drifted. `content` is ONLY the new lines (the range deletes; never restate unchanged or removed lines). Omit `end_line` to replace a single line; `end_line = start_line - 1` inserts before `start_line`.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File path relative to working directory" },
                    "hash": { "type": "string", "description": "REQUIRED — the whole-file hash from the latest read_file header. Covers the whole file (stable across partial reads), so it catches drift anywhere, not just at the edited lines." },
                    "start_line": { "type": "integer", "description": "First line to replace (1-based, from read_file output)" },
                    "end_line": { "type": "integer", "description": "Last line to replace (1-based, inclusive). OMIT to replace exactly one line (start_line). Set to start_line - 1 to insert before start_line." },
                    "content": { "type": "string", "description": "Replacement content — the final lines only. Replaces the entire start_line..end_line range; empty string deletes the range." }
                },
                "required": ["path", "hash", "start_line", "content"]
            }),
        },
        ToolDefinition {
            name: "delete_file".into(),
            description: "Delete a file. Does not delete directories. Fails if the path does not exist.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File path relative to working directory" }
                },
                "required": ["path"]
            }),
        },
        ToolDefinition {
            name: "list_files".into(),
            description: "List files and directories. Defaults to immediate children only (depth 1). Use depth > 1 for recursive listing. Directories are suffixed with '/'.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Directory path relative to working directory. Defaults to '.'." },
                    "depth": { "type": "integer", "description": "How many levels deep to recurse. Default 1 (immediate children only). Use 0 for unlimited." },
                    "pattern": { "type": "string", "description": "Optional glob pattern to filter results, e.g. '**/*.rs'" }
                }
            }),
        },
        ToolDefinition {
            name: "search_files".into(),
            description: "Search for a regex pattern in files. Returns matching lines with file path and line number. Capped at 50 matches by default (set max_results to raise it) — when more exist, a note reports the total so you can narrow the pattern. Patterns that match every line (.*, ^, …) are rejected with guidance.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "pattern": { "type": "string", "description": "Regex pattern to search for" },
                    "path": { "type": "string", "description": "Directory to search in, defaults to '.'" },
                    "glob": { "type": "string", "description": "File glob filter, e.g. '*.rs'" },
                    "max_results": { "type": "integer", "description": "Maximum matches to return. Default 50." }
                },
                "required": ["pattern"]
            }),
        },
        // ── Shell ─────────────────────────────────────────────────────────
        ToolDefinition {
            name: "run_command".into(),
            description: "Run a shell command in the session working directory. Returns combined stdout + stderr. Killed after 60s by default (set timeout_secs to override). Output is capped at 96000 chars by default (set max_output_chars to override).".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": { "type": "string", "description": "Shell command to run (via sh -c)" },
                    "timeout_secs": { "type": "integer", "description": "Timeout in seconds before the command is killed. Default 60." },
                    "max_output_chars": { "type": "integer", "description": "Maximum result length in characters before truncation. Default 96000." }
                },
                "required": ["command"]
            }),
        },
        // ── Skills ────────────────────────────────────────────────────────
        ToolDefinition {
            name: "load_skill".into(),
            description: "Load a skill by its id and make it active. Reading the full content gives you that skill's working knowledge, AND any tools declared by the skill are added to your available tools for the rest of this session. Use the skill index in the system prompt to decide which skill is relevant, then call this to read it and gain its tools.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "skill_id": { "type": "string", "description": "The id of the skill to load (from the skill index in the system prompt)" }
                },
                "required": ["skill_id"]
            }),
        },
        // ── Session ───────────────────────────────────────────────────────
        ToolDefinition {
            name: "change_directory".into(),
            description: "Change the session working directory. All subsequent file and shell operations will use the new path. The path must exist.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Absolute path or relative to the current working directory" }
                },
                "required": ["path"]
            }),
        },
        // ── Questions ─────────────────────────────────────────────────────
        ToolDefinition {
            name: "ask_question".into(),
            description: "Ask the user a question and pause until they answer (they get an app notification when the window is in the background). Use for genuinely blocking decisions — a branch in the plan that changes what you do next, or missing information only the user can provide (e.g. which folder to work in). Do NOT use for routine confirmation of work already requested, or for anything you can decide yourself by reading more context. Provide 2-4 short options, OR omit options when the answer is open-ended — the user then answers with free text.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "text": { "type": "string", "description": "The question to ask, phrased as a concrete decision" },
                    "options": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Optional 2-4 short answer options. The user will see a rendered 'Needs your answer' block and pick one. When omitted, the user answers with free text — use this for open questions like 'which folder?'."
                    },
                    "timeout_secs": { "type": "integer", "description": "Seconds to wait for an answer before continuing with your best judgment. Default 600." }
                },
                "required": ["text"]
            }),
        },
    ]
}

/// Tool sets contributed by skills, keyed by skill id. Native skills are
/// dispatched by the Rust ToolExecutor; plugin skills with a handler execute
/// via their sidecar process instead (their tools are declared in the
/// plugin's tools.json — this list only covers the built-in skills).
pub fn skill_tools() -> Vec<(&'static str, Vec<ToolDefinition>)> {
    vec![
        (
            "git",
            vec![
                // ── Git ───────────────────────────────────────────────────
                ToolDefinition {
                    name: "git_status".into(),
                    description: "Return `git status --short` output for the working directory.".into(),
                    input_schema: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "repo_path": { "type": "string", "description": "Path to git repo root. Defaults to session working directory." }
                        }
                    }),
                },
                ToolDefinition {
                    name: "git_diff".into(),
                    description: "Return git diff output. Shows unstaged changes by default.".into(),
                    input_schema: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "repo_path": { "type": "string" },
                            "staged": { "type": "boolean", "description": "If true, show staged changes (--cached). Default false." },
                            "path": { "type": "string", "description": "Limit diff to a specific file or directory." }
                        }
                    }),
                },
                ToolDefinition {
                    name: "git_log".into(),
                    description: "Return recent git commit log.".into(),
                    input_schema: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "repo_path": { "type": "string" },
                            "limit": { "type": "integer", "description": "Number of commits to show. Default 10." }
                        }
                    }),
                },
                ToolDefinition {
                    name: "git_add".into(),
                    description: "Stage files for commit with `git add`.".into(),
                    input_schema: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "paths": {
                                "type": "array",
                                "items": { "type": "string" },
                                "description": "List of file paths to stage. Use ['.'] to stage everything."
                            },
                            "repo_path": { "type": "string" }
                        },
                        "required": ["paths"]
                    }),
                },
                ToolDefinition {
                    name: "git_commit".into(),
                    description: "Create a git commit with the given message. Staged files must be added first with git_add.".into(),
                    input_schema: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "message": { "type": "string", "description": "Commit message" },
                            "repo_path": { "type": "string" }
                        },
                        "required": ["message"]
                    }),
                },
            ],
        ),
        (
            "web",
            vec![
                // ── Web ───────────────────────────────────────────────────
                ToolDefinition {
                    name: "web_search".into(),
                    description: "Search the web using DuckDuckGo. Returns titles, URLs, and snippets. No API key required.".into(),
                    input_schema: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "query": { "type": "string", "description": "Search query" },
                            "num_results": { "type": "integer", "description": "Number of results to return. Default 5, max 10." }
                        },
                        "required": ["query"]
                    }),
                },
                ToolDefinition {
                    name: "fetch_url".into(),
                    description: "Fetch a URL and return its text content (HTML tags stripped). Useful for reading documentation, articles, or search result pages.".into(),
                    input_schema: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "url": { "type": "string", "description": "URL to fetch" },
                            "max_chars": { "type": "integer", "description": "Truncate output to this many characters. Default 8000." }
                        },
                        "required": ["url"]
                    }),
                },
            ],
        ),
    ]
}

/// All tools in one flat list (core first, then skill tools). Kept for
/// back-compat and tests — the agent loop builds its set from `core_tools()` +
/// `tools_for_skill()` instead.
#[allow(dead_code)]
pub fn standard_tools() -> Vec<ToolDefinition> {
    let mut all = core_tools();
    for (_, tools) in skill_tools() {
        all.extend(tools);
    }
    all
}

/// The tools contributed by one skill (empty for unknown/untooled skills).
pub fn tools_for_skill(skill_id: &str) -> Vec<ToolDefinition> {
    skill_tools()
        .into_iter()
        .find(|(id, _)| *id == skill_id)
        .map(|(_, tools)| tools)
        .unwrap_or_default()
}

/// Parse a plugin skill's tools.json into tool definitions. Format:
/// a JSON array of `{ "name", "description", "inputSchema": <JSON Schema> }`.
pub fn parse_plugin_tools(text: &str) -> Result<Vec<ToolDefinition>, String> {
    let v: serde_json::Value =
        serde_json::from_str(text).map_err(|e| format!("invalid tools.json: {e}"))?;
    let arr = v
        .as_array()
        .ok_or("tools.json must be a JSON array of tool definitions")?;
    let mut out = Vec::new();
    for t in arr {
        let name = t
            .get("name")
            .and_then(|x| x.as_str())
            .ok_or("tool entry missing string \"name\"")?
            .to_string();
        if name.is_empty() {
            return Err("tool entry has empty name".into());
        }
        let description = t
            .get("description")
            .and_then(|x| x.as_str())
            .ok_or_else(|| format!("tool '{name}' missing string \"description\""))?
            .to_string();
        let input_schema = t.get("inputSchema").cloned().unwrap_or_else(|| {
            serde_json::json!({ "type": "object", "properties": {} })
        });
        if input_schema.get("type").is_none() {
            return Err(format!("tool '{name}' inputSchema must have a \"type\""));
        }
        out.push(ToolDefinition {
            name,
            description,
            input_schema,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_tools_have_required_fields() {
        for tool in standard_tools() {
            assert!(!tool.name.is_empty(), "tool has no name");
            assert!(!tool.description.is_empty(), "tool {} has no description", tool.name);
            assert!(tool.input_schema.get("type").is_some(), "tool {} schema missing type", tool.name);
        }
    }

    #[test]
    fn test_expected_tool_count() {
        // Ensures no tools are accidentally dropped when editing
        assert_eq!(core_tools().len(), 10, "core: file ops + shell + load_skill + change_directory + ask_question");
        let skill_sets = skill_tools();
        assert_eq!(skill_sets.len(), 2, "git + web");
        let total: usize = skill_sets.iter().map(|(_, t)| t.len()).sum();
        assert_eq!(total, 7, "git(5) + web(2)");
        assert_eq!(standard_tools().len(), 17);
    }

    #[test]
    fn test_tools_for_skill_lookup() {
        let git = tools_for_skill("git");
        assert_eq!(git.len(), 5);
        assert!(git.iter().any(|t| t.name == "git_status"));
        assert!(git.iter().all(|t| t.name.starts_with("git_")));
        let web = tools_for_skill("web");
        assert_eq!(web.len(), 2);
        assert_eq!(tools_for_skill("nope").len(), 0);
        // Core tools must never leak into a skill's set.
        assert!(!git.iter().any(|t| t.name == "read_file"));
    }

    #[test]
    fn test_parse_plugin_tools() {
        let text = r#"[
            { "name": "frontend_serve_start", "description": "Start the dev server", "inputSchema": { "type": "object", "properties": { "port": { "type": "integer" } } } },
            { "name": "frontend_serve_status", "description": "Is the server up?" }
        ]"#;
        let tools = parse_plugin_tools(text).unwrap();
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0].name, "frontend_serve_start");
        assert_eq!(tools[0].input_schema["properties"]["port"]["type"], "integer");
        // missing schema defaults to an empty object schema
        assert_eq!(tools[1].input_schema["type"], "object");
        // errors: not an array / missing name / missing description
        assert!(parse_plugin_tools("not json").is_err());
        assert!(parse_plugin_tools(r#"{"a":1}"#).is_err());
        assert!(parse_plugin_tools(r#"[{"description":"x"}]"#).is_err());
        assert!(parse_plugin_tools(r#"[{"name":"x"}]"#).is_err());
    }
}