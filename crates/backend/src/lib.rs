mod document_store;
mod embedder;
pub mod hasher;
mod store;

pub use document_store::LanceDocumentStore;
pub use embedder::CandleEmbedder;
pub use store::LanceVectorStore;

use anyhow::Result;
use kajet_core::search::SearchEngine;
use std::sync::Arc;

/// Production constructor — connects to LanceDB and loads Candle embedding model.
/// Returns a SearchEngine with both vector store and document store.
pub async fn create_production_search_engine(vault_path: &str) -> Result<SearchEngine> {
    let embedder = CandleEmbedder::new()?;
    let store = LanceVectorStore::new(vault_path).await?;
    let doc_store = LanceDocumentStore::new(vault_path).await?;
    Ok(SearchEngine::new(
        Arc::new(embedder),
        Arc::new(store),
        Arc::new(doc_store),
    ))
}
