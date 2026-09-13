use std::sync::Arc;
use axum::extract::State;
use axum::response::{Html, IntoResponse};
use axum::routing::{get, post};
use axum::{Json, Router};
use tower_http::cors::CorsLayer;

use super::sse::sse_handler;
use crate::mcp::tools::{call_tool, list_tools};
use crate::state::AppState;

const INDEX_HTML: &str = include_str!("../../static/index.html");

pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(serve_index))
        .route("/api/environment", get(get_environment))
        .route("/api/tools", get(get_tools))
        .route("/api/execute", post(execute_tool))
        .route("/api/history", get(get_history))
        .route("/api/events", get(get_events))
        .route("/api/record", post(receive_external_record))
        .layer(CorsLayer::permissive())
        .with_state(state)
}

async fn receive_external_record(
    State(state): State<Arc<AppState>>,
    Json(record): Json<crate::state::ExecutionRecord>,
) -> impl IntoResponse {
    state.record_locally(record).await;
    axum::http::StatusCode::OK
}

async fn serve_index() -> Html<&'static str> {
    Html(INDEX_HTML)
}

async fn get_environment(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let fp = state.get_fingerprint().await;
    Json(fp)
}

async fn get_tools() -> impl IntoResponse {
    let tools = list_tools();
    Json(tools)
}

#[derive(serde::Deserialize)]
struct ExecuteRequest {
    tool: String,
    #[serde(default)]
    arguments: serde_json::Value,
}

async fn execute_tool(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ExecuteRequest>,
) -> impl IntoResponse {
    let res = call_tool(&state, &payload.tool, payload.arguments).await;
    Json(res)
}

async fn get_history(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let hist = state.get_history().await;
    Json(hist)
}

async fn get_events(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    sse_handler(state).await
}
