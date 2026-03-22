use crate::config::KajetConfig;
use crate::link_graph::InMemoryLinkGraph;
use crate::logging::broadcast_layer::LogBuffer;
use crate::search::SearchEngine;
use crate::similarity_graph::{CsrGraph, SimilarityGraph};
use std::sync::atomic::{AtomicBool, AtomicUsize};
use std::sync::{Arc, RwLock};
use tokio::sync::broadcast;

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

/// Original CLI arguments — preserved for config reload.
/// Web handlers must use these (not current config values) when reloading
/// to prevent CLI override priority from masking vault-level overrides.
#[derive(Debug, Clone)]
pub struct CliArgs {
    pub port: u16,
    pub language: Option<String>,
    pub model: Option<String>,
}

pub struct AppState {
    pub search_engine: SearchEngine,
    pub action_bus: broadcast::Sender<crate::actions::ActionEvent>,
    pub log_buffer: Arc<LogBuffer>,
    pub cli_args: CliArgs,
    pub config: RwLock<KajetConfig>,
    pub vault_path: String,
    pub db_path: std::path::PathBuf,
    pub note_count: AtomicUsize,
    pub chunk_count: AtomicUsize,
    pub indexing: AtomicBool,
    pub indexer: Arc<tokio::sync::RwLock<Arc<dyn IndexerHandle>>>,
    pub discover_context: tokio::sync::RwLock<Option<Arc<DiscoverContext>>>,
}

/// Bundle of precomputed structures for discover tools (bridges, clusters, etc.).
/// Loaded from disk after reindex; None if graph has not been built yet.
pub struct DiscoverContext {
    pub similarity_graph: CsrGraph,
    pub link_graph: InMemoryLinkGraph,
}

impl AppState {
    /// Load or clear the discover context based on the current graph file and documents.
    /// Call after every reindex (full or incremental).
    pub async fn refresh_discover_context(&self) {
        let graph_path = self.db_path.join("similarity_graph.kjsg");
        if !graph_path.exists() {
            *self.discover_context.write().await = None;
            tracing::debug!("Discover context cleared (no similarity graph file)");
            return;
        }

        let graph = match CsrGraph::load_from_path(&graph_path) {
            Ok(g) => g,
            Err(e) => {
                tracing::warn!("Failed to load similarity graph: {e}");
                *self.discover_context.write().await = None;
                return;
            }
        };

        let docs = match self.search_engine.doc_store().get_all_documents().await {
            Ok(d) => d,
            Err(e) => {
                tracing::warn!("Failed to load documents for link graph: {e}");
                *self.discover_context.write().await = None;
                return;
            }
        };

        let link_graph = InMemoryLinkGraph::from_documents(&docs);

        tracing::info!(
            graph_chunks = graph.len(),
            graph_has_identity = graph.has_identity(),
            link_docs = link_graph.doc_count(),
            "Discover context loaded"
        );

        *self.discover_context.write().await = Some(Arc::new(DiscoverContext {
            similarity_graph: graph,
            link_graph,
        }));
    }
}

// ---------------------------------------------------------------------------
// Document — represents a full indexed file
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, ts_rs::TS)]
#[ts(export, export_to = "../../../frontend/src/lib/types/generated/")]
pub struct Document {
    pub source_file: String,
    pub full_text: String,
    pub title: String,
    pub tags: Vec<String>,
    pub content_hash: String,
    pub last_modified: f64, // Unix timestamp
    pub outgoing_links: Vec<String>,
    pub backlinks: Vec<String>,
}

// ---------------------------------------------------------------------------
// IndexStats — summary of the current index state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, ts_rs::TS)]
#[ts(export, export_to = "../../../frontend/src/lib/types/generated/")]
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
