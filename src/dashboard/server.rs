use axum::{response::Html, routing::get, Router};
use tower_http::cors::{Any, CorsLayer};

use super::api;
use crate::AppState;

/// Create the dashboard axum router.
pub fn create_router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/", get(serve_index))
        .route("/app.js", get(serve_js))
        .route("/style.css", get(serve_css))
        .route("/api/health", get(api::health))
        .route("/api/stats", get(api::stats))
        .route("/api/requests", get(api::requests))
        .route("/api/models", get(api::models))
        .route("/api/tasks", get(api::tasks))
        .route("/api/daily", get(api::daily_spend))
        .route("/api/insights", get(api::insights))
        .layer(cors)
        .with_state(state)
}

async fn serve_index() -> Html<&'static str> {
    Html(include_str!("static/index.html"))
}

async fn serve_js() -> (
    [(axum::http::header::HeaderName, &'static str); 1],
    &'static str,
) {
    (
        [(axum::http::header::CONTENT_TYPE, "application/javascript")],
        include_str!("static/app.js"),
    )
}

async fn serve_css() -> (
    [(axum::http::header::HeaderName, &'static str); 1],
    &'static str,
) {
    (
        [(axum::http::header::CONTENT_TYPE, "text/css")],
        include_str!("static/style.css"),
    )
}
