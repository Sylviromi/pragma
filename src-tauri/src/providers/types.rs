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

/// Token usage captured from a provider call (SSE usage events when the API
/// provides them, else a best-effort chars/4 estimate). Serialized camelCase
/// to match the frontend's Usage type.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
}

impl Usage {
    pub fn total(&self) -> u64 {
        self.input_tokens + self.output_tokens
    }
}

/// Best-effort estimate when the API omits usage data (chars/4, cf. OpenAI).
pub fn estimate_usage(text: &str) -> u64 {
    (text.chars().count() / 4).max(1) as u64
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
    /// Optional per-model output-token ceiling (provider.json `maxTokens`).
    /// None = fall back to the provider's default max_tokens.
    #[serde(default)]
    pub max_tokens: Option<u32>,
    /// Optional reasoning effort for reasoning-capable OpenAI-compat models
    /// (provider.json `reasoningEffort`, e.g. "low" | "medium" | "high").
    /// None = don't send a reasoning_effort field.
    #[serde(default)]
    pub reasoning_effort: Option<String>,
    /// Whether the model supports extended thinking / reasoning. Comes from
    /// the capability catalog for known families, from /v1/models metadata
    /// where servers expose it, or explicitly from provider.json
    /// (`supportsThinking`). Drives the Anthropic `thinking` param, the picker
    /// badge, and the reasoning_effort behavior for OpenAI-compat models.
    #[serde(default)]
    pub supports_thinking: bool,
    /// Optional thinking budget in tokens (Anthropic `budget_tokens`). None =
    /// the harness default.
    #[serde(default)]
    pub thinking_budget: Option<u32>,
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
    /// A thinking block finished streaming (Anthropic `content_block_stop`,
    /// or the end of a DeepSeek reasoning-only segment). Lets the frontend
    /// seal the current live thought so the next reasoning chunk opens a new
    /// block — mirroring the ContentBlock structure of the persisted message.
    ThinkingEnd,
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
    /// The model asked the user a question and is paused waiting for an answer
    /// (`ask_question` tool). Frontend shows the "Needs your answer" block;
    /// `answer_question(question_id, …)` resumes the loop.
    #[serde(rename_all = "camelCase")]
    Question {
        question_id: String,
        text: String,
        options: Vec<String>,
    },
    Done,
    Error { message: String },
    /// User pressed Esc / Stop — the loop ended before the turn completed.
    Interrupted,
    #[serde(rename_all = "camelCase")]
    WorkingDirChanged { new_path: String },
}
