//! Known-model catalog.
//!
//! `/v1/models` endpoints (OpenAI-compat) and even Anthropic's `/v1/models`
//! rarely expose every useful attribute. This catalog fills the gaps with
//! well-known context windows / vision support per model family, so the model
//! picker can show correct context numbers and capabilities regardless of
//! whether the upstream API reports them.

use super::types::ModelInfo;

/// OpenCode GO (https://opencode.ai/zen/go/v1) — paid tier, authenticated with
/// an OpenCode API key. Data from models.dev (2026-08). The GO /models response
/// carries no context info, so without this table the generic family fallbacks
/// below would report e.g. deepseek-v4-pro as 128k instead of 1M.
/// (id, context, max output)
const OPENCODE_GO: &[(&str, u32, u32)] = &[
    ("deepseek-v4-flash", 1_000_000, 384_000),
    ("deepseek-v4-pro", 1_000_000, 384_000),
    ("glm-5", 202_752, 32_768),
    ("glm-5.1", 202_752, 32_768),
    ("glm-5.2", 1_000_000, 131_072),
    ("gpt-5.6-luna", 1_050_000, 128_000),
    ("grok-4.5", 500_000, 500_000),
    ("hy3", 256_000, 64_000),
    ("kimi-k2.5", 262_144, 65_536),
    ("kimi-k2.6", 262_144, 65_536),
    ("kimi-k2.7-code", 262_144, 262_144),
    ("kimi-k3", 1_048_576, 131_072),
    ("mimo-v2-omni", 262_144, 128_000),
    ("mimo-v2-pro", 1_048_576, 128_000),
    ("mimo-v2.5", 1_000_000, 128_000),
    ("mimo-v2.5-pro", 1_048_576, 128_000),
    ("minimax-m2.5", 204_800, 65_536),
    ("minimax-m2.7", 204_800, 131_072),
    ("minimax-m3", 1_000_000, 131_072),
    ("qwen3.5-plus", 262_144, 65_536),
    ("qwen3.6-plus", 1_000_000, 65_536),
    ("qwen3.7-max", 1_000_000, 65_536),
    ("qwen3.7-plus", 1_000_000, 65_536),
    ("qwen3.8-max", 1_000_000, 131_072),
];

/// OpenCode ZEN (https://opencode.ai/zen/v1) — free tier. Only `-free` models
/// are actually served (paid ids on the ZEN list fail with CreditsError), so
/// the harness exposes just the free ones. (id, context, max output)
const OPENCODE_ZEN_FREE: &[(&str, u32, u32)] = &[("deepseek-v4-flash-free", 200_000, 128_000)];

fn opencode_go_info(id: &str) -> Option<(u32, u32)> {
    OPENCODE_GO
        .iter()
        .find(|(mid, _, _)| *mid == id)
        .map(|(_, ctx, out)| (*ctx, *out))
}

fn opencode_zen_info(id: &str) -> Option<(u32, u32)> {
    OPENCODE_ZEN_FREE
        .iter()
        .find(|(mid, _, _)| *mid == id)
        .map(|(_, ctx, out)| (*ctx, *out))
}

/// Best-effort context window for a model id, from the known families below.
/// `None` means we don't recognise the id — the UI simply omits the badge.
pub fn context_for(id: &str) -> Option<u32> {
    let id = id.to_ascii_lowercase();
    // OpenCode tiers carry exact context windows (no upstream info — see above).
    if let Some((ctx, _)) = opencode_go_info(&id) {
        return Some(ctx);
    }
    if let Some((ctx, _)) = opencode_zen_info(&id) {
        return Some(ctx);
    }
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
    if id.starts_with("mixtral")
        || id.starts_with("mistral-nemo")
        || id.starts_with("mistral-large")
    {
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
    // OpenCode GO models are all reasoning models (deepseek-v4 reasons by
    // default, like deepseek-reasoner; the others stream reasoning tokens).
    if opencode_go_info(&id).is_some() {
        return true;
    }
    if let Some((_, _)) = opencode_zen_info(&id) {
        return true;
    }
    if id.contains("haiku") {
        return false;
    }
    if id.starts_with("claude-") {
        return id.contains("opus-4")
            || id.contains("sonnet-4-5")
            || id.contains("sonnet-4-6")
            || id.contains("claude-3-7");
    }
    if id.starts_with("gpt-5")
        || id.starts_with("o1")
        || id.starts_with("o3")
        || id.starts_with("o4")
    {
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
    // OpenCode GO: deepseek-v4 accepts reasoning_effort (verified); the rest
    // reason by default but tolerate no explicit level field, so leave them
    // on/off only. Must short-circuit BEFORE the qwen3/gpt-5 family checks
    // (qwen3.7-plus etc. would otherwise match them).
    if opencode_go_info(&id).is_some() {
        return matches!(id.as_str(), "deepseek-v4-pro" | "deepseek-v4-flash");
    }
    if id.contains("haiku") {
        return false;
    }
    if id.starts_with("claude-") {
        return id.contains("opus-4") || id.contains("sonnet-4-5") || id.contains("sonnet-4-6");
    }
    if id.starts_with("gpt-5")
        || id.starts_with("o1")
        || id.starts_with("o3")
        || id.starts_with("o4")
    {
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
    fn opencode_tiers_have_exact_contexts() {
        // OpenCode GO (paid) — not the generic deepseek-* 128k fallback.
        assert_eq!(context_for("deepseek-v4-pro"), Some(1_000_000));
        assert_eq!(context_for("deepseek-v4-flash"), Some(1_000_000));
        assert_eq!(context_for("qwen3.7-plus"), Some(1_000_000));
        assert_eq!(context_for("minimax-m3"), Some(1_000_000));
        assert_eq!(context_for("kimi-k3"), Some(1_048_576));
        assert_eq!(context_for("glm-5"), Some(202_752));
        // OpenCode ZEN free tier.
        assert_eq!(context_for("deepseek-v4-flash-free"), Some(200_000));
        // All GO models reason; deepseek-v4 also takes explicit levels.
        assert!(thinking_for("deepseek-v4-pro"));
        assert!(thinking_for("qwen3.7-plus"));
        assert!(thinking_levels_for("deepseek-v4-flash"));
        assert!(!thinking_levels_for("qwen3.7-plus"));
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
        for m in [
            "claude-sonnet-4-5-20250929",
            "claude-opus-4-1-20250805",
            "claude-sonnet-4-6-20251022",
        ] {
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
        let m = enrich(
            "deepseek-chat",
            "deepseek-chat".into(),
            None,
            None,
            None,
            None,
            None,
        );
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
