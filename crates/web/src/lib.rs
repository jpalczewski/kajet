#[macro_use]
extern crate rust_i18n;

i18n!("../../locales", fallback = "en");

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Query, State,
    },
    http::{header, StatusCode},
    response::{IntoResponse, Json},
    routing::get,
    Router,
};
use kajet_core::types::AppState;
use rust_embed::RustEmbed;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(RustEmbed, Clone)]
#[folder = "../../frontend/dist/"]
struct Assets;

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub async fn serve(state: Arc<AppState>, port: u16) -> anyhow::Result<()> {
    let app = Router::new()
        .route("/api/search", get(api_search))
        .route("/api/status", get(api_status))
        .route("/api/config", get(api_config))
        .route("/api/i18n", get(api_i18n))
        .route("/ws", get(ws_handler))
        .fallback(serve_spa)
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port)).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// SPA — serve static assets or index.html as fallback
// ---------------------------------------------------------------------------

async fn serve_spa(uri: axum::http::Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');

    // Try to serve the exact file first
    if !path.is_empty() {
        if let Some(file) = Assets::get(path) {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            return (
                StatusCode::OK,
                [(header::CONTENT_TYPE, mime.as_ref().to_string())],
                file.data.to_vec(),
            )
                .into_response();
        }
    }

    // Fallback to index.html for SPA routing
    match Assets::get("index.html") {
        Some(file) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/html".to_string())],
            file.data.to_vec(),
        )
            .into_response(),
        None => (StatusCode::NOT_FOUND, "index.html not found").into_response(),
    }
}

// ---------------------------------------------------------------------------
// i18n API — returns dashboard translations as JSON
// ---------------------------------------------------------------------------

const DASHBOARD_KEYS: &[&str] = &[
    "title",
    "subtitle",
    "ws_connecting",
    "ws_connected",
    "ws_disconnected",
    "panel_query_log",
    "panel_stats",
    "panel_search",
    "waiting_for_queries",
    "stats_message",
    "search_placeholder",
    "searching",
    "dashboard_no_results",
    "results_suffix",
    "nav_dashboard",
    "nav_status",
    "nav_settings",
    "status_vault_path",
    "status_note_count",
    "status_chunk_count",
    "status_model",
    "status_language",
    "loading",
];

async fn api_i18n() -> Json<HashMap<String, String>> {
    let translations: HashMap<String, String> = DASHBOARD_KEYS
        .iter()
        .map(|&key| (key.to_string(), t!(key).to_string()))
        .collect();
    Json(translations)
}

// ---------------------------------------------------------------------------
// Search API — JSON response
// ---------------------------------------------------------------------------

#[derive(serde::Deserialize)]
struct SearchQuery {
    q: String,
    #[serde(default = "default_limit")]
    limit: usize,
}

fn default_limit() -> usize {
    10
}

async fn api_search(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SearchQuery>,
) -> impl IntoResponse {
    match state
        .search_engine
        .vector_search(&params.q, params.limit)
        .await
    {
        Ok(results) => Json(results).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

// ---------------------------------------------------------------------------
// Status API
// ---------------------------------------------------------------------------

#[derive(serde::Serialize)]
struct VaultStatus {
    vault_path: String,
    note_count: usize,
    chunk_count: usize,
    model: String,
    language: String,
}

async fn api_status(State(state): State<Arc<AppState>>) -> Json<VaultStatus> {
    Json(VaultStatus {
        vault_path: state.vault_path.clone(),
        note_count: state.note_count,
        chunk_count: state.chunk_count,
        model: "AllMiniLM-L6-v2".to_string(),
        language: state.config.language.clone(),
    })
}

// ---------------------------------------------------------------------------
// Config API
// ---------------------------------------------------------------------------

async fn api_config(State(state): State<Arc<AppState>>) -> Json<kajet_core::config::KajetConfig> {
    Json(state.config.clone())
}

// ---------------------------------------------------------------------------
// WebSocket — live query event stream
// ---------------------------------------------------------------------------

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_ws(socket, state))
}

async fn handle_ws(mut socket: WebSocket, state: Arc<AppState>) {
    let mut rx = state.events.subscribe();

    while let Ok(event) = rx.recv().await {
        let json = serde_json::to_string(&event).unwrap_or_default();
        if socket.send(Message::Text(json.into())).await.is_err() {
            break;
        }
    }
}
