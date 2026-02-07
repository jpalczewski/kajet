#[macro_use]
extern crate rust_i18n;

i18n!("locales", fallback = "en");

use anyhow::Result;
use clap::Parser;
use kajet_core::logging::types::LogEntry;
use kajet_core::types::{AppState, QueryEvent};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};

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
    let cli = Cli::parse();

    let mut cfg = kajet_core::config::load_config(&cli.vault, cli.port, cli.language)?;

    // Dual-sink logging: file (.kajet/kajet.log) + broadcast (dashboard)
    let vault_path = std::path::Path::new(&cli.vault);
    let (log_tx, _) = broadcast::channel::<LogEntry>(512);
    let log_buffer = kajet_core::logging::init_logging(&cfg.logging, vault_path, log_tx.clone())?;

    // CLI --model overrides config
    if let Some(ref model) = cli.model {
        cfg.embedding_model = model.clone();
    }

    tracing::info!(
        "Config loaded: language={}, port={}, model={}",
        cfg.language,
        cfg.port,
        cfg.embedding_model
    );

    rust_i18n::set_locale(&cfg.language);

    let (tx, _) = broadcast::channel::<QueryEvent>(100);

    // Check if embedding model changed — need full reindex if so
    let model_changed = match kajet_backend::metadata::VaultMetadata::load(vault_path)? {
        Some(meta) => meta.embedding_model != cfg.embedding_model,
        None => false, // First run, incremental is fine
    };

    if model_changed {
        tracing::info!(
            "Embedding model changed to '{}', will perform full reindex",
            cfg.embedding_model
        );
    }

    let search_engine =
        kajet_backend::create_production_search_engine(&cli.vault, &cfg.embedding_model).await?;

    let indexer = Arc::new(
        kajet_indexer::Indexer::new(
            search_engine.embedder().clone(),
            search_engine.store().clone(),
            search_engine.doc_store().clone(),
        )
        .with_concurrency(cfg.max_concurrent_files, cfg.pipeline_buffer_size)
        .with_progress_step(cfg.logging.progress_percent_step),
    );

    let port = cfg.port;
    let open_browser = cfg.open_browser;
    let exclude_folders = cfg.exclude_folders.clone();
    let state = Arc::new(AppState {
        search_engine,
        events: tx,
        log_events: log_tx,
        log_buffer,
        vault_path: cli.vault.clone(),
        note_count: AtomicUsize::new(0),
        chunk_count: AtomicUsize::new(0),
        indexing: AtomicBool::new(true),
        config: std::sync::RwLock::new(cfg),
        indexer: indexer.clone(),
    });

    // Start dashboard BEFORE indexing — Logs tab shows progress live
    let web_state = state.clone();
    tokio::spawn(async move {
        if let Err(e) = kajet_web::serve(web_state, port).await {
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
        let stats = if model_changed {
            idx_indexer.full_reindex(vault, &idx_exclude).await
        } else {
            idx_indexer.incremental_index(vault, &idx_exclude).await
        };

        match stats {
            Ok(stats) => {
                if let Err(e) = kajet_backend::metadata::VaultMetadata::save(
                    vault,
                    &idx_state.config.read().unwrap().embedding_model,
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

                // Start file watcher after indexing completes
                let (watch_tx, mut watch_rx) = mpsc::unbounded_channel();
                match kajet_indexer::watcher::VaultWatcher::start(vault, watch_tx) {
                    Ok(_watcher) => {
                        let watcher_indexer = idx_state.indexer.clone();
                        let watcher_state = idx_state.clone();
                        let watcher_vault = vault.to_path_buf();
                        tokio::spawn(async move {
                            let _watcher = _watcher;
                            while let Some(changed_paths) = watch_rx.recv().await {
                                let rel_paths: Vec<String> = changed_paths
                                    .iter()
                                    .filter_map(|p| p.strip_prefix(&watcher_vault).ok())
                                    .map(|p| p.to_string_lossy().to_string())
                                    .collect();
                                if !rel_paths.is_empty() {
                                    tracing::info!(
                                        "File watcher: reindexing {} files",
                                        rel_paths.len()
                                    );
                                    if let Err(e) = watcher_indexer
                                        .reindex_files(&watcher_vault, &rel_paths)
                                        .await
                                    {
                                        tracing::error!("Watcher reindex error: {e}");
                                    }
                                    if let Ok(new_stats) = watcher_indexer.get_index_stats().await {
                                        watcher_state
                                            .note_count
                                            .store(new_stats.total_documents, Ordering::Relaxed);
                                        watcher_state
                                            .chunk_count
                                            .store(new_stats.total_chunks, Ordering::Relaxed);
                                    }
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
