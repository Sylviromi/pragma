use crate::providers::types::ToolDefinition;

pub fn standard_tools() -> Vec<ToolDefinition> {
    vec![
        // ── File operations ──────────────────────────────────────────────
        ToolDefinition {
            name: "read_file".into(),
            description: "Read the contents of a file. Path is relative to the session working directory.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File path relative to working directory" }
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
            name: "patch_file".into(),
            description: "Apply a unified diff to a file. Use this for targeted edits without rewriting the whole file. The diff must be exact: `--- a/<path>` and `+++ b/<path>` headers, then one or more `@@ -start,count +start,count @@` hunks whose line numbers, counts, and space-prefixed context lines match the current file verbatim. There is no fuzzy matching — any mismatch fails the whole patch. Read the file first to pin exact positions. Use write_file for full rewrites.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File path relative to working directory" },
                    "diff": { "type": "string", "description": "Unified diff string: `--- a/<path>` / `+++ b/<path>` headers then `@@ -start,count +start,count @@` hunks. Prefix removed lines with '-', added lines with '+', context with a space. Line numbers, counts, and context must match the file exactly — no fuzzy matching." }
                },
                "required": ["path", "diff"]
            }),
        },
        ToolDefinition {
            name: "create_directory".into(),
            description: "Create a directory and any missing parent directories.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Directory path relative to working directory" }
                },
                "required": ["path"]
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
            name: "move_file".into(),
            description: "Move or rename a file or directory.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "src": { "type": "string", "description": "Source path relative to working directory" },
                    "dest": { "type": "string", "description": "Destination path relative to working directory" }
                },
                "required": ["src", "dest"]
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
            description: "Search for a regex pattern in files. Returns matching lines with file path and line number.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "pattern": { "type": "string", "description": "Regex pattern to search for" },
                    "path": { "type": "string", "description": "Directory to search in, defaults to '.'" },
                    "glob": { "type": "string", "description": "File glob filter, e.g. '*.rs'" }
                },
                "required": ["pattern"]
            }),
        },
        // ── Shell ─────────────────────────────────────────────────────────
        ToolDefinition {
            name: "run_command".into(),
            description: "Run a shell command in the session working directory. Returns combined stdout + stderr. Timeout: 60s.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": { "type": "string", "description": "Shell command to run (via sh -c)" }
                },
                "required": ["command"]
            }),
        },
        // ── Git ───────────────────────────────────────────────────────────
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
        // ── Skills ────────────────────────────────────────────────────────
        ToolDefinition {
            name: "load_skill".into(),
            description: "Load the full content of a plugin skill by its id. Use the skill index in the system prompt to decide which skill is relevant, then call this to read it.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "skill_id": { "type": "string", "description": "The plugin id whose skill.md to load" }
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
                    "path": { "type": "string", "description": "Absolute path or path relative to current working directory" }
                },
                "required": ["path"]
            }),
        },
        // ── Web ───────────────────────────────────────────────────────────
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
    ]
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
        assert_eq!(standard_tools().len(), 18);
    }
}