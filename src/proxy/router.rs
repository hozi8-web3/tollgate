use axum::{
    body::Body,
    extract::{Path, State},
    http::{HeaderMap, Method, Response, StatusCode},
    response::IntoResponse,
};
use bytes::Bytes;
use chrono::Utc;
use std::collections::HashMap;
use uuid::Uuid;

use crate::agents::router_agent;
use crate::db;
use crate::db::write::RequestRow;
use crate::pricing;
use crate::proxy::forwarder;
use crate::proxy::normalizer;
use crate::AppState;

/// Main proxy handler — catches all requests to /:provider/*path
pub async fn proxy_handler(
    State(state): State<AppState>,
    Path((provider, path)): Path<(String, String)>,
    method: Method,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let start = std::time::Instant::now();
    let request_id = Uuid::new_v4().to_string();

    tracing::info!("[{}] {} /{}/{}", request_id, method, provider, path);

    // 1. Resolve provider base URL
    let base_url = match state.config.get_base_url(&provider) {
        Some(url) => url,
        None => {
            tracing::error!("Unknown provider: {}", provider);
            return Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Body::from(format!("{{\"error\": \"Unknown provider: {}\"}}", provider)))
                .unwrap();
        }
    };

    // 2. Get API key from environment
    let api_key = state.config.get_api_key(&provider);

    // 3. Parse request body to extract model info
    let body_json: Option<serde_json::Value> = serde_json::from_slice(&body).ok();
    let original_model = body_json.as_ref()
        .and_then(|b| b.get("model"))
        .and_then(|m| m.as_str())
        .unwrap_or("unknown")
        .to_string();

    // 4. Extract last user message for routing/classification
    let last_user_message = extract_last_user_message(&body_json);

    // 5. Run Router Agent (rule-based) to decide routing
    let route_decision = router_agent::decide_route(
        &state.config,
        &provider,
        &original_model,
        &last_user_message,
    );

    // 6. Check for block decision
    if route_decision.action == "block" {
        tracing::warn!("[{}] Request blocked: {}", request_id, route_decision.block_reason.as_deref().unwrap_or("unknown"));
        return Response::builder()
            .status(StatusCode::TOO_MANY_REQUESTS)
            .body(Body::from(serde_json::json!({
                "error": {
                    "message": route_decision.block_reason.unwrap_or("Request blocked by cost policy".to_string()),
                    "type": "cost_limit_exceeded"
                }
            }).to_string()))
            .unwrap();
    }

    // 7. Apply substitution if needed
    let (effective_provider, effective_model) = if route_decision.action == "substitute" {
        tracing::info!("[{}] Substituting {} -> {} ({})", request_id,
            original_model, route_decision.model, route_decision.substitution_reason.as_deref().unwrap_or(""));
        (route_decision.provider.clone(), route_decision.model.clone())
    } else {
        (provider.clone(), original_model.clone())
    };

    let was_substituted = route_decision.action == "substitute";

    // 8. Modify body if model was substituted
    let final_body = if was_substituted {
        if let Some(mut json) = body_json.clone() {
            json["model"] = serde_json::Value::String(effective_model.clone());
            Bytes::from(serde_json::to_vec(&json).unwrap_or_else(|_| body.to_vec()))
        } else {
            body.clone()
        }
    } else {
        body.clone()
    };

    // 9. Build target URL
    let effective_base = state.config.get_base_url(&effective_provider)
        .unwrap_or(base_url);
    let target_url = forwarder::build_target_url(&effective_base, &path);

    // 10. Build headers map
    let mut header_map: HashMap<String, String> = HashMap::new();
    for (key, value) in headers.iter() {
        if let Ok(v) = value.to_str() {
            header_map.insert(key.to_string(), v.to_string());
        }
    }

    // Inject API key if available and not already present
    if let Some(key) = &api_key {
        if provider == "anthropic" {
            header_map.entry("x-api-key".to_string()).or_insert_with(|| key.clone());
        } else {
            header_map.entry("authorization".to_string())
                .or_insert_with(|| format!("Bearer {}", key));
        }
    }

    // 11. Forward request
    let client = &state.http_client;
    let response = match forwarder::forward_request(
        client, &target_url, method.as_str(), &header_map, final_body,
    ).await {
        Ok(resp) => resp,
        Err(e) => {
            tracing::error!("[{}] Forward error: {}", request_id, e);
            return Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .body(Body::from(format!("{{\"error\": \"Proxy forward error: {}\"}}", e)))
                .unwrap();
        }
    };

    let latency_ms = start.elapsed().as_millis() as i64;
    let response_status = response.status();

    // 12. Copy response headers
    let mut resp_builder = Response::builder().status(response_status.as_u16());
    for (key, value) in response.headers() {
        if key != "transfer-encoding" {
            resp_builder = resp_builder.header(key, value);
        }
    }

    // 13. Check if streaming
    let is_stream = forwarder::is_sse_response(&response);

    if is_stream {
        // For streaming responses, pass through and log asynchronously
        let db = state.db.clone();
        let pricing_table = state.pricing.clone();
        let prov = effective_provider.clone();
        let model = effective_model.clone();
        let orig_model = original_model.clone();
        let anomaly_mult = state.config.alerts.anomaly_multiplier;
        let user_msg = last_user_message.clone();

        // Create a streaming passthrough that captures the final usage chunk
        let (tx, rx) = tokio::sync::mpsc::channel::<Result<Bytes, std::io::Error>>(64);
        let mut byte_stream = response.bytes_stream();

        tokio::spawn(async move {
            use futures_util::StreamExt;
            let mut all_chunks: Vec<Bytes> = Vec::new();

            while let Some(chunk_result) = byte_stream.next().await {
                match chunk_result {
                    Ok(chunk) => {
                        all_chunks.push(chunk.clone());
                        if tx.send(Ok(chunk)).await.is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(Err(std::io::Error::new(
                            std::io::ErrorKind::Other, e.to_string()
                        ))).await;
                        break;
                    }
                }
            }
            drop(tx);

            // Extract usage from the accumulated SSE chunks
            if let Some(final_data) = crate::proxy::streamer::extract_usage_from_sse_chunks(&all_chunks) {
                let usage = normalizer::normalize_response(&prov, &final_data);
                log_request(
                    &db, &pricing_table, &request_id, &prov, &model, &orig_model,
                    was_substituted, &usage, latency_ms, anomaly_mult, &user_msg,
                ).await;
            }
        });

        let body_stream = tokio_stream::wrappers::ReceiverStream::new(rx);
        let body = Body::from_stream(body_stream);
        resp_builder.body(body).unwrap()
    } else {
        // Non-streaming: read full body, extract usage, log, return
        let resp_bytes = match response.bytes().await {
            Ok(b) => b,
            Err(e) => {
                tracing::error!("[{}] Failed to read response body: {}", request_id, e);
                return Response::builder()
                    .status(StatusCode::BAD_GATEWAY)
                    .body(Body::from(format!("{{\"error\": \"Failed to read response: {}\"}}", e)))
                    .unwrap();
            }
        };

        // Parse and extract usage from response
        if let Ok(resp_json) = serde_json::from_slice::<serde_json::Value>(&resp_bytes) {
            let usage = normalizer::normalize_response(&effective_provider, &resp_json);

            // Log asynchronously
            let db = state.db.clone();
            let pricing_table = state.pricing.clone();
            let prov = effective_provider.clone();
            let model = effective_model.clone();
            let orig_model = original_model.clone();
            let anomaly_mult = state.config.alerts.anomaly_multiplier;
            let user_msg = last_user_message.clone();
            let rid = request_id.clone();

            tokio::spawn(async move {
                log_request(
                    &db, &pricing_table, &rid, &prov, &model, &orig_model,
                    was_substituted, &usage, latency_ms, anomaly_mult, &user_msg,
                ).await;
            });
        }

        resp_builder.body(Body::from(resp_bytes)).unwrap()
    }
}

/// Extract the last user message from the request body for routing/classification.
fn extract_last_user_message(body: &Option<serde_json::Value>) -> String {
    body.as_ref()
        .and_then(|b| b.get("messages"))
        .and_then(|m| m.as_array())
        .and_then(|arr| {
            arr.iter().rev().find(|msg| {
                msg.get("role").and_then(|r| r.as_str()) == Some("user")
            })
        })
        .and_then(|msg| msg.get("content"))
        .and_then(|c| {
            // Content can be a string or array of content blocks
            if let Some(s) = c.as_str() {
                Some(s.to_string())
            } else if let Some(arr) = c.as_array() {
                arr.iter()
                    .filter_map(|block| block.get("text").and_then(|t| t.as_str()))
                    .collect::<Vec<_>>()
                    .join(" ")
                    .into()
            } else {
                None
            }
        })
        .unwrap_or_default()
        .chars()
        .take(500)
        .collect()
}

/// Log a completed request to the database with accurate token counts and costs.
async fn log_request(
    db: &db::DbPool,
    pricing_table: &pricing::PricingTable,
    request_id: &str,
    provider: &str,
    model: &str,
    original_model: &str,
    was_substituted: bool,
    usage: &normalizer::NormalizedUsage,
    latency_ms: i64,
    anomaly_multiplier: f64,
    last_user_message: &str,
) {
    // Calculate costs using actual token counts from the API response
    let costs = pricing::lookup(pricing_table, provider, model)
        .map(|p| pricing::calculate_costs(
            p, usage.input_tokens, usage.output_tokens,
            usage.cache_read_tokens, usage.cache_write_tokens,
        ))
        .unwrap_or_else(|| {
            tracing::warn!("No pricing data for {}/{}, costs will be zero", provider, model);
            pricing::CostBreakdown {
                input_cost_usd: 0.0, output_cost_usd: 0.0,
                cache_read_cost_usd: 0.0, cache_write_cost_usd: 0.0,
                total_cost_usd: 0.0,
            }
        });

    // Check for anomaly
    let rolling_avg = db::read::get_rolling_avg_cost(db).unwrap_or(0.0);
    let is_anomaly = rolling_avg > 0.0 && costs.total_cost_usd > (rolling_avg * anomaly_multiplier);
    let anomaly_reason = if is_anomaly {
        Some(format!("Cost ${:.4} is {:.1}x the 7-day average ${:.4}",
            costs.total_cost_usd, costs.total_cost_usd / rolling_avg, rolling_avg))
    } else {
        None
    };

    // Classify task type from the user message
    let task_type = classify_task(last_user_message);

    let row = RequestRow {
        id: request_id.to_string(),
        timestamp: Utc::now().to_rfc3339(),
        provider: provider.to_string(),
        model: model.to_string(),
        original_model: original_model.to_string(),
        was_substituted,
        input_tokens: usage.input_tokens,
        output_tokens: usage.output_tokens,
        cache_read_tokens: usage.cache_read_tokens,
        cache_write_tokens: usage.cache_write_tokens,
        input_cost_usd: costs.input_cost_usd,
        output_cost_usd: costs.output_cost_usd,
        cache_read_cost_usd: costs.cache_read_cost_usd,
        cache_write_cost_usd: costs.cache_write_cost_usd,
        total_cost_usd: costs.total_cost_usd,
        latency_ms,
        stop_reason: usage.stop_reason.clone(),
        task_type: Some(task_type),
        tags: None,
        anomaly: is_anomaly,
        anomaly_reason,
    };

    if let Err(e) = db::write::insert_request(db, &row) {
        tracing::error!("Failed to log request {}: {}", request_id, e);
    }
}

/// Simple keyword-based task classification.
fn classify_task(message: &str) -> String {
    let msg = message.to_lowercase();
    if msg.contains("```") || msg.contains("code") || msg.contains("function") || msg.contains("implement") || msg.contains("debug") || msg.contains("fix") {
        "code".to_string()
    } else if msg.contains("summarize") || msg.contains("summary") || msg.contains("tldr") {
        "summarization".to_string()
    } else if msg.contains("translate") || msg.contains("translation") {
        "translation".to_string()
    } else if msg.contains("classify") || msg.contains("categorize") || msg.contains("label") {
        "classification".to_string()
    } else if msg.contains("extract") || msg.contains("parse") || msg.contains("json") {
        "data_extraction".to_string()
    } else if msg.contains("write") || msg.contains("story") || msg.contains("poem") || msg.contains("creative") {
        "creative_writing".to_string()
    } else if msg.contains("analyze") || msg.contains("analysis") || msg.contains("compare") {
        "analysis".to_string()
    } else if msg.contains("?") || msg.contains("what") || msg.contains("how") || msg.contains("why") || msg.contains("explain") {
        "question_answering".to_string()
    } else {
        "other".to_string()
    }
}
