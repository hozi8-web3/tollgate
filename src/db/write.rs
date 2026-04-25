use anyhow::Result;
use serde::Serialize;

use super::DbPool;

/// A single request log row.
#[derive(Debug, Serialize, Clone)]
pub struct RequestRow {
    pub id: String,
    pub timestamp: String,
    pub provider: String,
    pub model: String,
    pub original_model: String,
    pub was_substituted: bool,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
    pub cache_write_tokens: i64,
    pub input_cost_usd: f64,
    pub output_cost_usd: f64,
    pub cache_read_cost_usd: f64,
    pub cache_write_cost_usd: f64,
    pub total_cost_usd: f64,
    pub latency_ms: i64,
    pub stop_reason: Option<String>,
    pub task_type: Option<String>,
    pub tags: Option<String>,
    pub anomaly: bool,
    pub anomaly_reason: Option<String>,
}

/// Insert a request log row into the database.
pub fn insert_request(db: &DbPool, row: &RequestRow) -> Result<()> {
    let conn = db
        .lock()
        .map_err(|e| anyhow::anyhow!("DB lock error: {}", e))?;
    conn.execute(
        "INSERT INTO requests (
            id, timestamp, provider, model, original_model, was_substituted,
            input_tokens, output_tokens, cache_read_tokens, cache_write_tokens,
            input_cost_usd, output_cost_usd, cache_read_cost_usd, cache_write_cost_usd,
            total_cost_usd, latency_ms, stop_reason, task_type, tags, anomaly, anomaly_reason
        ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6,
            ?7, ?8, ?9, ?10,
            ?11, ?12, ?13, ?14,
            ?15, ?16, ?17, ?18, ?19, ?20, ?21
        )",
        rusqlite::params![
            row.id,
            row.timestamp,
            row.provider,
            row.model,
            row.original_model,
            row.was_substituted as i32,
            row.input_tokens,
            row.output_tokens,
            row.cache_read_tokens,
            row.cache_write_tokens,
            row.input_cost_usd,
            row.output_cost_usd,
            row.cache_read_cost_usd,
            row.cache_write_cost_usd,
            row.total_cost_usd,
            row.latency_ms,
            row.stop_reason,
            row.task_type,
            row.tags,
            row.anomaly as i32,
            row.anomaly_reason,
        ],
    )?;

    tracing::debug!("Logged request {} — ${:.6}", row.id, row.total_cost_usd);
    Ok(())
}
