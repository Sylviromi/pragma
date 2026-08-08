pub mod definitions;
pub mod file_ops;
pub mod shell;
pub mod git;
pub mod web;

use std::path::{Path, PathBuf};

pub struct ToolExecutor {
    pub working_dir: PathBuf,
}

impl ToolExecutor {
    pub fn new(working_dir: impl AsRef<Path>) -> Self {
        Self {
            working_dir: working_dir.as_ref().to_path_buf(),
        }
    }

    pub fn execute(&self, name: &str, input: &serde_json::Value) -> Result<String, String> {
        match name {
            // file ops
            "read_file" => file_ops::read_file(&self.working_dir, input),
            "write_file" => file_ops::write_file(&self.working_dir, input),
            "patch_file" => file_ops::patch_file(&self.working_dir, input),
            "create_directory" => file_ops::create_directory(&self.working_dir, input),
            "delete_file" => file_ops::delete_file(&self.working_dir, input),
            "move_file" => file_ops::move_file(&self.working_dir, input),
            "list_files" => file_ops::list_files(&self.working_dir, input),
            "search_files" => file_ops::search_files(&self.working_dir, input),
            // shell
            "run_command" => shell::run_command(&self.working_dir, input),
            // git
            "git_status" => git::git_status(&self.working_dir, input),
            "git_diff" => git::git_diff(&self.working_dir, input),
            "git_log" => git::git_log(&self.working_dir, input),
            "git_add" => git::git_add(&self.working_dir, input),
            "git_commit" => git::git_commit(&self.working_dir, input),
            // web
            "web_search" => web::web_search(input),
            "fetch_url" => web::fetch_url(input),
            other => Err(format!("unknown tool: {other}")),
        }
    }
}