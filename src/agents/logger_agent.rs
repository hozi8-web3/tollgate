use crate::pricing::{self, PricingTable};
use crate::proxy::normalizer::NormalizedUsage;

/// Logger Agent output — computed from actual API response token counts.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
    pub cache_write_tokens: i64,
    pub input_cost_usd: f64,
    pub output_cost_usd: f64,
    pub cache_read_cost_usd: f64,
    pub cache_write_cost_usd: f64,
    pub total_cost_usd: f64,
    pub task_type: String,
    pub anomaly: bool,
    pub anomaly_reason: Option<String>,
}

/// Compute the full log entry from actual API response usage data.
/// Token counts come directly from the LLM API response — they are accurate.
/// Costs are calculated from those exact token counts × pricing rates.
#[allow(dead_code)]
pub fn compute_log_entry(
    usage: &NormalizedUsage,
    pricing_table: &PricingTable,
    provider: &str,
    model: &str,
    rolling_7d_avg: f64,
    anomaly_multiplier: f64,
    task_type: &str,
) -> LogEntry {
    let costs = pricing::lookup(pricing_table, provider, model)
        .map(|p| {
            pricing::calculate_costs(
                p,
                usage.input_tokens,
                usage.output_tokens,
                usage.cache_read_tokens,
                usage.cache_write_tokens,
            )
        })
        .unwrap_or(pricing::CostBreakdown {
            input_cost_usd: 0.0,
            output_cost_usd: 0.0,
            cache_read_cost_usd: 0.0,
            cache_write_cost_usd: 0.0,
            total_cost_usd: 0.0,
        });

    let anomaly =
        rolling_7d_avg > 0.0 && costs.total_cost_usd > (rolling_7d_avg * anomaly_multiplier);
    let anomaly_reason = if anomaly {
        Some(format!(
            "Cost ${:.4} exceeds {:.1}x the 7-day average of ${:.4}",
            costs.total_cost_usd, anomaly_multiplier, rolling_7d_avg,
        ))
    } else {
        None
    };

    LogEntry {
        input_tokens: usage.input_tokens,
        output_tokens: usage.output_tokens,
        cache_read_tokens: usage.cache_read_tokens,
        cache_write_tokens: usage.cache_write_tokens,
        input_cost_usd: costs.input_cost_usd,
        output_cost_usd: costs.output_cost_usd,
        cache_read_cost_usd: costs.cache_read_cost_usd,
        cache_write_cost_usd: costs.cache_write_cost_usd,
        total_cost_usd: costs.total_cost_usd,
        task_type: task_type.to_string(),
        anomaly,
        anomaly_reason,
    }
}
