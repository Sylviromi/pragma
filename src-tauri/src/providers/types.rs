use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text { text: String },
    ToolUse { id: String, name: String, input: serde_json::Value },
    ToolResult {
        tool_use_id: String,
        content: String,
        #[serde(default)]
        is_error: bool,
    },
    /// Extended-thinking block (Anthropic). Must be passed back verbatim in subsequent requests.
    Thinking { thinking: String, signature: String },
    /// Reasoning block (DeepSeek / OpenAI-compat). Stored separately and reconstructed as
    /// `reasoning_content` top-level field when replaying history.
    Reasoning { content: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub context_window: Option<u32>,
    #[serde(default)]
    pub is_free: bool,
    #[serde(default)]
    pub is_vision: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderSource {
    Builtin,
    #[serde(rename_all = "camelCase")]
    Plugin { plugin_id: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderInfo {
    pub id: String,
    pub name: String,
    pub source: ProviderSource,
    pub models: Vec<ModelInfo>,
}

// Events emitted over the Channel during an agentic loop.
// The outer rename_all = "snake_case" controls tag values (token, tool_call, etc.).
// Per-variant rename_all = "camelCase" controls field names sent to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum AgentEvent {
    Token { content: String },
    /// Streaming reasoning/thinking tokens (DeepSeek `reasoning_content`,
    /// Anthropic `thinking_delta`) — surfaced live in the UI so the user can
    /// see the model thinking, like other harnesses.
    Reasoning { content: String },
    #[serde(rename_all = "camelCase")]
    ToolCall {
        run_id: String,
        tool_use_id: String,
        name: String,
        input: serde_json::Value,
    },
    #[serde(rename_all = "camelCase")]
    ToolResult {
        tool_use_id: String,
        output: String,
        is_error: bool,
    },
    Done,
    Error { message: String },
    /// User pressed Esc / Stop — the loop ended before the turn completed.
    Interrupted,
    #[serde(rename_all = "camelCase")]
    WorkingDirChanged { new_path: String },
}
