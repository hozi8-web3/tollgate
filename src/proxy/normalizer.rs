use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Normalized usage data extracted from any provider's response.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NormalizedUsage {
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
    pub cache_write_tokens: i64,
    pub stop_reason: Option<String>,
    pub model: Option<String>,
}

/// Extract normalized usage from an OpenAI-format response.
/// OpenAI responses have: { "usage": { "prompt_tokens": N, "completion_tokens": N, ... } }
pub fn normalize_openai(body: &Value) -> NormalizedUsage {
    let usage = body.get("usage");
    let mut norm = NormalizedUsage::default();

    if let Some(u) = usage {
        norm.input_tokens = u.get("prompt_tokens").and_then(|v| v.as_i64()).unwrap_or(0);
        norm.output_tokens = u
            .get("completion_tokens")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        // OpenAI cached tokens
        if let Some(details) = u.get("prompt_tokens_details") {
            norm.cache_read_tokens = details
                .get("cached_tokens")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
        }
    }

    norm.stop_reason = body
        .get("choices")
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.first())
        .and_then(|c| c.get("finish_reason"))
        .and_then(|v| v.as_str())
        .map(String::from);

    norm.model = body.get("model").and_then(|v| v.as_str()).map(String::from);

    norm
}

/// Extract normalized usage from an Anthropic-format response.
/// Anthropic responses have: { "usage": { "input_tokens": N, "output_tokens": N, ... } }
pub fn normalize_anthropic(body: &Value) -> NormalizedUsage {
    let usage = body.get("usage");
    let mut norm = NormalizedUsage::default();

    if let Some(u) = usage {
        norm.input_tokens = u.get("input_tokens").and_then(|v| v.as_i64()).unwrap_or(0);
        norm.output_tokens = u.get("output_tokens").and_then(|v| v.as_i64()).unwrap_or(0);
        norm.cache_read_tokens = u
            .get("cache_read_input_tokens")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        norm.cache_write_tokens = u
            .get("cache_creation_input_tokens")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
    }

    norm.stop_reason = body
        .get("stop_reason")
        .and_then(|v| v.as_str())
        .map(String::from);
    norm.model = body.get("model").and_then(|v| v.as_str()).map(String::from);

    norm
}

/// Auto-detect provider format and normalize.
pub fn normalize_response(provider: &str, body: &Value) -> NormalizedUsage {
    match provider {
        "anthropic" => normalize_anthropic(body),
        "openai" | "groq" => normalize_openai(body),
        _ => {
            // Try OpenAI format first (most common), fall back to Anthropic
            if body
                .get("usage")
                .and_then(|u| u.get("prompt_tokens"))
                .is_some()
            {
                normalize_openai(body)
            } else if body
                .get("usage")
                .and_then(|u| u.get("input_tokens"))
                .is_some()
            {
                normalize_anthropic(body)
            } else {
                tracing::warn!(
                    "Unknown response format for provider '{}', returning zero usage",
                    provider
                );
                NormalizedUsage::default()
            }
        }
    }
}
