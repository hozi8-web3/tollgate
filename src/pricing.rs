use anyhow::Result;
use serde::Deserialize;
use std::collections::HashMap;

/// Pricing data for a single model.
#[derive(Debug, Deserialize, Clone)]
pub struct ModelPricing {
    pub input_per_1m: f64,
    pub output_per_1m: f64,
    pub cache_read_per_1m: Option<f64>,
    pub cache_write_per_1m: Option<f64>,
}

/// Full pricing table: provider -> model -> pricing.
pub type PricingTable = HashMap<String, HashMap<String, ModelPricing>>;

/// Load the embedded pricing table.
pub fn load_pricing() -> Result<PricingTable> {
    let data = include_str!("../pricing.json");
    let table: PricingTable = serde_json::from_str(data)?;
    tracing::info!("Loaded pricing for {} providers", table.len());
    Ok(table)
}

/// Look up pricing for a given provider and model.
pub fn lookup<'a>(
    table: &'a PricingTable,
    provider: &str,
    model: &str,
) -> Option<&'a ModelPricing> {
    table.get(provider).and_then(|models| models.get(model))
}

/// Calculate costs from actual token counts returned by the API.
pub fn calculate_costs(
    pricing: &ModelPricing,
    input_tokens: i64,
    output_tokens: i64,
    cache_read_tokens: i64,
    cache_write_tokens: i64,
) -> CostBreakdown {
    let input_cost = (input_tokens as f64 / 1_000_000.0) * pricing.input_per_1m;
    let output_cost = (output_tokens as f64 / 1_000_000.0) * pricing.output_per_1m;
    let cache_read_cost = pricing
        .cache_read_per_1m
        .map(|rate| (cache_read_tokens as f64 / 1_000_000.0) * rate)
        .unwrap_or(0.0);
    let cache_write_cost = pricing
        .cache_write_per_1m
        .map(|rate| (cache_write_tokens as f64 / 1_000_000.0) * rate)
        .unwrap_or(0.0);
    let total = input_cost + output_cost + cache_read_cost + cache_write_cost;

    CostBreakdown {
        input_cost_usd: input_cost,
        output_cost_usd: output_cost,
        cache_read_cost_usd: cache_read_cost,
        cache_write_cost_usd: cache_write_cost,
        total_cost_usd: total,
    }
}

/// Cost breakdown for a single request.
#[derive(Debug, Clone)]
pub struct CostBreakdown {
    pub input_cost_usd: f64,
    pub output_cost_usd: f64,
    pub cache_read_cost_usd: f64,
    pub cache_write_cost_usd: f64,
    pub total_cost_usd: f64,
}
