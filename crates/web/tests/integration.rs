use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt as _;
use kajet_core::actions::{ActionEvent, ActionRequest};
use kajet_core::config::KajetConfig;
use kajet_core::logging::broadcast_layer::LogBuffer;
use kajet_core::search::SearchEngine;
use kajet_core::traits::mocks::{MockDocumentStore, MockEmbedder, MockVectorStore};
use kajet_core::traits::{DocumentStore, SearchHit, StoredChunk, VectorStore};
use kajet_core::types::{AppState, CliArgs, Document, FtsHit, IndexStats};
use std::sync::atomic::{AtomicBool, AtomicUsize};
use std::sync::{Arc, RwLock};
use tokio::sync::broadcast;
use tower::util::ServiceExt as _;

/// Mock indexer for tests
struct MockIndexer;

#[async_trait::async_trait]
impl kajet_core::types::IndexerHandle for MockIndexer {
    async fn full_reindex(
        &self,
        _vault_path: &std::path::Path,
        _exclude_folders: &[String],
    ) -> anyhow::Result<IndexStats> {
        Ok(IndexStats {
            total_documents: 2,
            total_chunks: 2,
            last_indexed: Some(chrono::Utc::now()),
        })
    }

    async fn reindex_files(
        &self,
        _vault_path: &std::path::Path,
        _rel_paths: &[String],
    ) -> anyhow::Result<()> {
        Ok(())
    }

    async fn get_index_stats(&self) -> anyhow::Result<IndexStats> {
        Ok(IndexStats {
            total_documents: 2,
            total_chunks: 2,
            last_indexed: Some(chrono::Utc::now()),
        })
    }
}

/// Helper function to build test AppState with mocks
async fn test_app_state() -> Arc<AppState> {
    let (action_tx, _) = broadcast::channel::<ActionEvent>(16);
    let log_buffer = Arc::new(LogBuffer::__test_new());
    let temp_dir = std::env::temp_dir().join(format!("kajet-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir).unwrap();

    // Create mock stores with test data
    let mock_embedder = Arc::new(MockEmbedder::new(384));
    let mock_vector_store = Arc::new(MockVectorStore::new());
    let mock_doc_store = Arc::new(MockDocumentStore::new());

    // Pre-populate with test documents
    let test_docs = vec![
        Document {
            source_file: "test1.md".to_string(),
            full_text: "This is a test document".to_string(),
            title: "Test Document 1".to_string(),
            tags: vec!["test".to_string()],
            content_hash: "hash1".to_string(),
            last_modified: 1234567890.0,
            outgoing_links: vec!["test2.md".to_string()],
            backlinks: vec![],
        },
        Document {
            source_file: "test2.md".to_string(),
            full_text: "Another test document".to_string(),
            title: "Test Document 2".to_string(),
            tags: vec!["test".to_string(), "demo".to_string()],
            content_hash: "hash2".to_string(),
            last_modified: 1234567891.0,
            outgoing_links: vec![],
            backlinks: vec!["test1.md".to_string()],
        },
    ];

    mock_doc_store.store_documents(&test_docs).await.unwrap();

    // Add test chunks to vector store
    let test_chunks = vec![
        StoredChunk {
            note_path: "test1.md".to_string(),
            breadcrumb: "test1.md > Heading".to_string(),
            content: "This is a test document".to_string(),
            raw_content: "This is a test document".to_string(),
            vector: vec![0.1; 384],
            chunk_index: 0,
            content_hash: "chunk_hash1".to_string(),
            links: vec![],
        },
        StoredChunk {
            note_path: "test2.md".to_string(),
            breadcrumb: "test2.md".to_string(),
            content: "Another test document".to_string(),
            raw_content: "Another test document".to_string(),
            vector: vec![0.2; 384],
            chunk_index: 0,
            content_hash: "chunk_hash2".to_string(),
            links: vec![],
        },
    ];
    mock_vector_store.store_chunks(&test_chunks).await.unwrap();

    let search_engine = SearchEngine::new(mock_embedder, mock_vector_store, mock_doc_store);

    Arc::new(AppState {
        search_engine,
        action_bus: action_tx,
        log_buffer,
        cli_args: CliArgs {
            port: 3579,
            language: Some("en".to_string()),
            model: None,
        },
        config: RwLock::new(KajetConfig::default()),
        vault_path: temp_dir.to_string_lossy().to_string(),
        db_path: temp_dir.clone(),
        note_count: AtomicUsize::new(2),
        chunk_count: AtomicUsize::new(2),
        indexing: AtomicBool::new(false),
        indexer: Arc::new(tokio::sync::RwLock::new(Arc::new(MockIndexer))),
    })
}

/// Helper to create test router (duplicates logic from serve() function)
fn test_router(state: Arc<AppState>) -> Router {
    use axum::routing::{get, post, put};

    Router::new()
        .route("/api/search", get(kajet_web::api_search))
        .route("/api/status", get(kajet_web::api_status))
        .route("/api/config", get(kajet_web::api_config))
        .route("/api/config/schema", get(kajet_web::api_config_schema))
        .route("/api/config/global", put(kajet_web::api_config_global))
        .route("/api/config/vault", put(kajet_web::api_config_vault))
        .route("/api/i18n", get(kajet_web::api_i18n))
        .route("/api/actions", post(kajet_web::api_actions))
        .route("/api/documents", get(kajet_web::api_documents))
        .route(
            "/api/documents/{*path}",
            get(kajet_web::api_document_detail),
        )
        .with_state(state)
}

#[tokio::test]
async fn test_documents_endpoint_returns_list() {
    let state = test_app_state().await;
    let app = test_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/documents")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["total"], 2);
    assert_eq!(json["documents"].as_array().unwrap().len(), 2);

    // Verify documents are sorted by last_modified descending (newest first)
    let docs = json["documents"].as_array().unwrap();
    assert_eq!(docs[0]["source_file"], "test2.md");
    assert_eq!(docs[1]["source_file"], "test1.md");
}

#[tokio::test]
async fn test_documents_endpoint_filters_by_search() {
    let state = test_app_state().await;
    let app = test_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/documents?search=Document%201")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["total"], 1);
    let docs = json["documents"].as_array().unwrap();
    assert_eq!(docs[0]["source_file"], "test1.md");
}

#[tokio::test]
async fn test_documents_endpoint_filters_by_tag() {
    let state = test_app_state().await;
    let app = test_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/documents?tag=demo")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["total"], 1);
    let docs = json["documents"].as_array().unwrap();
    assert_eq!(docs[0]["source_file"], "test2.md");
}

#[tokio::test]
async fn test_document_detail_endpoint() {
    let state = test_app_state().await;
    let app = test_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/documents/test1.md")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["document"]["source_file"], "test1.md");
    assert_eq!(json["document"]["title"], "Test Document 1");
    assert_eq!(json["chunks"].as_array().unwrap().len(), 1);
    assert_eq!(json["chunks"][0]["content"], "This is a test document");
}

#[tokio::test]
async fn test_document_detail_not_found() {
    let state = test_app_state().await;
    let app = test_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/documents/nonexistent.md")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_config_schema_endpoint() {
    let state = test_app_state().await;
    let app = test_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/config/schema")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    // Verify schema has expected structure
    assert!(
        json["sections"].is_array(),
        "Schema should have sections array"
    );
    let sections = json["sections"].as_array().unwrap();
    assert!(
        !sections.is_empty(),
        "Schema should have at least one section"
    );

    // Verify each section has key and fields
    for section in sections {
        assert!(section["key"].is_string(), "Section should have key");
        assert!(
            section["fields"].is_array(),
            "Section should have fields array"
        );
    }
}

#[tokio::test]
async fn test_actions_endpoint_reindex() {
    let state = test_app_state().await;
    let mut rx = state.action_bus.subscribe();
    let app = test_router(state.clone());

    let request_body = serde_json::to_string(&ActionRequest::Reindex { path: None }).unwrap();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/actions")
                .header("content-type", "application/json")
                .body(Body::from(request_body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    // Should return action_id
    assert!(json["action_id"].is_string());
    let action_id = json["action_id"].as_str().unwrap();
    assert!(!action_id.is_empty());

    // Wait for background task to emit events
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Should receive IndexStarted event
    let mut received_started = false;
    while let Ok(event) = rx.try_recv() {
        if matches!(event, ActionEvent::IndexStarted { .. }) {
            received_started = true;
            break;
        }
    }
    assert!(received_started, "Should emit IndexStarted event");
}

#[tokio::test]
async fn test_search_endpoint_returns_results() {
    // Build test state with pre-populated search results
    let (action_tx, _) = broadcast::channel::<ActionEvent>(16);
    let log_buffer = Arc::new(LogBuffer::__test_new());
    let temp_dir = std::env::temp_dir().join(format!("kajet-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let mock_embedder = Arc::new(MockEmbedder::new(384));

    // Create vector store with search results
    let search_hits = vec![SearchHit {
        note_path: "test1.md".to_string(),
        breadcrumb: "test1.md > Heading".to_string(),
        content: "This is a test document".to_string(),
        raw_content: "This is a test document".to_string(),
        links: vec![],
        distance: 0.5,
        chunk_index: 0,
    }];
    let mock_vector_store = Arc::new(MockVectorStore::with_search_results(search_hits));

    // Create doc store with FTS results
    let fts_hits = vec![FtsHit {
        source_file: "test1.md".to_string(),
        title: "Test Document 1".to_string(),
        content_snippet: "This is a test document".to_string(),
        score: 1.0,
    }];
    let mock_doc_store = Arc::new(MockDocumentStore::with_fts_results(fts_hits));

    let search_engine = SearchEngine::new(mock_embedder, mock_vector_store, mock_doc_store);

    // Mock indexer
    struct MockIndexer;

    #[async_trait::async_trait]
    impl kajet_core::types::IndexerHandle for MockIndexer {
        async fn full_reindex(
            &self,
            _vault_path: &std::path::Path,
            _exclude_folders: &[String],
        ) -> anyhow::Result<IndexStats> {
            Ok(IndexStats {
                total_documents: 1,
                total_chunks: 1,
                last_indexed: Some(chrono::Utc::now()),
            })
        }

        async fn reindex_files(
            &self,
            _vault_path: &std::path::Path,
            _rel_paths: &[String],
        ) -> anyhow::Result<()> {
            Ok(())
        }

        async fn get_index_stats(&self) -> anyhow::Result<IndexStats> {
            Ok(IndexStats {
                total_documents: 1,
                total_chunks: 1,
                last_indexed: Some(chrono::Utc::now()),
            })
        }
    }

    let state = Arc::new(AppState {
        search_engine,
        action_bus: action_tx,
        log_buffer,
        cli_args: CliArgs {
            port: 3579,
            language: Some("en".to_string()),
            model: None,
        },
        config: RwLock::new(KajetConfig::default()),
        vault_path: temp_dir.to_string_lossy().to_string(),
        db_path: temp_dir.clone(),
        note_count: AtomicUsize::new(1),
        chunk_count: AtomicUsize::new(1),
        indexing: AtomicBool::new(false),
        indexer: Arc::new(tokio::sync::RwLock::new(Arc::new(MockIndexer))),
    });

    let app = test_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/search?q=test&limit=10")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert!(json.is_array());
    let results = json.as_array().unwrap();
    assert!(!results.is_empty());
}

#[tokio::test]
async fn test_status_endpoint() {
    let state = test_app_state().await;
    let app = test_router(state.clone());

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert!(json["vault_path"].is_string());
    assert_eq!(json["note_count"], 2);
    assert_eq!(json["chunk_count"], 2);
    assert_eq!(json["language"], "en");
    assert_eq!(json["indexing"], false);
}

#[tokio::test]
async fn test_config_endpoint() {
    let state = test_app_state().await;
    let app = test_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/config")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    // Verify config has expected structure
    assert!(json["language"].is_string());
    assert!(json["port"].is_number());
    assert!(json["embedding"].is_object());

    // API key should be cleared for security
    assert_eq!(json["embedding"]["api_key"], "");
}

#[tokio::test]
async fn test_i18n_endpoint_returns_translations() {
    let state = test_app_state().await;
    let app = test_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/i18n")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    // Should be an object with translation keys
    assert!(json.is_object());
    let translations = json.as_object().unwrap();
    assert!(!translations.is_empty());

    // Check for some expected keys
    assert!(translations.contains_key("title"));
    assert!(translations.contains_key("subtitle"));
    assert!(translations.contains_key("nav_dashboard"));
}
