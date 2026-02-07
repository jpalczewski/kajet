mod embedder;
mod store;

pub use embedder::CandleEmbedder;
pub use store::LanceVectorStore;

use anyhow::Result;
use kajet_core::engine::Engine;

/// Production constructor — connects to LanceDB and loads Candle embedding model.
pub async fn create_production_engine(vault_path: &str) -> Result<Engine> {
    let embedder = CandleEmbedder::new()?;
    let store = LanceVectorStore::new(vault_path).await?;
    Ok(Engine::new(Box::new(embedder), Box::new(store)))
}
