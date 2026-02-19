mod document_store;
mod embedder;
pub mod hasher;
pub mod metadata;
mod store;

pub use document_store::LanceDocumentStore;
pub use embedder::CandleEmbedder;
pub use store::LanceVectorStore;

use anyhow::Result;
use kajet_core::search::SearchEngine;
use kajet_core::traits::Embedder;
use std::sync::Arc;

/// Production constructor — connects to LanceDB and wraps a provided embedder.
///
/// `db_path` is the resolved database directory (see `kajet_core::db_path`).
pub async fn create_production_search_engine(
    db_path: &str,
    embedder: Arc<dyn Embedder>,
) -> Result<SearchEngine> {
    let store = LanceVectorStore::new(db_path).await?;
    let doc_store = LanceDocumentStore::new(db_path).await?;
    Ok(SearchEngine::new(
        embedder,
        Arc::new(store),
        Arc::new(doc_store),
    ))
}
