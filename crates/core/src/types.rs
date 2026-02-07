use crate::config::KajetConfig;
use crate::logging::broadcast_layer::LogBuffer;
use crate::logging::types::LogEntry;
use crate::search::SearchEngine;
use std::sync::atomic::{AtomicBool, AtomicUsize};
use std::sync::{Arc, RwLock};
use tokio::sync::broadcast;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct QueryEvent {
    pub query: String,
    pub num_results: usize,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Trait-based indexer interface so MCP can trigger reindexing
/// without depending on kajet-indexer crate directly.
#[async_trait::async_trait]
pub trait IndexerHandle: Send + Sync {
    async fn full_reindex(
        &self,
        vault_path: &std::path::Path,
        exclude_folders: &[String],
    ) -> anyhow::Result<IndexStats>;

    async fn reindex_files(
        &self,
        vault_path: &std::path::Path,
        rel_paths: &[String],
    ) -> anyhow::Result<()>;

    async fn get_index_stats(&self) -> anyhow::Result<IndexStats>;
}

pub struct AppState {
    pub search_engine: SearchEngine,
    pub events: broadcast::Sender<QueryEvent>,
    pub log_events: broadcast::Sender<LogEntry>,
    pub log_buffer: Arc<LogBuffer>,
    pub config: RwLock<KajetConfig>,
    pub vault_path: String,
    pub db_path: std::path::PathBuf,
    pub note_count: AtomicUsize,
    pub chunk_count: AtomicUsize,
    pub indexing: AtomicBool,
    pub indexer: Arc<dyn IndexerHandle>,
}

// ---------------------------------------------------------------------------
// Document — represents a full indexed file
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Document {
    pub source_file: String,
    pub full_text: String,
    pub title: String,
    pub tags: Vec<String>,
    pub content_hash: String,
    pub last_modified: f64, // Unix timestamp
}

// ---------------------------------------------------------------------------
// IndexStats — summary of the current index state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct IndexStats {
    pub total_documents: usize,
    pub total_chunks: usize,
    pub last_indexed: Option<chrono::DateTime<chrono::Utc>>,
}

// ---------------------------------------------------------------------------
// FtsHit — a single full-text search result
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct FtsHit {
    pub source_file: String,
    pub title: String,
    pub content_snippet: String,
    pub score: f32,
}

// ---------------------------------------------------------------------------
// FileChange — represents a detected change in the vault
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum FileChange {
    Added(std::path::PathBuf),
    Modified(std::path::PathBuf),
    Deleted(String), // relative path
}

// ---------------------------------------------------------------------------
// SearchType — how a result was found
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub enum SearchType {
    #[default]
    Vector,
    Fts,
    Hybrid,
}
