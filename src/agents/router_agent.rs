use crate::config::AppConfig;

/// Router Agent decision output.
#[derive(Debug, Clone)]
pub struct RouteDecision {
    pub action: String, // "forward" | "substitute" | "block"
    pub provider: String,
    pub model: String,
    pub substitution_reason: Option<String>,
    pub block_reason: Option<String>,
}

/// Decide how to route an incoming request.
/// Uses config-based rules (no LLM call needed for routing).
pub fn decide_route(
    config: &AppConfig,
    provider: &str,
    model: &str,
    last_user_message: &str,
) -> RouteDecision {
    let msg_lower = last_user_message.to_lowercase();

    // 1. Check custom rules — if message contains a rule's keyword, apply it
    for rule in &config.routing.rules {
        if msg_lower.contains(&rule.if_prompt_contains.to_lowercase()) {
            return RouteDecision {
                action: "substitute".to_string(),
                provider: rule.use_provider.clone(),
                model: rule.use_model.clone(),
                substitution_reason: Some(rule.reason.clone()),
                block_reason: None,
            };
        }
    }

    // 2. If cost_optimize is enabled, try to downgrade simple tasks to cheaper models
    if config.routing.cost_optimize && is_simple_task(&msg_lower) {
        // Downgrade to cheapest available model
        let (cheap_provider, cheap_model) = cheapest_model(provider);
        if cheap_model != model {
            return RouteDecision {
                action: "substitute".to_string(),
                provider: cheap_provider.to_string(),
                model: cheap_model.to_string(),
                substitution_reason: Some("Simple task routed to cheaper model".to_string()),
                block_reason: None,
            };
        }
    }

    // 3. Forward as-is
    RouteDecision {
        action: "forward".to_string(),
        provider: provider.to_string(),
        model: model.to_string(),
        substitution_reason: None,
        block_reason: None,
    }
}

/// Check if a message looks like a simple task.
fn is_simple_task(msg: &str) -> bool {
    let simple_verbs = [
        "classify",
        "translate",
        "summarize",
        "extract",
        "list",
        "label",
        "categorize",
    ];
    let is_short = msg.len() < 200;
    let has_simple_verb = simple_verbs.iter().any(|v| msg.contains(v));
    let no_code = !msg.contains("```");
    let is_question = msg.ends_with('?') && msg.split_whitespace().count() < 15;

    is_short && (has_simple_verb || is_question) && no_code
}

/// Return the cheapest model for a given provider.
fn cheapest_model(provider: &str) -> (&str, &str) {
    match provider {
        "anthropic" => ("anthropic", "claude-haiku-4-5-20251001"),
        "openai" => ("openai", "gpt-4o-mini"),
        "groq" => ("groq", "llama-3.1-8b-instant"),
        _ => (provider, "unknown"),
    }
}
