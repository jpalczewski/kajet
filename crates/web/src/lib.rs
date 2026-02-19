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
    routing::{get, post},
};
use kajet_core::actions::ActionEvent;
use kajet_core::config::EmbeddingConfig;
use kajet_core::traits::Embedder;
use kajet_core::types::AppState;
#[cfg(not(debug_assertions))]
use rust_embed::RustEmbed;
use std::collections::HashMap;
use std::sync::Arc;
use unicode_normalization::UnicodeNormalization;

// ---------------------------------------------------------------------------
// Embedder Factory (duplicated from root crate to avoid circular dependency)
// ---------------------------------------------------------------------------

async fn create_embedder(config: &EmbeddingConfig) -> anyhow::Result<Arc<dyn Embedder>> {
    match config.backend {
        kajet_core::config::EmbeddingBackend::Candle => {
            Ok(Arc::new(kajet_backend::CandleEmbedder::new(&config.model)?))
        }
        kajet_core::config::EmbeddingBackend::Remote => {
            let mut remote_cfg = kajet_remote::RemoteEmbedderConfig {
                base_url: config.base_url.clone(),
                model: config.model.clone(),
                api_key: if config.api_key.is_empty() {
                    None
                } else {
                    Some(config.api_key.clone())
                },
                ..kajet_remote::RemoteEmbedderConfig::default()
            };
            remote_cfg.max_batch_size = config.remote_max_batch_size.max(1);
            remote_cfg.max_input_chars = config.remote_max_input_chars.max(128);
            Ok(Arc::new(
                kajet_remote::RemoteEmbedder::connect_with_config(remote_cfg)
                    .await
                    .map_err(anyhow::Error::new)?,
            ))
        }
    }
}

// In release mode, embed files into binary at compile time
#[cfg(not(debug_assertions))]
#[derive(RustEmbed, Clone)]
#[folder = "../../frontend/dist/"]
struct Assets;

// In debug mode, we'll serve files directly from filesystem (see serve_spa function)

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub async fn serve(state: Arc<AppState>, port: u16) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port)).await?;
    serve_with_listener(state, listener).await
}

pub async fn serve_with_listener(
    state: Arc<AppState>,
    listener: tokio::net::TcpListener,
) -> anyhow::Result<()> {
    let app = Router::new()
        .route("/api/search", get(api_search))
        .route("/api/status", get(api_status))
        .route("/api/config", get(api_config))
        .route("/api/config/schema", get(api_config_schema))
        .route(
            "/api/config/global",
            get(api_config_global_raw).put(api_config_global),
        )
        .route(
            "/api/config/vault",
            get(api_config_vault_raw)
                .put(api_config_vault)
                .delete(api_config_vault_delete),
        )
        .route("/api/i18n", get(api_i18n))
        .route("/api/actions", post(api_actions))
        .route("/api/documents", get(api_documents))
        .route("/api/documents/{*path}", get(api_document_detail))
        .route("/ws", get(ws_handler))
        .fallback(serve_spa)
        .with_state(state);

    axum::serve(listener, app).await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// SPA — serve static assets or index.html as fallback
// ---------------------------------------------------------------------------

#[cfg(debug_assertions)]
async fn serve_spa(uri: axum::http::Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');

    let frontend_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../frontend/dist");

    let file_path = if path.is_empty() {
        frontend_dir.join("index.html")
    } else {
        frontend_dir.join(path)
    };

    match tokio::fs::read(&file_path).await {
        Ok(content) => {
            let mime = mime_guess::from_path(&file_path).first_or_octet_stream();
            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, mime.as_ref().to_string())],
                content,
            )
                .into_response()
        }
        Err(_) if !path.is_empty() => {
            // Fallback to index.html for SPA routing
            match tokio::fs::read(frontend_dir.join("index.html")).await {
                Ok(content) => (
                    StatusCode::OK,
                    [(header::CONTENT_TYPE, "text/html".to_string())],
                    content,
                )
                    .into_response(),
                Err(_) => (StatusCode::NOT_FOUND, "index.html not found").into_response(),
            }
        }
        Err(_) => (StatusCode::NOT_FOUND, "File not found").into_response(),
    }
}

#[cfg(not(debug_assertions))]
async fn serve_spa(uri: axum::http::Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');

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
    "settings_embedding_api_key_hint",
    "settings_embedding_document_prefix",
    "settings_embedding_query_prefix",
    "settings_remote_max_batch_size",
    "settings_remote_max_input_chars",
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
    "logs_tab_logs",
    "logs_tab_queries",
    "query_log_empty",
    "query_results",
    "query_duration",
    "indexing_in_progress",
    "nav_documents",
    "documents_title",
    "documents_search",
    "documents_tag_filter",
    "documents_no_results",
    "documents_modified",
    "documents_of",
    "documents_prev",
    "documents_next",
    "documents_back",
    "documents_path",
    "documents_tags",
    "documents_chunks",
    "documents_outgoing_links",
    "documents_backlinks",
    "documents_content_chunks",
    "chunk_raw",
    "chunk_processed",
    "chunk_links",
    // Schema-driven settings
    "settings_section_general",
    "settings_section_embedding",
    "settings_section_logging",
    "settings_section_writer",
    "settings_section_tree",
    "settings_section_search_tuning",
    "settings_field_general_port",
    "settings_field_general_language",
    "settings_field_general_exclude_folders",
    "settings_field_general_default_limit",
    "settings_field_general_max_concurrent_files",
    "settings_field_general_pipeline_buffer_size",
    "settings_field_general_open_browser",
    "settings_field_general_resolve_wikilinks",
    "settings_field_embedding_backend",
    "settings_field_embedding_model",
    "settings_field_embedding_base_url",
    "settings_field_embedding_api_key",
    "settings_field_embedding_document_prefix",
    "settings_field_embedding_query_prefix",
    "settings_field_embedding_remote_max_batch_size",
    "settings_field_embedding_remote_max_input_chars",
    "settings_field_logging_level",
    "settings_field_logging_file_level",
    "settings_field_logging_dashboard_level",
    "settings_field_logging_progress_percent_step",
    "settings_field_writer_backup_enabled",
    "settings_field_writer_backup_max_per_file",
    "settings_field_writer_timestamps_enabled",
    "settings_field_writer_timestamps_created_field",
    "settings_field_writer_timestamps_modified_field",
    "settings_field_writer_timestamps_format",
    "settings_field_writer_timestamps_timezone",
    "settings_field_writer_frontmatter_default_tags",
    "settings_field_writer_frontmatter_created_date_field",
    "settings_field_writer_frontmatter_modified_date_field",
    "settings_field_tree_depth",
    "settings_field_tree_size",
    "settings_field_tree_max_chars",
    "settings_field_search_tuning_filter_overfetch_multiplier",
    "settings_field_search_tuning_tags_only_fetch_limit",
];

pub async fn api_i18n() -> Json<HashMap<String, String>> {
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
pub struct SearchQuery {
    q: String,
    #[serde(default = "default_limit")]
    limit: usize,
}

fn default_limit() -> usize {
    10
}

pub async fn api_search(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SearchQuery>,
) -> impl IntoResponse {
    let start = std::time::Instant::now();

    match state
        .search_engine
        .hybrid_search(&params.q, params.limit)
        .await
    {
        Ok(results) => {
            let duration_ms = start.elapsed().as_millis() as u64;

            // Emit QueryExecuted event to action bus
            let _ = state.action_bus.send(ActionEvent::QueryExecuted {
                query: params.q.clone(),
                results: results
                    .iter()
                    .take(5)
                    .map(|r| kajet_core::actions::SearchResultSummary {
                        note_path: r.note_path.clone(),
                        breadcrumb: r.breadcrumb.clone(),
                        content_preview: r.content.chars().take(200).collect(),
                        score: r.score as f64,
                        chunk_index: r.chunk_index,
                    })
                    .collect(),
                duration_ms,
                timestamp: chrono::Utc::now().to_rfc3339(),
            });

            Json(results).into_response()
        }
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
pub struct VaultStatus {
    vault_path: String,
    note_count: usize,
    chunk_count: usize,
    model: String,
    language: String,
    indexing: bool,
}

pub async fn api_status(State(state): State<Arc<AppState>>) -> Json<VaultStatus> {
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

pub async fn api_config(
    State(state): State<Arc<AppState>>,
) -> Json<kajet_core::config::KajetConfig> {
    let mut config = state.config.read().unwrap().clone();
    // Treat API key as write-only for dashboard clients.
    config.embedding.api_key.clear();
    Json(config)
}

// ---------------------------------------------------------------------------
// Config Schema API — returns schema metadata for dynamic UI
// ---------------------------------------------------------------------------

pub async fn api_config_schema(
    State(state): State<Arc<AppState>>,
) -> Json<kajet_core::schema::ConfigSchema> {
    let config = state.config.read().unwrap().clone();
    Json(kajet_core::schema::build_config_schema(&config))
}

// ---------------------------------------------------------------------------
// Config API — write global
// ---------------------------------------------------------------------------

#[derive(serde::Deserialize)]
pub struct ConfigUpdateRequest {
    updates: HashMap<String, toml::Value>,
}

#[derive(Debug, serde::Deserialize)]
pub struct ConfigDeleteRequest {
    fields: Vec<String>,
}

pub async fn api_config_global(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ConfigUpdateRequest>,
) -> impl IntoResponse {
    // Capture old embedding config for change detection
    let old_embedding = state.config.read().unwrap().embedding.clone();

    if let Err(e) = kajet_core::config::write_global_config(&body.updates) {
        return (StatusCode::BAD_REQUEST, e.to_string()).into_response();
    }

    // Reload config into RwLock
    let (port, cli_lang) = (state.cli_args.port, state.cli_args.language.clone());
    match kajet_core::config::reload_config(&state.db_path, port, cli_lang) {
        Ok(new_cfg) => {
            // Detect embedding config change (backend + model only - these affect vector compatibility)
            let embedding_changed = old_embedding.backend != new_cfg.embedding.backend
                || old_embedding.model != new_cfg.embedding.model;

            // If language changed, update locale
            let new_lang = new_cfg.language.clone();
            *state.config.write().unwrap() = new_cfg.clone();
            rust_i18n::set_locale(&new_lang);

            // If embedding changed, spawn task to swap embedder + reindex
            if embedding_changed {
                let s = state.clone();
                let new_embedding_cfg = new_cfg.embedding.clone();
                let similarity_graph_cfg = new_cfg.similarity_graph.clone();
                // Read exclude_folders before spawning to avoid std::sync::RwLock in async context
                let exclude_folders = state
                    .config
                    .read()
                    .expect("config lock poisoned")
                    .exclude_folders
                    .clone();

                tokio::spawn(async move {
                    match create_embedder(&new_embedding_cfg).await {
                        Ok(new_embedder) => {
                            // Swap embedder in SearchEngine
                            s.search_engine.swap_embedder(new_embedder.clone()).await;

                            // Swap indexer with new embedder
                            let new_indexer = Arc::new(
                                kajet_indexer::Indexer::new(
                                    new_embedder,
                                    s.search_engine.store().clone(),
                                    s.search_engine.doc_store().clone(),
                                )
                                .with_db_path(s.db_path.clone())
                                .with_similarity_graph_config(similarity_graph_cfg)
                                .with_concurrency(
                                    new_embedding_cfg.remote_max_batch_size.max(1),
                                    256,
                                )
                                .with_document_prefix(new_embedding_cfg.document_prefix.clone()),
                            );
                            *s.indexer.write().await = new_indexer.clone();

                            // Emit EmbedderSwapped event
                            let backend_str = match new_embedding_cfg.backend {
                                kajet_core::config::EmbeddingBackend::Candle => "candle",
                                kajet_core::config::EmbeddingBackend::Remote => "remote",
                            };
                            let _ = s.action_bus.send(ActionEvent::EmbedderSwapped {
                                new_backend: backend_str.to_string(),
                                new_model: new_embedding_cfg.model.clone(),
                            });

                            // Trigger full reindex
                            let action_id = uuid::Uuid::new_v4().to_string();
                            let _ = s.action_bus.send(ActionEvent::IndexStarted {
                                action_id: action_id.clone(),
                                mode: kajet_core::actions::IndexMode::Full,
                            });

                            let vault = std::path::Path::new(&s.vault_path);
                            match new_indexer.full_reindex(vault, &exclude_folders).await {
                                Ok(stats) => {
                                    s.note_count.store(
                                        stats.total_documents,
                                        std::sync::atomic::Ordering::Relaxed,
                                    );
                                    s.chunk_count.store(
                                        stats.total_chunks,
                                        std::sync::atomic::Ordering::Relaxed,
                                    );
                                    let _ = s
                                        .action_bus
                                        .send(ActionEvent::IndexCompleted { action_id, stats });
                                }
                                Err(e) => {
                                    tracing::error!(error = %e, "Reindex after embedder swap failed");
                                    let _ = s.action_bus.send(ActionEvent::IndexFailed {
                                        action_id,
                                        error: e.to_string(),
                                    });
                                }
                            }
                        }
                        Err(e) => {
                            tracing::error!(error = %e, "Failed to create new embedder");
                            // Emit error event so frontend knows swap failed
                            let action_id = uuid::Uuid::new_v4().to_string();
                            let _ = s.action_bus.send(ActionEvent::IndexFailed {
                                action_id,
                                error: format!("Failed to create embedder: {}", e),
                            });
                        }
                    }
                });
            }

            StatusCode::OK.into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

// ---------------------------------------------------------------------------
// Config API — write vault
// ---------------------------------------------------------------------------

pub async fn api_config_vault(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ConfigUpdateRequest>,
) -> impl IntoResponse {
    // Capture old embedding config for change detection
    let old_embedding = state.config.read().unwrap().embedding.clone();

    if let Err(e) = kajet_core::config::write_vault_config(&state.db_path, &body.updates) {
        return (StatusCode::BAD_REQUEST, e.to_string()).into_response();
    }

    // Reload config into RwLock
    let (port, cli_lang) = (state.cli_args.port, state.cli_args.language.clone());
    match kajet_core::config::reload_config(&state.db_path, port, cli_lang) {
        Ok(new_cfg) => {
            // Detect embedding config change (backend + model only - these affect vector compatibility)
            let embedding_changed = old_embedding.backend != new_cfg.embedding.backend
                || old_embedding.model != new_cfg.embedding.model;

            *state.config.write().unwrap() = new_cfg.clone();

            // If embedding changed, spawn task to swap embedder + reindex
            if embedding_changed {
                let s = state.clone();
                let new_embedding_cfg = new_cfg.embedding.clone();
                let similarity_graph_cfg = new_cfg.similarity_graph.clone();
                // Read exclude_folders before spawning to avoid std::sync::RwLock in async context
                let exclude_folders = state
                    .config
                    .read()
                    .expect("config lock poisoned")
                    .exclude_folders
                    .clone();

                tokio::spawn(async move {
                    match create_embedder(&new_embedding_cfg).await {
                        Ok(new_embedder) => {
                            // Swap embedder in SearchEngine
                            s.search_engine.swap_embedder(new_embedder.clone()).await;

                            // Swap indexer with new embedder
                            let new_indexer = Arc::new(
                                kajet_indexer::Indexer::new(
                                    new_embedder,
                                    s.search_engine.store().clone(),
                                    s.search_engine.doc_store().clone(),
                                )
                                .with_db_path(s.db_path.clone())
                                .with_similarity_graph_config(similarity_graph_cfg)
                                .with_concurrency(
                                    new_embedding_cfg.remote_max_batch_size.max(1),
                                    256,
                                )
                                .with_document_prefix(new_embedding_cfg.document_prefix.clone()),
                            );
                            *s.indexer.write().await = new_indexer.clone();

                            // Emit EmbedderSwapped event
                            let backend_str = match new_embedding_cfg.backend {
                                kajet_core::config::EmbeddingBackend::Candle => "candle",
                                kajet_core::config::EmbeddingBackend::Remote => "remote",
                            };
                            let _ = s.action_bus.send(ActionEvent::EmbedderSwapped {
                                new_backend: backend_str.to_string(),
                                new_model: new_embedding_cfg.model.clone(),
                            });

                            // Trigger full reindex
                            let action_id = uuid::Uuid::new_v4().to_string();
                            let _ = s.action_bus.send(ActionEvent::IndexStarted {
                                action_id: action_id.clone(),
                                mode: kajet_core::actions::IndexMode::Full,
                            });

                            let vault = std::path::Path::new(&s.vault_path);
                            match new_indexer.full_reindex(vault, &exclude_folders).await {
                                Ok(stats) => {
                                    s.note_count.store(
                                        stats.total_documents,
                                        std::sync::atomic::Ordering::Relaxed,
                                    );
                                    s.chunk_count.store(
                                        stats.total_chunks,
                                        std::sync::atomic::Ordering::Relaxed,
                                    );
                                    let _ = s
                                        .action_bus
                                        .send(ActionEvent::IndexCompleted { action_id, stats });
                                }
                                Err(e) => {
                                    tracing::error!(error = %e, "Reindex after embedder swap failed");
                                    let _ = s.action_bus.send(ActionEvent::IndexFailed {
                                        action_id,
                                        error: e.to_string(),
                                    });
                                }
                            }
                        }
                        Err(e) => {
                            tracing::error!(error = %e, "Failed to create new embedder");
                            // Emit error event so frontend knows swap failed
                            let action_id = uuid::Uuid::new_v4().to_string();
                            let _ = s.action_bus.send(ActionEvent::IndexFailed {
                                action_id,
                                error: format!("Failed to create embedder: {}", e),
                            });
                        }
                    }
                });
            }

            StatusCode::OK.into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

/// GET /api/config/vault - returns raw vault config without merging
pub async fn api_config_vault_raw(State(state): State<Arc<AppState>>) -> Json<toml::Table> {
    match kajet_core::config::read_vault_config(&state.db_path) {
        Ok(table) => Json(table),
        Err(e) => {
            tracing::error!(error = %e, "Failed to read vault config");
            Json(toml::Table::new())
        }
    }
}

/// GET /api/config/global - returns raw global config without merging
pub async fn api_config_global_raw() -> Json<toml::Table> {
    match kajet_core::config::read_global_config() {
        Ok(table) => Json(table),
        Err(e) => {
            tracing::error!(error = %e, "Failed to read global config");
            Json(toml::Table::new())
        }
    }
}

/// DELETE /api/config/vault - delete specified fields from vault config
pub async fn api_config_vault_delete(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ConfigDeleteRequest>,
) -> impl IntoResponse {
    if let Err(e) = kajet_core::config::delete_vault_config_fields(&state.db_path, &body.fields) {
        tracing::error!(error = %e, fields = ?body.fields, "Failed to delete vault config fields");
        return (StatusCode::BAD_REQUEST, e.to_string()).into_response();
    }

    // Reload config into RwLock
    let (port, cli_lang) = (state.cli_args.port, state.cli_args.language.clone());
    match kajet_core::config::reload_config(&state.db_path, port, cli_lang) {
        Ok(new_cfg) => {
            *state.config.write().unwrap() = new_cfg;
            StatusCode::OK.into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to reload config after deletion");
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

// ---------------------------------------------------------------------------
// Actions API — trigger actions (reindex, refresh stats)
// ---------------------------------------------------------------------------

pub async fn api_actions(
    State(state): State<Arc<AppState>>,
    Json(request): Json<kajet_core::actions::ActionRequest>,
) -> Json<kajet_core::actions::ActionResponse> {
    use kajet_core::actions::*;

    let action_id = uuid::Uuid::new_v4().to_string();

    match request {
        ActionRequest::Reindex { path } => {
            let id = action_id.clone();
            let s = state.clone();
            tokio::spawn(async move {
                let mode = match &path {
                    Some(p) => IndexMode::SingleFile(p.clone()),
                    None => IndexMode::Full,
                };
                let _ = s.action_bus.send(ActionEvent::IndexStarted {
                    action_id: id.clone(),
                    mode,
                });

                let vault = std::path::Path::new(&s.vault_path);
                let indexer = s.indexer.read().await.clone();
                let result = match path {
                    Some(ref p) => indexer
                        .reindex_files(vault, std::slice::from_ref(p))
                        .await
                        .map(|_| None),
                    None => {
                        let exclude = s.config.read().unwrap().exclude_folders.clone();
                        indexer.full_reindex(vault, &exclude).await.map(Some)
                    }
                };

                match result {
                    Ok(stats) => {
                        if let Some(stats) = stats {
                            s.note_count
                                .store(stats.total_documents, std::sync::atomic::Ordering::Relaxed);
                            s.chunk_count
                                .store(stats.total_chunks, std::sync::atomic::Ordering::Relaxed);
                            let _ = s.action_bus.send(ActionEvent::IndexCompleted {
                                action_id: id,
                                stats,
                            });
                        }
                        let _ = s.action_bus.send(ActionEvent::StatsUpdated {
                            note_count: s.note_count.load(std::sync::atomic::Ordering::Relaxed),
                            chunk_count: s.chunk_count.load(std::sync::atomic::Ordering::Relaxed),
                        });
                    }
                    Err(e) => {
                        let _ = s.action_bus.send(ActionEvent::IndexFailed {
                            action_id: id,
                            error: e.to_string(),
                        });
                    }
                }
            });
        }
        ActionRequest::RefreshStats => {
            let indexer = state.indexer.read().await.clone();
            if let Ok(stats) = indexer.get_index_stats().await {
                state
                    .note_count
                    .store(stats.total_documents, std::sync::atomic::Ordering::Relaxed);
                state
                    .chunk_count
                    .store(stats.total_chunks, std::sync::atomic::Ordering::Relaxed);
                let _ = state.action_bus.send(ActionEvent::StatsUpdated {
                    note_count: stats.total_documents,
                    chunk_count: stats.total_chunks,
                });
            }
        }
    }

    Json(kajet_core::actions::ActionResponse { action_id })
}

// ---------------------------------------------------------------------------
// Documents API — list documents with filtering/pagination
// ---------------------------------------------------------------------------

#[derive(serde::Deserialize)]
pub struct DocumentsQuery {
    #[serde(default)]
    search: Option<String>,
    #[serde(default)]
    tag: Option<String>,
    #[serde(default = "default_documents_limit")]
    limit: usize,
    #[serde(default)]
    offset: usize,
}

fn default_documents_limit() -> usize {
    50
}

pub async fn api_documents(
    State(state): State<Arc<AppState>>,
    Query(params): Query<DocumentsQuery>,
) -> impl IntoResponse {
    match state.search_engine.doc_store().get_all_documents().await {
        Ok(mut docs) => {
            // Filter by search query (title substring match)
            if let Some(ref search_term) = params.search {
                let search_lower = search_term.to_lowercase();
                docs.retain(|d| d.title.to_lowercase().contains(&search_lower));
            }

            // Filter by tag (exact match)
            if let Some(ref tag) = params.tag {
                docs.retain(|d| d.tags.contains(tag));
            }

            let total = docs.len();

            // Sort by last_modified descending (newest first)
            docs.sort_by(|a, b| {
                b.last_modified
                    .partial_cmp(&a.last_modified)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            // Apply pagination
            let documents: Vec<kajet_core::web_types::DocumentSummary> = docs
                .into_iter()
                .skip(params.offset)
                .take(params.limit)
                .map(|doc| {
                    // Count chunks for this document by querying the store
                    // Note: This is async, so we'll need to handle it differently
                    kajet_core::web_types::DocumentSummary {
                        source_file: doc.source_file.clone(),
                        title: doc.title,
                        tags: doc.tags,
                        chunk_count: 0, // Will be populated below
                        last_modified: doc.last_modified,
                    }
                })
                .collect();

            // Populate chunk counts asynchronously
            let mut summaries_with_counts = Vec::new();
            for mut summary in documents {
                match state
                    .search_engine
                    .store()
                    .get_chunks_by_path(&summary.source_file)
                    .await
                {
                    Ok(chunks) => {
                        summary.chunk_count = chunks.len();
                        summaries_with_counts.push(summary);
                    }
                    Err(e) => {
                        tracing::warn!(
                            path = %summary.source_file,
                            error = %e,
                            "Failed to get chunk count"
                        );
                        summaries_with_counts.push(summary);
                    }
                }
            }

            Json(kajet_core::web_types::DocumentListResponse {
                documents: summaries_with_counts,
                total,
            })
            .into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to get documents");
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

// ---------------------------------------------------------------------------
// Document Detail API — get document with all chunks
// ---------------------------------------------------------------------------

pub async fn api_document_detail(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(path): axum::extract::Path<String>,
) -> impl IntoResponse {
    // Remove leading slash if present
    let path = path.strip_prefix('/').unwrap_or(&path);
    // NFC-normalize so Polish chars (ł, ę, ą …) match the indexed form
    let path: String = path.nfc().collect();

    match state
        .search_engine
        .doc_store()
        .get_document_by_path(&path)
        .await
    {
        Ok(Some(document)) => {
            match state
                .search_engine
                .store()
                .get_chunks_by_path(&document.source_file)
                .await
            {
                Ok(stored_chunks) => {
                    let chunks: Vec<kajet_core::web_types::ChunkDetail> = stored_chunks
                        .into_iter()
                        .map(|chunk| kajet_core::web_types::ChunkDetail {
                            chunk_index: chunk.chunk_index,
                            breadcrumb: chunk.breadcrumb,
                            content: chunk.content.clone(),
                            raw_content: chunk.raw_content,
                            links: chunk.links,
                            char_count: chunk.content.chars().count(),
                        })
                        .collect();

                    Json(kajet_core::web_types::DocumentDetail { document, chunks }).into_response()
                }
                Err(e) => {
                    tracing::error!(error = %e, path = %path, "Failed to get chunks");
                    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
                }
            }
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            format!("Document not found: {}", path),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, path = %path, "Failed to get document");
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
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
        let event = ActionEvent::LogEntry {
            level: entry.level.clone(),
            message: entry.message.clone(),
            timestamp: entry.timestamp.to_rfc3339(),
            target: entry.target.clone(),
            fields: entry.fields.clone(),
        };
        let json = serde_json::to_string(&event).unwrap_or_default();
        if socket.send(Message::Text(json.into())).await.is_err() {
            return;
        }
    }

    // Send current stats on connect
    let stats_event = ActionEvent::StatsUpdated {
        note_count: state.note_count.load(std::sync::atomic::Ordering::Relaxed),
        chunk_count: state.chunk_count.load(std::sync::atomic::Ordering::Relaxed),
    };
    let json = serde_json::to_string(&stats_event).unwrap_or_default();
    if socket.send(Message::Text(json.into())).await.is_err() {
        return;
    }

    let mut rx = state.action_bus.subscribe();

    loop {
        match rx.recv().await {
            Ok(event) => {
                let json = serde_json::to_string(&event).unwrap_or_default();
                if socket.send(Message::Text(json.into())).await.is_err() {
                    break;
                }
            }
            Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                tracing::warn!(skipped = n, "WebSocket client lagged");
            }
            Err(_) => break,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use kajet_core::actions::{ActionRequest, IndexMode};
    use kajet_core::config::KajetConfig;
    use kajet_core::logging::broadcast_layer::LogBuffer;
    use kajet_core::search::SearchEngine;
    use kajet_core::types::{CliArgs, IndexStats};
    use std::sync::RwLock;
    use std::sync::atomic::{AtomicBool, AtomicUsize};
    use tokio::sync::broadcast;

    /// Mock IndexerHandle for testing
    struct MockIndexer {
        stats: IndexStats,
    }

    impl MockIndexer {
        fn new() -> Self {
            Self {
                stats: IndexStats {
                    total_documents: 42,
                    total_chunks: 100,
                    last_indexed: Some(chrono::Utc::now()),
                },
            }
        }
    }

    #[async_trait::async_trait]
    impl kajet_core::types::IndexerHandle for MockIndexer {
        async fn full_reindex(
            &self,
            _vault_path: &std::path::Path,
            _exclude_folders: &[String],
        ) -> anyhow::Result<IndexStats> {
            Ok(self.stats.clone())
        }

        async fn reindex_files(
            &self,
            _vault_path: &std::path::Path,
            _rel_paths: &[String],
        ) -> anyhow::Result<()> {
            Ok(())
        }

        async fn get_index_stats(&self) -> anyhow::Result<IndexStats> {
            Ok(self.stats.clone())
        }
    }

    fn create_test_state() -> Arc<AppState> {
        let (action_tx, _) = broadcast::channel::<ActionEvent>(100);
        let log_buffer = Arc::new(LogBuffer::__test_new());
        let temp_dir = std::env::temp_dir().join(format!("kajet-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        Arc::new(AppState {
            search_engine: SearchEngine::__test_new(),
            action_bus: action_tx,
            log_buffer,
            cli_args: CliArgs {
                port: 3579,
                language: None,
                model: None,
            },
            config: RwLock::new(KajetConfig::default()),
            vault_path: temp_dir.to_string_lossy().to_string(),
            db_path: temp_dir.clone(),
            note_count: AtomicUsize::new(0),
            chunk_count: AtomicUsize::new(0),
            indexing: AtomicBool::new(false),
            indexer: Arc::new(tokio::sync::RwLock::new(Arc::new(MockIndexer::new()))),
        })
    }

    #[tokio::test]
    async fn test_api_actions_reindex_full() {
        let state = create_test_state();
        let mut rx = state.action_bus.subscribe();

        let request = ActionRequest::Reindex { path: None };
        let Json(action_response) = api_actions(State(state.clone()), Json(request)).await;
        assert!(!action_response.action_id.is_empty());

        // Wait for background task to emit events
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Should receive IndexStarted and eventually IndexCompleted
        let mut received_started = false;
        let mut received_completed = false;

        while let Ok(event) = rx.try_recv() {
            match event {
                ActionEvent::IndexStarted { mode, .. } => {
                    assert!(matches!(mode, IndexMode::Full));
                    received_started = true;
                }
                ActionEvent::IndexCompleted { stats, .. } => {
                    assert_eq!(stats.total_documents, 42);
                    assert_eq!(stats.total_chunks, 100);
                    received_completed = true;
                }
                ActionEvent::StatsUpdated {
                    note_count,
                    chunk_count,
                } => {
                    assert_eq!(note_count, 42);
                    assert_eq!(chunk_count, 100);
                }
                _ => {}
            }
        }

        assert!(received_started, "Should emit IndexStarted");
        assert!(received_completed, "Should emit IndexCompleted");
    }

    #[tokio::test]
    async fn test_api_actions_reindex_single_file() {
        let state = create_test_state();
        let mut rx = state.action_bus.subscribe();

        let request = ActionRequest::Reindex {
            path: Some("test.md".to_string()),
        };
        let Json(action_response) = api_actions(State(state.clone()), Json(request)).await;
        assert!(!action_response.action_id.is_empty());

        // Wait for background task to emit events
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Should receive IndexStarted with SingleFile mode
        let mut received_started = false;

        while let Ok(event) = rx.try_recv() {
            if let ActionEvent::IndexStarted { mode, .. } = event {
                assert!(matches!(mode, IndexMode::SingleFile(_)));
                received_started = true;
            }
        }

        assert!(received_started, "Should emit IndexStarted");
    }

    #[tokio::test]
    async fn test_api_actions_refresh_stats() {
        let state = create_test_state();
        let mut rx = state.action_bus.subscribe();

        let request = ActionRequest::RefreshStats;
        let Json(action_response) = api_actions(State(state.clone()), Json(request)).await;
        assert!(!action_response.action_id.is_empty());

        // Should immediately emit StatsUpdated
        let event = rx.try_recv().expect("Should receive StatsUpdated");
        match event {
            ActionEvent::StatsUpdated {
                note_count,
                chunk_count,
            } => {
                assert_eq!(note_count, 42);
                assert_eq!(chunk_count, 100);
            }
            _ => panic!("Expected StatsUpdated event"),
        }

        // Verify state was updated
        assert_eq!(
            state.note_count.load(std::sync::atomic::Ordering::Relaxed),
            42
        );
        assert_eq!(
            state.chunk_count.load(std::sync::atomic::Ordering::Relaxed),
            100
        );
    }

    #[tokio::test]
    async fn test_api_actions_returns_unique_action_id() {
        let state = create_test_state();

        let request1 = ActionRequest::RefreshStats;
        let Json(action_response1) = api_actions(State(state.clone()), Json(request1)).await;

        let request2 = ActionRequest::RefreshStats;
        let Json(action_response2) = api_actions(State(state.clone()), Json(request2)).await;

        assert_ne!(
            action_response1.action_id, action_response2.action_id,
            "Action IDs should be unique"
        );
    }
}
