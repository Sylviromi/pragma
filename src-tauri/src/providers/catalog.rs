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

/// Merge upstream-reported attributes with catalog knowledge.
/// Upstream values win when present; the catalog fills the gaps.
pub fn enrich(
    id: &str,
    name: String,
    api_context: Option<u32>,
    api_vision: Option<bool>,
    api_free: Option<bool>,
) -> ModelInfo {
    ModelInfo {
        id: id.to_string(),
        name: name.trim().to_string().replace('\n', " "),
        context_window: api_context.or_else(|| context_for(id)),
        is_free: api_free.unwrap_or(false),
        is_vision: api_vision.unwrap_or_else(|| vision_for(id)),
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
    fn upstream_values_win_over_catalog() {
        let m = enrich(
            "claude-sonnet-4-5-20250929",
            "Claude Sonnet 4.5".into(),
            Some(1_000_000), // API says 1M — trust it
            Some(false),
            None,
        );
        assert_eq!(m.context_window, Some(1_000_000));
        assert!(!m.is_vision);
    }

    #[test]
    fn catalog_fills_gaps() {
        let m = enrich("deepseek-chat", "deepseek-chat".into(), None, None, None);
        assert_eq!(m.context_window, Some(128_000));
        assert!(!m.is_vision);
    }
}
