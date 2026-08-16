# Git skill

The git tools operate on the session working directory (or `repo_path` when a subproject repo is targeted):

- `git_status` — `git status --short`. Fast orientation: what changed, what's staged.
- `git_diff` — unstaged diff by default; pass `staged: true` for `--cached`; scope with `path`. Read it before staging — know exactly what you're committing.
- `git_log` — recent commit history (`limit`, default 10). Read it before changing staged files so commits follow the project's local convention.
- `git_add` — stage paths; `["."]` stages everything. Only stage what belongs in the commit (no build artifacts, no secrets).
- `git_commit` — create a commit with a message. Requires staged files (git_add first). Match the repo's message style (conventional commits, imperative, etc.).

## Rules

- `git_status` / `git_diff` / `git_log` are read-only — safe to call freely.
- **NEVER run `git_add` or `git_commit` unless the user explicitly asks.** Inspection is always fine; mutating history or the staging area is not.
- If the working directory isn't a git repo, say so and let the user decide — don't guess a repo path.
- Prefer `run_command` (core) for unusual git operations; use these tools for the standard status/diff/log/add/commit flow.
