#[macro_use]
extern crate rust_i18n;

i18n!("../../locales", fallback = "en");

use axum::{
    Router,
    extract::{
        Query, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::{StatusCode, header},
    response::{IntoResponse, Json},
    routing::{get, put},
};
use kajet_core::logging::types::WsMessage;
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
        .route("/api/config/global", put(api_config_global))
        .route("/api/config/vault", put(api_config_vault))
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
    if !path.is_empty()
        && let Some(file) = Assets::get(path)
    {
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        return (
            StatusCode::OK,
            [(header::CONTENT_TYPE, mime.as_ref().to_string())],
            file.data.to_vec(),
        )
            .into_response();
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
    "settings_global",
    "settings_vault",
    "settings_save",
    "settings_saved",
    "settings_error",
    "settings_restart_required",
    "settings_language",
    "settings_port",
    "settings_default_limit",
    "settings_max_concurrent_files",
    "settings_pipeline_buffer_size",
    "settings_exclude_folders",
    "settings_embedding_backend",
    "settings_embedding_model",
    "settings_embedding_base_url",
    "settings_embedding_api_key",
    "settings_embedding_document_prefix",
    "settings_embedding_query_prefix",
    "settings_open_browser",
    "settings_logging",
    "settings_log_level",
    "settings_file_level",
    "settings_dashboard_level",
    "settings_progress_step",
    "settings_search_tuning",
    "settings_filter_overfetch_multiplier",
    "settings_tags_only_fetch_limit",
    "settings_filter_overfetch_multiplier_hint",
    "settings_tags_only_fetch_limit_hint",
    "nav_logs",
    "logs_title",
    "logs_level_filter",
    "logs_entries",
    "logs_empty",
    "indexing_in_progress",
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
        Err(e) => {
            tracing::error!(error = %e, "Search failed");
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
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
    indexing: bool,
}

async fn api_status(State(state): State<Arc<AppState>>) -> Json<VaultStatus> {
    let config = state.config.read().unwrap();
    Json(VaultStatus {
        vault_path: state.vault_path.clone(),
        note_count: state.note_count.load(std::sync::atomic::Ordering::Relaxed),
        chunk_count: state.chunk_count.load(std::sync::atomic::Ordering::Relaxed),
        model: config.embedding.model.clone(),
        language: config.language.clone(),
        indexing: state.indexing.load(std::sync::atomic::Ordering::Relaxed),
    })
}

// ---------------------------------------------------------------------------
// Config API — read
// ---------------------------------------------------------------------------

async fn api_config(State(state): State<Arc<AppState>>) -> Json<kajet_core::config::KajetConfig> {
    let config = state.config.read().unwrap();
    Json(config.clone())
}

// ---------------------------------------------------------------------------
// Config API — write global
// ---------------------------------------------------------------------------

#[derive(serde::Deserialize)]
struct ConfigUpdateRequest {
    updates: HashMap<String, toml::Value>,
}

async fn api_config_global(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ConfigUpdateRequest>,
) -> impl IntoResponse {
    if let Err(e) = kajet_core::config::write_global_config(&body.updates) {
        return (StatusCode::BAD_REQUEST, e.to_string()).into_response();
    }

    // Reload config into RwLock
    let (port, cli_lang) = {
        let cfg = state.config.read().unwrap();
        (cfg.port, None::<String>)
    };
    match kajet_core::config::reload_config(&state.db_path, port, cli_lang) {
        Ok(new_cfg) => {
            // If language changed, update locale
            let new_lang = new_cfg.language.clone();
            *state.config.write().unwrap() = new_cfg;
            rust_i18n::set_locale(&new_lang);
            StatusCode::OK.into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

// ---------------------------------------------------------------------------
// Config API — write vault
// ---------------------------------------------------------------------------

async fn api_config_vault(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ConfigUpdateRequest>,
) -> impl IntoResponse {
    if let Err(e) = kajet_core::config::write_vault_config(&state.db_path, &body.updates) {
        return (StatusCode::BAD_REQUEST, e.to_string()).into_response();
    }

    // Reload config into RwLock
    let (port, cli_lang) = {
        let cfg = state.config.read().unwrap();
        (cfg.port, None::<String>)
    };
    match kajet_core::config::reload_config(&state.db_path, port, cli_lang) {
        Ok(new_cfg) => {
            *state.config.write().unwrap() = new_cfg;
            StatusCode::OK.into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

// ---------------------------------------------------------------------------
// WebSocket — live query event stream
// ---------------------------------------------------------------------------

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_ws(socket, state))
}

async fn handle_ws(mut socket: WebSocket, state: Arc<AppState>) {
    // Send ring buffer history on connect
    for entry in state.log_buffer.recent_entries() {
        let msg = WsMessage::Log(entry);
        let json = serde_json::to_string(&msg).unwrap_or_default();
        if socket.send(Message::Text(json.into())).await.is_err() {
            return;
        }
    }

    let mut query_rx = state.events.subscribe();
    let mut log_rx = state.log_events.subscribe();

    loop {
        let msg = tokio::select! {
            Ok(event) = query_rx.recv() => WsMessage::Query(event),
            Ok(entry) = log_rx.recv() => WsMessage::Log(entry),
            else => break,
        };
        let json = serde_json::to_string(&msg).unwrap_or_default();
        if socket.send(Message::Text(json.into())).await.is_err() {
            break;
        }
    }
}
