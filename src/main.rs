#[macro_use]
extern crate rust_i18n;

i18n!("locales", fallback = "en");

pub mod embedder_factory;

use anyhow::Result;
use clap::Parser;
use kajet_core::types::AppState;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use tokio::sync::{broadcast, mpsc};
use unicode_normalization::UnicodeNormalization;

#[derive(Parser)]
#[command(name = "kajet", about = "MCP server for Obsidian vault with RAG")]
struct Cli {
    /// Path to Obsidian vault
    #[arg(short, long)]
    vault: String,

    /// Dashboard port
    #[arg(short, long, default_value = "3579")]
    port: u16,

    /// UI/results language (en, pl)
    #[arg(short, long)]
    language: Option<String>,

    /// Embedding model (HuggingFace model ID)
    #[arg(short, long)]
    model: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Log panics through tracing so they appear in kajet.log
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let payload = if let Some(s) = info.payload().downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "unknown panic".to_string()
        };
        let location = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_default();
        tracing::error!(payload = %payload, location = %location, "PANIC in tokio task");
        default_hook(info);
    }));

    let cli = Cli::parse();

    let vault_path = std::path::Path::new(&cli.vault);
    let db_path = kajet_core::db_path::resolve_db_path(vault_path);
    std::fs::create_dir_all(&db_path)?;

    let mut cfg = kajet_core::config::load_config(&db_path, cli.port, cli.language.clone())?;

    // Action bus — unified event channel for all events (logs, queries, indexing, config changes)
    let (action_tx, _) = broadcast::channel::<kajet_core::actions::ActionEvent>(512);

    // Dual-sink logging: file ({db_path}/kajet.log) + broadcast (dashboard via action_bus)
    let log_buffer = kajet_core::logging::init_logging(&cfg.logging, &db_path, action_tx.clone())?;

    // CLI --model overrides config
    if let Some(ref model) = cli.model {
        cfg.embedding.model = model.clone();
    }

    tracing::info!(
        "Config loaded: language={}, port={}, embedding={:?}/{}",
        cfg.language,
        cfg.port,
        cfg.embedding.backend,
        cfg.embedding.model
    );

    rust_i18n::set_locale(&cfg.language);

    // Check if embedding config or schema version changed — need full reindex if so
    let (embedding_changed, schema_changed) =
        match kajet_backend::metadata::VaultMetadata::load(&db_path)? {
            Some(meta) => {
                let (backend_str, base_url_for_compare) = match cfg.embedding.backend {
                    kajet_core::config::EmbeddingBackend::Candle => ("candle", ""),
                    kajet_core::config::EmbeddingBackend::Remote => {
                        ("remote", cfg.embedding.base_url.as_str())
                    }
                };
                (
                    meta.needs_reindex(backend_str, &cfg.embedding.model, base_url_for_compare),
                    meta.schema_version != Some(kajet_backend::metadata::CURRENT_SCHEMA_VERSION),
                )
            }
            None => (true, false), // First run: use full_reindex to build similarity graph
        };

    let needs_full_reindex = embedding_changed || schema_changed;

    if embedding_changed {
        tracing::info!("Embedding config changed, will perform full reindex");
    }
    if schema_changed {
        tracing::info!(
            "Schema version changed, will perform full reindex (NFC path normalization)"
        );
    }

    let embedder = embedder_factory::create_embedder(&cfg.embedding).await?;

    let db_path_str = db_path.to_string_lossy().to_string();
    let search_engine =
        kajet_backend::create_production_search_engine(&db_path_str, embedder.clone())
            .await?
            .with_query_prefix(cfg.embedding.query_prefix.clone());

    let indexer = Arc::new(
        kajet_indexer::Indexer::new(
            embedder,
            search_engine.store().clone(),
            search_engine.doc_store().clone(),
        )
        .with_concurrency(cfg.max_concurrent_files, cfg.pipeline_buffer_size)
        .with_progress_step(cfg.logging.progress_percent_step)
        .with_date_fields(
            cfg.writer.frontmatter.created_date_field.clone(),
            cfg.writer.frontmatter.modified_date_field.clone(),
        )
        .with_document_prefix(cfg.embedding.document_prefix.clone()),
    );

    let port = cfg.port;
    let open_browser = cfg.open_browser;
    let exclude_folders = cfg.exclude_folders.clone();
    let state = Arc::new(AppState {
        search_engine,
        action_bus: action_tx,
        log_buffer,
        cli_args: kajet_core::types::CliArgs {
            port: cli.port,
            language: cli.language.clone(),
            model: cli.model.clone(),
        },
        vault_path: cli.vault.clone(),
        db_path: db_path.clone(),
        note_count: AtomicUsize::new(0),
        chunk_count: AtomicUsize::new(0),
        indexing: AtomicBool::new(true),
        config: std::sync::RwLock::new(cfg),
        indexer: Arc::new(tokio::sync::RwLock::new(indexer.clone())),
        discover_context: tokio::sync::RwLock::new(None),
    });

    // Start dashboard BEFORE indexing — Logs tab shows progress live
    // Bind early so we fail fast if port is already in use
    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port))
        .await
        .map_err(|e| {
            anyhow::anyhow!(
                "Cannot bind to port {}: {} — is another kajet instance running?",
                port,
                e
            )
        })?;

    let web_state = state.clone();
    tokio::spawn(async move {
        if let Err(e) = kajet_web::serve_with_listener(web_state, listener).await {
            tracing::error!("Dashboard error: {}", e);
        }
    });

    tracing::info!("kajet dashboard: http://localhost:{}", port);

    if open_browser {
        let _ = open::that(format!("http://localhost:{}", port));
    }

    // Spawn indexing + file watcher as background task
    let idx_state = state.clone();
    let idx_vault = cli.vault.clone();
    let idx_indexer = indexer.clone();
    let idx_exclude = exclude_folders.clone();
    tokio::spawn(async move {
        tracing::info!("Indexing vault: {}", idx_vault);
        let vault = std::path::Path::new(&idx_vault);
        let stats = if needs_full_reindex {
            idx_indexer.full_reindex(vault, &idx_exclude).await
        } else {
            idx_indexer.incremental_index(vault, &idx_exclude).await
        };

        match stats {
            Ok(stats) => {
                let (backend, model, base_url) = {
                    let cfg = idx_state.config.read().unwrap();
                    let (backend, base_url) = match cfg.embedding.backend {
                        kajet_core::config::EmbeddingBackend::Candle => ("candle", String::new()),
                        kajet_core::config::EmbeddingBackend::Remote => (
                            "remote",
                            cfg.embedding.base_url.trim_end_matches('/').to_string(),
                        ),
                    };
                    (backend, cfg.embedding.model.clone(), base_url)
                };
                if let Err(e) = kajet_backend::metadata::VaultMetadata::save_embedding(
                    &idx_state.db_path,
                    backend,
                    &model,
                    &base_url,
                ) {
                    tracing::error!("Failed to save vault metadata: {e}");
                }

                idx_state.indexing.store(false, Ordering::Relaxed);
                idx_state
                    .note_count
                    .store(stats.total_documents, Ordering::Relaxed);
                idx_state
                    .chunk_count
                    .store(stats.total_chunks, Ordering::Relaxed);
                tracing::info!(
                    "Indexing complete: {} documents, {} chunks",
                    stats.total_documents,
                    stats.total_chunks
                );

                // Load discover context (similarity graph + link graph for bridges/clusters)
                idx_state.refresh_discover_context().await;

                // Start file watcher after indexing completes
                let (watch_tx, mut watch_rx) = mpsc::unbounded_channel();
                match kajet_indexer::watcher::VaultWatcher::start(vault, watch_tx) {
                    Ok(_watcher) => {
                        let watcher_indexer = idx_state.indexer.clone();
                        let watcher_state = idx_state.clone();
                        let watcher_vault =
                            vault.canonicalize().unwrap_or_else(|_| vault.to_path_buf());
                        tokio::spawn(async move {
                            let _watcher = _watcher;
                            while let Some(changed_paths) = watch_rx.recv().await {
                                let rel_paths: Vec<String> = changed_paths
                                    .iter()
                                    .filter_map(|p| p.strip_prefix(&watcher_vault).ok())
                                    .map(|p| p.to_string_lossy().nfc().collect::<String>())
                                    .collect();
                                if !rel_paths.is_empty() {
                                    tracing::info!(
                                        "File watcher: reindexing {} files",
                                        rel_paths.len()
                                    );
                                    let indexer = watcher_indexer.read().await.clone();
                                    if let Err(e) =
                                        indexer.reindex_files(&watcher_vault, &rel_paths).await
                                    {
                                        tracing::error!("Watcher reindex error: {e}");
                                    }
                                    if let Ok(new_stats) = indexer.get_index_stats().await {
                                        watcher_state
                                            .note_count
                                            .store(new_stats.total_documents, Ordering::Relaxed);
                                        watcher_state
                                            .chunk_count
                                            .store(new_stats.total_chunks, Ordering::Relaxed);
                                    }
                                    // Refresh discover context (graph invalidated by incremental reindex)
                                    watcher_state.refresh_discover_context().await;
                                }
                            }
                        });
                    }
                    Err(e) => {
                        tracing::error!("Failed to start file watcher: {e}");
                    }
                }
            }
            Err(e) => {
                tracing::error!("Indexing failed: {e}");
                idx_state.indexing.store(false, Ordering::Relaxed);
            }
        }
    });

    // MCP server on main task (stdio) — starts immediately, no wait for indexing
    kajet_mcp::serve(state).await?;

    Ok(())
}
