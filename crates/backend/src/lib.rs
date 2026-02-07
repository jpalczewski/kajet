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
use std::sync::Arc;

/// Production constructor — connects to LanceDB and loads Candle embedding model.
/// Returns a SearchEngine with both vector store and document store.
///
/// `db_path` is the resolved database directory (see `kajet_core::db_path`).
pub async fn create_production_search_engine(
    db_path: &str,
    model_id: &str,
) -> Result<SearchEngine> {
    let embedder = CandleEmbedder::new(model_id)?;
    let store = LanceVectorStore::new(db_path).await?;
    let doc_store = LanceDocumentStore::new(db_path).await?;
    Ok(SearchEngine::new(
        Arc::new(embedder),
        Arc::new(store),
        Arc::new(doc_store),
    ))
}
