#[macro_use]
extern crate rust_i18n;

i18n!("locales", fallback = "en");

use anyhow::Result;
use clap::Parser;
use kajet_core::types::{AppState, QueryEvent};
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing_subscriber::EnvFilter;

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
}

#[tokio::main]
async fn main() -> Result<()> {
    // Logs to stderr only — stdout is MCP
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("kajet=debug".parse()?))
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();

    let cfg = kajet_core::config::load_config(&cli.vault, cli.port, cli.language)?;
    tracing::info!("Config loaded: language={}, port={}", cfg.language, cfg.port);

    rust_i18n::set_locale(&cfg.language);

    let (tx, _) = broadcast::channel::<QueryEvent>(100);

    tracing::info!("Indexing vault: {}", cli.vault);
    let engine = kajet_backend::create_production_engine(&cli.vault).await?;
    let chunks = kajet_core::parser::parse_vault(&cli.vault, &cfg.exclude_folders)?;
    engine.index(chunks).await?;
    tracing::info!("Indexing complete");

    let port = cfg.port;
    let state = Arc::new(AppState {
        engine,
        events: tx,
        config: cfg,
    });

    // Axum dashboard in background
    let web_state = state.clone();
    tokio::spawn(async move {
        if let Err(e) = kajet_web::serve(web_state, port).await {
            tracing::error!("Dashboard error: {}", e);
        }
    });

    eprintln!("📓 kajet dashboard: http://localhost:{}", port);

    // MCP server on main task (stdio)
    kajet_mcp::serve(state).await?;

    Ok(())
}
