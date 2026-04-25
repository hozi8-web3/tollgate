use axum::{
    extract::{Query, State},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::agents::insights_agent;
use crate::db::read;
use crate::AppState;

#[derive(Deserialize)]
pub struct PeriodParams {
    #[serde(default = "default_period")]
    pub days: i64,
}

fn default_period() -> i64 {
    7
}

#[derive(Deserialize)]
pub struct PaginationParams {
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}

fn default_limit() -> i64 {
    50
}

/// Health check.
pub async fn health() -> Json<Value> {
    Json(json!({ "status": "ok", "version": env!("CARGO_PKG_VERSION") }))
}

/// Get period statistics.
pub async fn stats(
    State(state): State<AppState>,
    Query(params): Query<PeriodParams>,
) -> Json<Value> {
    match read::get_stats(&state.db, params.days) {
        Ok(s) => Json(json!({
            "period_days": params.days,
            "spend_usd": s.spend_usd,
            "requests": s.requests,
            "input_tokens": s.input_tokens,
            "output_tokens": s.output_tokens,
            "prev_period_spend_usd": s.prev_period_spend_usd,
            "avg_cost_per_request": if s.requests > 0 { s.spend_usd / s.requests as f64 } else { 0.0 },
        })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

/// Get paginated request log.
pub async fn requests(
    State(state): State<AppState>,
    Query(params): Query<PaginationParams>,
) -> Json<Value> {
    match read::get_requests(&state.db, params.limit, params.offset) {
        Ok(rows) => Json(json!({ "requests": rows })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

/// Get model breakdown.
pub async fn models(
    State(state): State<AppState>,
    Query(params): Query<PeriodParams>,
) -> Json<Value> {
    match read::get_model_breakdown(&state.db, params.days) {
        Ok(rows) => Json(json!({ "models": rows })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

/// Get task type breakdown.
pub async fn tasks(
    State(state): State<AppState>,
    Query(params): Query<PeriodParams>,
) -> Json<Value> {
    match read::get_task_breakdown(&state.db, params.days) {
        Ok(rows) => Json(json!({ "tasks": rows })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

/// Get daily spend data for charts.
pub async fn daily_spend(
    State(state): State<AppState>,
    Query(params): Query<PeriodParams>,
) -> Json<Value> {
    match read::get_daily_spend(&state.db, params.days) {
        Ok(rows) => Json(json!({ "daily": rows })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

/// Get AI-generated insights.
pub async fn insights(
    State(state): State<AppState>,
    Query(params): Query<PeriodParams>,
) -> Json<Value> {
    let stats = match read::get_stats(&state.db, params.days) {
        Ok(s) => s,
        Err(e) => return Json(json!({"error": e.to_string()})),
    };
    let models = match read::get_model_breakdown(&state.db, params.days) {
        Ok(m) => m,
        Err(e) => return Json(json!({"error": e.to_string()})),
    };
    let tasks = match read::get_task_breakdown(&state.db, params.days) {
        Ok(t) => t,
        Err(e) => return Json(json!({"error": e.to_string()})),
    };
    let cache = match read::get_cache_stats(&state.db, params.days) {
        Ok(c) => c,
        Err(e) => return Json(json!({"error": e.to_string()})),
    };
    let anomalies = match read::get_anomaly_stats(&state.db, params.days) {
        Ok(a) => a,
        Err(e) => return Json(json!({"error": e.to_string()})),
    };

    let output =
        insights_agent::generate_insights(&stats, &models, &tasks, &cache, &anomalies, params.days);
    Json(serde_json::to_value(output).unwrap_or(json!({"error": "serialization failed"})))
}
