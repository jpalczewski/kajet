use anyhow::Result;
use async_trait::async_trait;

/// A chunk ready to be stored in the vector database.
#[derive(Debug, Clone)]
pub struct StoredChunk {
    pub note_path: String,
    pub breadcrumb: String,
    pub content: String,
    pub vector: Vec<f32>,
}

/// A hit returned from vector similarity search.
#[derive(Debug, Clone)]
pub struct SearchHit {
    pub note_path: String,
    pub breadcrumb: String,
    pub content: String,
    pub distance: f32,
}

/// Abstraction over an embedding model.
pub trait Embedder: Send + Sync {
    /// Embed a batch of texts, returns one vector per input.
    fn embed(&self, texts: Vec<&str>) -> Result<Vec<Vec<f32>>>;
    /// Dimensionality of the embedding vectors.
    fn dimension(&self) -> usize;
}

impl<T: Embedder> Embedder for std::sync::Arc<T> {
    fn embed(&self, texts: Vec<&str>) -> Result<Vec<Vec<f32>>> {
        (**self).embed(texts)
    }
    fn dimension(&self) -> usize {
        (**self).dimension()
    }
}

/// Abstraction over a vector store.
#[async_trait]
pub trait VectorStore: Send + Sync {
    async fn store_chunks(&self, chunks: &[StoredChunk]) -> Result<()>;
    async fn search(&self, vector: &[f32], limit: usize) -> Result<Vec<SearchHit>>;
}

#[async_trait]
impl<T: VectorStore> VectorStore for std::sync::Arc<T> {
    async fn store_chunks(&self, chunks: &[StoredChunk]) -> Result<()> {
        (**self).store_chunks(chunks).await
    }
    async fn search(&self, vector: &[f32], limit: usize) -> Result<Vec<SearchHit>> {
        (**self).search(vector, limit).await
    }
}

#[cfg(test)]
pub mod mocks {
    use super::*;
    use std::sync::Mutex;

    /// Mock embedder that returns fixed-dimension zero vectors.
    pub struct MockEmbedder {
        pub dim: usize,
        pub calls: Mutex<Vec<Vec<String>>>,
    }

    impl MockEmbedder {
        pub fn new(dim: usize) -> Self {
            Self {
                dim,
                calls: Mutex::new(Vec::new()),
            }
        }
    }

    impl Embedder for MockEmbedder {
        fn embed(&self, texts: Vec<&str>) -> Result<Vec<Vec<f32>>> {
            self.calls
                .lock()
                .unwrap()
                .push(texts.iter().map(|s| s.to_string()).collect());
            Ok(texts.iter().map(|_| vec![0.0; self.dim]).collect())
        }

        fn dimension(&self) -> usize {
            self.dim
        }
    }

    /// Mock vector store that records store calls and returns preset search results.
    pub struct MockVectorStore {
        pub stored: Mutex<Vec<StoredChunk>>,
        pub search_results: Mutex<Vec<SearchHit>>,
    }

    impl MockVectorStore {
        pub fn new() -> Self {
            Self {
                stored: Mutex::new(Vec::new()),
                search_results: Mutex::new(Vec::new()),
            }
        }

        pub fn with_search_results(results: Vec<SearchHit>) -> Self {
            Self {
                stored: Mutex::new(Vec::new()),
                search_results: Mutex::new(results),
            }
        }
    }

    #[async_trait]
    impl VectorStore for MockVectorStore {
        async fn store_chunks(&self, chunks: &[StoredChunk]) -> Result<()> {
            self.stored.lock().unwrap().extend(chunks.iter().cloned());
            Ok(())
        }

        async fn search(&self, _vector: &[f32], limit: usize) -> Result<Vec<SearchHit>> {
            let results = self.search_results.lock().unwrap();
            Ok(results.iter().take(limit).cloned().collect())
        }
    }
}
