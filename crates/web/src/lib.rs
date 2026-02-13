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
    routing::{get, post, put},
};
use kajet_core::actions::ActionEvent;
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
        .route("/api/config/schema", get(api_config_schema))
        .route("/api/config/global", put(api_config_global))
        .route("/api/config/vault", put(api_config_vault))
        .route("/api/i18n", get(api_i18n))
        .route("/api/actions", post(api_actions))
        .route("/api/documents", get(api_documents))
        .route("/api/documents/*path", get(api_document_detail))
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
    let mut config = state.config.read().unwrap().clone();
    // Treat API key as write-only for dashboard clients.
    config.embedding.api_key.clear();
    Json(config)
}

// ---------------------------------------------------------------------------
// Config Schema API — returns schema metadata for dynamic UI
// ---------------------------------------------------------------------------

async fn api_config_schema(
    State(state): State<Arc<AppState>>,
) -> Json<kajet_core::schema::ConfigSchema> {
    let config = state.config.read().unwrap().clone();
    Json(kajet_core::schema::build_config_schema(&config))
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
    let (port, cli_lang) = (state.cli_args.port, state.cli_args.language.clone());
    match kajet_core::config::reload_config(&state.db_path, port, cli_lang) {
        Ok(mut new_cfg) => {
            // Apply CLI model override if present
            if let Some(ref model) = state.cli_args.model {
                new_cfg.embedding.model = model.clone();
            }
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
    let (port, cli_lang) = (state.cli_args.port, state.cli_args.language.clone());
    match kajet_core::config::reload_config(&state.db_path, port, cli_lang) {
        Ok(mut new_cfg) => {
            // Apply CLI model override if present
            if let Some(ref model) = state.cli_args.model {
                new_cfg.embedding.model = model.clone();
            }
            *state.config.write().unwrap() = new_cfg;
            StatusCode::OK.into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

// ---------------------------------------------------------------------------
// Actions API — trigger actions (reindex, refresh stats)
// ---------------------------------------------------------------------------

async fn api_actions(
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
                let result = match path {
                    Some(ref p) => s
                        .indexer
                        .reindex_files(vault, std::slice::from_ref(p))
                        .await
                        .map(|_| None),
                    None => {
                        let exclude = s.config.read().unwrap().exclude_folders.clone();
                        s.indexer.full_reindex(vault, &exclude).await.map(Some)
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
            if let Ok(stats) = state.indexer.get_index_stats().await {
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
struct DocumentsQuery {
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

async fn api_documents(
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

async fn api_document_detail(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(path): axum::extract::Path<String>,
) -> impl IntoResponse {
    // Remove leading slash if present
    let path = path.strip_prefix('/').unwrap_or(&path);

    match state
        .search_engine
        .doc_store()
        .get_document_by_path(path)
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
            indexer: Arc::new(MockIndexer::new()),
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
