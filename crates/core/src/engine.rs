use crate::traits::{Embedder, StoredChunk, VectorStore};
use anyhow::Result;
use kajet_parser::Chunk;

#[derive(Debug, Clone, serde::Serialize)]
pub struct SearchResult {
    pub note_path: String,
    pub breadcrumb: String,
    pub content: String,
    pub raw_content: String,
    pub links: Vec<kajet_parser::Link>,
    pub score: f32,
    pub chunk_index: u32,
}

pub struct Engine {
    embedder: Box<dyn Embedder>,
    store: Box<dyn VectorStore>,
}

impl Engine {
    pub fn new(embedder: Box<dyn Embedder>, store: Box<dyn VectorStore>) -> Self {
        Self { embedder, store }
    }

    /// Embed and store chunks in the vector database. Returns count of indexed chunks.
    pub async fn index(&self, chunks: Vec<Chunk>) -> Result<usize> {
        if chunks.is_empty() {
            tracing::warn!("No chunks to index");
            return Ok(0);
        }

        let total = chunks.len();
        tracing::info!("Embedding {} chunks...", total);

        let texts: Vec<String> = chunks.iter().map(|c| c.embed_text()).collect();
        let text_refs: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();
        let embeddings = self.embedder.embed(text_refs).await?;

        let stored: Vec<StoredChunk> = chunks
            .into_iter()
            .zip(embeddings)
            .enumerate()
            .map(|(i, (chunk, vector))| StoredChunk {
                note_path: chunk.note_path,
                breadcrumb: chunk.breadcrumb,
                content: chunk.content,
                raw_content: chunk.raw_content,
                vector,
                chunk_index: i as u32,
                content_hash: String::new(),
                links: chunk.links,
            })
            .collect();

        self.store.store_chunks(&stored).await?;

        tracing::info!("Indexed {} chunks", total);
        Ok(total)
    }

    /// Search the vault for chunks similar to the query.
    pub async fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let query_emb = self.embedder.embed(vec![query]).await?;

        let hits = self.store.search(&query_emb[0], limit).await?;

        let results = hits
            .into_iter()
            .map(|hit| SearchResult {
                note_path: hit.note_path,
                breadcrumb: hit.breadcrumb,
                content: hit.content,
                raw_content: hit.raw_content,
                links: hit.links,
                score: hit.distance,
                chunk_index: hit.chunk_index,
            })
            .collect();

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::SearchHit;
    use crate::traits::mocks::{MockEmbedder, MockVectorStore};
    use std::sync::Arc;

    fn make_test_chunks() -> Vec<Chunk> {
        vec![
            Chunk {
                note_path: "note.md".into(),
                breadcrumb: "note.md > Intro".into(),
                content: "Hello world".into(),
                raw_content: "Hello world".into(),
                chunk_index: 0,
                links: vec![],
            },
            Chunk {
                note_path: "note.md".into(),
                breadcrumb: "note.md > Body".into(),
                content: "Some body text".into(),
                raw_content: "Some body text".into(),
                chunk_index: 1,
                links: vec![],
            },
        ]
    }

    #[tokio::test]
    async fn index_embeds_and_stores_all_chunks() {
        let embedder = Arc::new(MockEmbedder::new(4));
        let store = Arc::new(MockVectorStore::new());
        let store_clone = store.clone();

        let engine = Engine::new(Box::new(embedder), Box::new(store));
        let chunks = make_test_chunks();
        let count = engine.index(chunks).await.unwrap();

        assert_eq!(count, 2);

        let stored = store_clone.stored.lock().unwrap();
        assert_eq!(stored.len(), 2);
        assert_eq!(stored[0].note_path, "note.md");
        assert_eq!(stored[0].vector.len(), 4);
    }

    #[tokio::test]
    async fn index_empty_chunks_returns_zero() {
        let embedder = Arc::new(MockEmbedder::new(4));
        let store = Arc::new(MockVectorStore::new());
        let engine = Engine::new(Box::new(embedder), Box::new(store));

        let count = engine.index(vec![]).await.unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn search_returns_mapped_results() {
        let embedder = Arc::new(MockEmbedder::new(4));
        let store = Arc::new(MockVectorStore::with_search_results(vec![
            SearchHit {
                note_path: "note.md".into(),
                breadcrumb: "note.md > Intro".into(),
                content: "Hello".into(),
                raw_content: "Hello".into(),
                links: vec![],
                distance: 0.1,
                chunk_index: 0,
            },
            SearchHit {
                note_path: "other.md".into(),
                breadcrumb: "other.md".into(),
                content: "World".into(),
                raw_content: "World".into(),
                links: vec![],
                distance: 0.5,
                chunk_index: 0,
            },
        ]));

        let engine = Engine::new(Box::new(embedder), Box::new(store));
        let results = engine.search("hello", 5).await.unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].note_path, "note.md");
        assert_eq!(results[0].score, 0.1);
        assert_eq!(results[1].content, "World");
    }

    #[tokio::test]
    async fn search_respects_limit() {
        let embedder = Arc::new(MockEmbedder::new(4));
        let store = Arc::new(MockVectorStore::with_search_results(vec![
            SearchHit {
                note_path: "a.md".into(),
                breadcrumb: "a.md".into(),
                content: "A".into(),
                raw_content: "A".into(),
                links: vec![],
                distance: 0.1,
                chunk_index: 0,
            },
            SearchHit {
                note_path: "b.md".into(),
                breadcrumb: "b.md".into(),
                content: "B".into(),
                raw_content: "B".into(),
                links: vec![],
                distance: 0.2,
                chunk_index: 1,
            },
        ]));

        let engine = Engine::new(Box::new(embedder), Box::new(store));
        let results = engine.search("query", 1).await.unwrap();

        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn index_sends_correct_texts_to_embedder() {
        let embedder = Arc::new(MockEmbedder::new(4));
        let embedder_clone = embedder.clone();
        let store = Arc::new(MockVectorStore::new());

        let engine = Engine::new(Box::new(embedder), Box::new(store));
        let chunks = make_test_chunks();
        engine.index(chunks).await.unwrap();

        let calls = embedder_clone.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].len(), 2);
        assert!(calls[0][0].contains("Hello world"));
        assert!(calls[0][1].contains("Some body text"));
    }
}
