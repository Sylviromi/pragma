//! Known-model catalog.
//!
//! `/v1/models` endpoints (OpenAI-compat) and even Anthropic's `/v1/models`
//! rarely expose every useful attribute. This catalog fills the gaps with
//! well-known context windows / vision support per model family, so the model
//! picker can show correct context numbers and capabilities regardless of
//! whether the upstream API reports them.

use super::types::ModelInfo;

/// Best-effort context window for a model id, from the known families below.
/// `None` means we don't recognise the id — the UI simply omits the badge.
pub fn context_for(id: &str) -> Option<u32> {
    let id = id.to_ascii_lowercase();
    // Anthropic — all current Claude models share a 200k context.
    if id.starts_with("claude-") {
        return Some(200_000);
    }
    // OpenAI
    if id.starts_with("gpt-4.1") || id.starts_with("gpt-5") {
        return Some(1_000_000); // gpt-4.1 / gpt-5 family
    }
    if id.starts_with("o1") || id.starts_with("o3") || id.starts_with("o4") {
        return Some(200_000);
    }
    if id.starts_with("gpt-") {
        return Some(131_072); // gpt-4o, gpt-4.5, gpt-oss, gpt-image …
    }
    // DeepSeek
    if id.starts_with("deepseek-") {
        return Some(128_000); // deepseek-chat (V3.x) and deepseek-reasoner
    }
    // Google
    if id.starts_with("gemini-") {
        return Some(1_000_000);
    }
    // Meta Llama
    if id.starts_with("llama-3") {
        return Some(131_072); // 3.x family (3.1 / 3.2 / 3.3)
    }
    if id.starts_with("llama-4") {
        return Some(1_000_000);
    }
    // Qwen
    if id.starts_with("qwen2.5") || id.starts_with("qwen3") || id.starts_with("qwen2-vl") {
        return Some(131_072);
    }
    // Mistral / Mixtral
    if id.starts_with("mixtral") || id.starts_with("mistral-nemo") || id.starts_with("mistral-large") {
        return Some(131_072);
    }
    if id.starts_with("mistral-7b") || id.starts_with("mistral-small") {
        return Some(32_768);
    }
    // Cohere Command-R
    if id.starts_with("command-r") {
        return Some(131_072);
    }
    // Microsoft Phi
    if id.starts_with("phi-") {
        return Some(131_072);
    }
    None
}

/// Whether the model family supports vision input. Covers the common
/// multimodal families; anything with "vision" / "vl" in the id counts too.
pub fn vision_for(id: &str) -> bool {
    let id = id.to_ascii_lowercase();
    if id.contains("vision") || id.contains("-vl") || id.contains("-vlm") {
        return true;
    }
    if id.starts_with("claude-") {
        return true;
    }
    if id.starts_with("gpt-4o") || id.starts_with("gpt-4.1") || id.starts_with("gpt-5") {
        return true;
    }
    if id.starts_with("gemini-") {
        return true;
    }
    false
}

/// Whether a model family supports extended thinking / reasoning. Fallback
/// used when the upstream doesn't declare it (most /v1/models endpoints
/// don't). Conservative: only families the harness can actually turn thinking
/// on for are marked true.
/// - Anthropic: Opus 4.x, Sonnet 4.5/4.6, Claude 3.7 Sonnet — NOT Haiku.
/// - OpenAI-compat reasoning models: gpt-5 / o-series, deepseek-reasoner,
///   Qwen3, GLM-z1 (reasoning_effort / thinking modes).
pub fn thinking_for(id: &str) -> bool {
    let id = id.to_ascii_lowercase();
    if id.contains("haiku") {
        return false;
    }
    if id.starts_with("claude-") {
        return id.contains("opus-4")
            || id.contains("sonnet-4-5")
            || id.contains("sonnet-4-6")
            || id.contains("claude-3-7");
    }
    if id.starts_with("gpt-5") || id.starts_with("o1") || id.starts_with("o3") || id.starts_with("o4") {
        return true;
    }
    if id.contains("deepseek-reasoner") || id.contains("deepseek-r1") {
        return true;
    }
    if id.starts_with("qwen3") || id.contains("glm-z1") {
        return true;
    }
    false
}

/// Whether a model can also take *graduated* thinking levels, not just
/// on/off. Anthropic: Sonnet 4.5/4.6 + Opus 4.x accept `thinking_level`
/// (low/medium/high); Claude 3.7 and everything before only takes a budget.
/// OpenAI-compat reasoning families accept `reasoning_effort`.
pub fn thinking_levels_for(id: &str) -> bool {
    let id = id.to_ascii_lowercase();
    if id.contains("haiku") {
        return false;
    }
    if id.starts_with("claude-") {
        return id.contains("opus-4") || id.contains("sonnet-4-5") || id.contains("sonnet-4-6");
    }
    if id.starts_with("gpt-5") || id.starts_with("o1") || id.starts_with("o3") || id.starts_with("o4") {
        return true;
    }
    if id.contains("deepseek-reasoner") || id.contains("deepseek-r1") {
        return true;
    }
    if id.starts_with("qwen3") || id.contains("glm-z1") {
        return true;
    }
    false
}

/// Merge upstream-reported attributes with catalog knowledge.
/// Upstream values win when present; the catalog fills the gaps.
pub fn enrich(
    id: &str,
    name: String,
    api_context: Option<u32>,
    api_vision: Option<bool>,
    api_free: Option<bool>,
    api_thinking: Option<bool>,
    api_thinking_levels: Option<bool>,
) -> ModelInfo {
    ModelInfo {
        id: id.to_string(),
        name: name.trim().to_string().replace('\n', " "),
        context_window: api_context.or_else(|| context_for(id)),
        is_free: api_free.unwrap_or(false),
        is_vision: api_vision.unwrap_or_else(|| vision_for(id)),
        max_tokens: None,
        reasoning_effort: None,
        supports_thinking: api_thinking.unwrap_or_else(|| thinking_for(id)),
        supports_thinking_levels: api_thinking_levels.unwrap_or_else(|| thinking_levels_for(id)),
        thinking_budget: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_families_get_contexts() {
        assert_eq!(context_for("claude-sonnet-4-5-20250929"), Some(200_000));
        assert_eq!(context_for("claude-opus-4-1-20250805"), Some(200_000));
        assert_eq!(context_for("deepseek-chat"), Some(128_000));
        assert_eq!(context_for("deepseek-reasoner"), Some(128_000));
        assert_eq!(context_for("gpt-4.1-mini"), Some(1_000_000));
        assert_eq!(context_for("gpt-4o"), Some(131_072));
        assert_eq!(context_for("gemini-2.5-flash"), Some(1_000_000));
        assert_eq!(context_for("llama-3.1-70b-instruct"), Some(131_072));
        assert_eq!(context_for("totally-unknown-model"), None);
    }

    #[test]
    fn vision_flags() {
        assert!(vision_for("claude-sonnet-4-5-20250929"));
        assert!(vision_for("gpt-4o"));
        assert!(vision_for("gemini-2.5-pro"));
        assert!(vision_for("qwen2.5-vl-7b"));
        assert!(!vision_for("deepseek-chat"));
        assert!(!vision_for("llama-3.1-70b-instruct"));
    }

    #[test]
    fn thinking_flags() {
        // Anthropic capable families (haiku never).
        for m in [
            "claude-sonnet-4-5-20250929",
            "claude-opus-4-1-20250805",
            "claude-sonnet-4-6-20251022",
            "claude-3-7-sonnet-20250219",
        ] {
            assert!(thinking_for(m), "{m} should support thinking");
        }
        assert!(!thinking_for("claude-haiku-4-5-20251001"));
        assert!(!thinking_for("claude-3-5-haiku-20241022"));
        assert!(!thinking_for("claude-sonnet-4-20250514")); // base Sonnet 4
        // OpenAI-compat reasoning models.
        assert!(thinking_for("gpt-5"));
        assert!(thinking_for("o3-mini"));
        assert!(thinking_for("deepseek-reasoner"));
        assert!(thinking_for("qwen3-coder"));
        assert!(thinking_for("glm-z1-9b"));
        // Non-reasoning models.
        assert!(!thinking_for("deepseek-chat"));
        assert!(!thinking_for("gpt-4o"));
        assert!(!thinking_for("totally-unknown-model"));
    }

    #[test]
    fn thinking_level_flags() {
        // Anthropic families that accept thinking_level (Sonnet 4.5/4.6, Opus 4.x).
        for m in ["claude-sonnet-4-5-20250929", "claude-opus-4-1-20250805", "claude-sonnet-4-6-20251022"] {
            assert!(thinking_levels_for(m), "{m} should support thinking levels");
        }
        // Thinking yes, but levels no: Claude 3.7 (budget only).
        assert!(!thinking_levels_for("claude-3-7-sonnet-20250219"));
        assert!(!thinking_levels_for("claude-haiku-4-5-20251001"));
        // OpenAI-compat reasoning families accept reasoning_effort.
        assert!(thinking_levels_for("gpt-5"));
        assert!(thinking_levels_for("o3-mini"));
        assert!(thinking_levels_for("deepseek-reasoner"));
        assert!(!thinking_levels_for("deepseek-chat"));
        assert!(!thinking_levels_for("gpt-4o"));
    }

    #[test]
    fn upstream_values_win_over_catalog() {
        let m = enrich(
            "claude-sonnet-4-5-20250929",
            "Claude Sonnet 4.5".into(),
            Some(1_000_000), // API says 1M — trust it
            Some(false),
            None,
            Some(false), // server says no thinking — trust it
            Some(false), // server says no levels — trust it
        );
        assert_eq!(m.context_window, Some(1_000_000));
        assert!(!m.is_vision);
        assert!(!m.supports_thinking);
    }

    #[test]
    fn catalog_fills_gaps() {
        let m = enrich("deepseek-chat", "deepseek-chat".into(), None, None, None, None, None);
        assert_eq!(m.context_window, Some(128_000));
        assert!(!m.is_vision);
        assert!(!m.supports_thinking);
        // Unknown model + explicit upstream thinking flag wins.
        let m = enrich(
            "my-custom-reasoner",
            "Custom Reasoner".into(),
            None,
            None,
            None,
            Some(true),
            Some(true),
        );
        assert!(m.supports_thinking);
    }
}
