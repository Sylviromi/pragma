use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
        #[serde(default)]
        is_error: bool,
    },
    /// Extended-thinking block (Anthropic). Must be passed back verbatim in subsequent requests.
    Thinking {
        thinking: String,
        signature: String,
    },
    /// Reasoning block (DeepSeek / OpenAI-compat). Stored separately and reconstructed as
    /// `reasoning_content` top-level field when replaying history.
    Reasoning {
        content: String,
    },
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

/// User preference for how much thinking/reasoning a model may do. Not all
/// values apply to all models — the backend maps them per provider and
/// silently ignores what the model can't do:
/// - `off`:    never enable thinking, even on capable models (cost control)
/// - `auto`:   model default — Anthropic sends no `thinking_level`, OpenAI-compat
///   sends the configured `reasoning_effort` (provider.json)
/// - `low` / `medium` / `high`: explicit thinking level — Anthropic
///   `thinking_level` ({low,medium,high}), OpenAI-compat
///   `reasoning_effort`, only for models that support levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThinkingPref {
    Off,
    Auto,
    Low,
    Medium,
    High,
}

impl ThinkingPref {
    /// Whether thinking may be enabled at all (anything but `Off`).
    pub fn enabled(&self) -> bool {
        !matches!(self, ThinkingPref::Off)
    }

    /// Explicit level for OpenAI-compat `reasoning_effort`: `None` for
    /// off/auto (auto falls back to the configured effort).
    pub fn effort(&self) -> Option<&'static str> {
        match self {
            ThinkingPref::Low => Some("low"),
            ThinkingPref::Medium => Some("medium"),
            ThinkingPref::High => Some("high"),
            ThinkingPref::Off | ThinkingPref::Auto => None,
        }
    }

    /// Explicit level for Anthropic `thinking_level`: same as `effort()`
    /// (Anthropic accepts low/medium/high).
    pub fn level(&self) -> Option<&'static str> {
        self.effort()
    }
}

/// Best-effort estimate when the API omits usage data (chars/4, cf. OpenAI).
pub fn estimate_usage(text: &str) -> u64 {
    (text.chars().count() / 4).max(1) as u64
}

/// A provider call failure, classified so the agentic loop can react:
/// rate-limited errors (HTTP 429/402/529 or a body that says so) trigger the
/// harness's API-key rotation; everything else surfaces as a plain error.
#[derive(Debug, Clone)]
pub enum CallError {
    RateLimited(String),
    Other(String),
}

impl CallError {
    pub fn message(&self) -> &str {
        match self {
            CallError::RateLimited(m) | CallError::Other(m) => m,
        }
    }

    /// Classify a non-success HTTP status (+ body) as rate-limited or not.
    pub fn classify(status: u16, message: String) -> Self {
        let rate_coded = matches!(status, 429 | 402 | 529);
        let rate_mentioned = {
            let lower = message.to_lowercase();
            [
                "rate limit",
                "rate_limit",
                "too many requests",
                "quota",
                "exceeded",
            ]
            .iter()
            .any(|needle| lower.contains(needle))
        };
        if rate_coded || rate_mentioned {
            CallError::RateLimited(message)
        } else {
            CallError::Other(message)
        }
    }
}

impl std::fmt::Display for CallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message())
    }
}

impl std::error::Error for CallError {}

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
    /// Whether the model also supports *graduated* thinking levels (Anthropic
    /// `thinking_level` on Sonnet 4.5+/Opus 4.x; `reasoning_effort` on
    /// OpenAI-compat reasoning families). False for e.g. Claude 3.7 which only
    /// takes a budget.
    #[serde(default)]
    pub supports_thinking_levels: bool,
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
    Plugin {
        plugin_id: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderInfo {
    pub id: String,
    pub name: String,
    pub source: ProviderSource,
    pub models: Vec<ModelInfo>,
    /// "api-key" | "none" — how the provider authenticates. The model picker
    /// hides api-key providers the user hasn't configured a key for.
    pub auth: String,
    /// Whether the user can actually use this provider right now: api-key
    /// providers need at least one configured key; "none" providers are
    /// always usable. Enriched by the list_providers command (the registry
    /// itself doesn't know about settings).
    pub has_key: bool,
}

// Agent events emitted over the app event bus (`agent:{session_id}`) during
// an agentic loop — tag = snake_case, fields = camelCase.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum AgentEvent {
    Token {
        content: String,
    },
    /// Streaming reasoning/thinking tokens (DeepSeek `reasoning_content`,
    /// Anthropic `thinking_delta`) — surfaced live in the UI so the user can
    /// see the model thinking, like other harnesses.
    Reasoning {
        content: String,
    },
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
    Error {
        message: String,
    },
    /// User pressed Esc / Stop — the loop ended before the turn completed.
    Interrupted,
    /// Harness verification phase (loop-done): started a sub-phase.
    #[serde(rename_all = "camelCase")]
    VerificationStarted {
        /// "checks" | "review"
        phase: String,
    },
    /// Harness verification phase finished (before Done). `passed` false means
    /// the report was either fed back for a fix round (rounds < maxRounds) or
    /// surfaced to the user (rounds exhausted). `cancelled` is set when the
    /// user stopped it (Esc) — distinct from a failure. `tokens`/`durationMs`
    /// power the bubble's meta line; `checksLabel` names what ran.
    #[serde(rename_all = "camelCase")]
    VerificationResult {
        passed: bool,
        cancelled: bool,
        summary: String,
        findings: Vec<String>,
        rounds: u32,
        tokens: u64,
        duration_ms: u64,
        checks_label: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    WorkingDirChanged {
        new_path: String,
    },
}
