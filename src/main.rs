#[macro_use]
extern crate rust_i18n;

i18n!("locales", fallback = "en");

use anyhow::Result;
use clap::Parser;
use kajet_core::logging::types::LogEntry;
use kajet_core::types::{AppState, QueryEvent};
use std::sync::atomic::{AtomicUsize, Ordering};
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

    // Index vault (dashboard already live, logs visible in real-time)
    tracing::info!("Indexing vault: {}", cli.vault);
    let stats = if model_changed {
        indexer.full_reindex(vault_path, &exclude_folders).await?
    } else {
        indexer
            .incremental_index(vault_path, &exclude_folders)
            .await?
    };

    // Save metadata after successful indexing
    kajet_backend::metadata::VaultMetadata::save(
        vault_path,
        &state.config.read().unwrap().embedding_model,
    )?;

    state
        .note_count
        .store(stats.total_documents, Ordering::Relaxed);
    state
        .chunk_count
        .store(stats.total_chunks, Ordering::Relaxed);

    tracing::info!(
        "Indexing complete: {} documents, {} chunks",
        stats.total_documents,
        stats.total_chunks
    );

    // Start file watcher for live reindexing
    let (watch_tx, mut watch_rx) = mpsc::unbounded_channel();
    let _watcher = kajet_indexer::watcher::VaultWatcher::start(vault_path, watch_tx)?;

    let watcher_indexer = state.indexer.clone();
    let watcher_vault = cli.vault.clone();
    let watcher_state = state.clone();
    tokio::spawn(async move {
        let vault = std::path::Path::new(&watcher_vault);
        while let Some(changed_paths) = watch_rx.recv().await {
            let rel_paths: Vec<String> = changed_paths
                .iter()
                .filter_map(|p| p.strip_prefix(vault).ok())
                .map(|p| p.to_string_lossy().to_string())
                .collect();
            if !rel_paths.is_empty() {
                tracing::info!("File watcher: reindexing {} files", rel_paths.len());
                if let Err(e) = watcher_indexer.reindex_files(vault, &rel_paths).await {
                    tracing::error!("Watcher reindex error: {e}");
                }
                // Update counts after watcher reindex
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

    // MCP server on main task (stdio)
    kajet_mcp::serve(state).await?;

    Ok(())
}
