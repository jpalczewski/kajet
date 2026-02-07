use crate::types::{Document, FtsHit, IndexStats};
use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;

/// A chunk ready to be stored in the vector database.
#[derive(Debug, Clone)]
pub struct StoredChunk {
    pub note_path: String,
    pub breadcrumb: String,
    pub content: String,
    pub vector: Vec<f32>,
    pub chunk_index: u32,
    pub content_hash: String,
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

/// Abstraction over a vector store (chunks table).
#[async_trait]
pub trait VectorStore: Send + Sync {
    async fn store_chunks(&self, chunks: &[StoredChunk]) -> Result<()>;
    async fn search(&self, vector: &[f32], limit: usize) -> Result<Vec<SearchHit>>;
    async fn upsert_chunks(&self, chunks: &[StoredChunk]) -> Result<()>;
    async fn delete_chunks_by_paths(&self, paths: &[String]) -> Result<()>;
}

#[async_trait]
impl<T: VectorStore> VectorStore for std::sync::Arc<T> {
    async fn store_chunks(&self, chunks: &[StoredChunk]) -> Result<()> {
        (**self).store_chunks(chunks).await
    }
    async fn search(&self, vector: &[f32], limit: usize) -> Result<Vec<SearchHit>> {
        (**self).search(vector, limit).await
    }
    async fn upsert_chunks(&self, chunks: &[StoredChunk]) -> Result<()> {
        (**self).upsert_chunks(chunks).await
    }
    async fn delete_chunks_by_paths(&self, paths: &[String]) -> Result<()> {
        (**self).delete_chunks_by_paths(paths).await
    }
}

/// Abstraction over a document store (documents table, FTS).
#[async_trait]
pub trait DocumentStore: Send + Sync {
    async fn store_documents(&self, docs: &[Document]) -> Result<()>;
    async fn get_document_hashes(&self) -> Result<HashMap<String, String>>;
    async fn delete_by_paths(&self, paths: &[String]) -> Result<()>;
    async fn fts_search(&self, query: &str, limit: usize) -> Result<Vec<FtsHit>>;
    async fn create_fts_index(&self) -> Result<()>;
    async fn get_index_stats(&self) -> Result<IndexStats>;
}

#[async_trait]
impl<T: DocumentStore> DocumentStore for std::sync::Arc<T> {
    async fn store_documents(&self, docs: &[Document]) -> Result<()> {
        (**self).store_documents(docs).await
    }
    async fn get_document_hashes(&self) -> Result<HashMap<String, String>> {
        (**self).get_document_hashes().await
    }
    async fn delete_by_paths(&self, paths: &[String]) -> Result<()> {
        (**self).delete_by_paths(paths).await
    }
    async fn fts_search(&self, query: &str, limit: usize) -> Result<Vec<FtsHit>> {
        (**self).fts_search(query, limit).await
    }
    async fn create_fts_index(&self) -> Result<()> {
        (**self).create_fts_index().await
    }
    async fn get_index_stats(&self) -> Result<IndexStats> {
        (**self).get_index_stats().await
    }
}

#[cfg(any(test, feature = "test-utils"))]
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

    impl Default for MockVectorStore {
        fn default() -> Self {
            Self {
                stored: Mutex::new(Vec::new()),
                search_results: Mutex::new(Vec::new()),
            }
        }
    }

    impl MockVectorStore {
        pub fn new() -> Self {
            Self::default()
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

        async fn upsert_chunks(&self, chunks: &[StoredChunk]) -> Result<()> {
            self.stored.lock().unwrap().extend(chunks.iter().cloned());
            Ok(())
        }

        async fn delete_chunks_by_paths(&self, paths: &[String]) -> Result<()> {
            self.stored
                .lock()
                .unwrap()
                .retain(|c| !paths.contains(&c.note_path));
            Ok(())
        }
    }

    /// Mock document store for testing.
    pub struct MockDocumentStore {
        pub documents: Mutex<Vec<Document>>,
        pub fts_results: Mutex<Vec<FtsHit>>,
    }

    impl Default for MockDocumentStore {
        fn default() -> Self {
            Self {
                documents: Mutex::new(Vec::new()),
                fts_results: Mutex::new(Vec::new()),
            }
        }
    }

    impl MockDocumentStore {
        pub fn new() -> Self {
            Self::default()
        }

        pub fn with_fts_results(results: Vec<FtsHit>) -> Self {
            Self {
                documents: Mutex::new(Vec::new()),
                fts_results: Mutex::new(results),
            }
        }
    }

    #[async_trait]
    impl DocumentStore for MockDocumentStore {
        async fn store_documents(&self, docs: &[Document]) -> Result<()> {
            let mut stored = self.documents.lock().unwrap();
            for doc in docs {
                if let Some(existing) = stored.iter_mut().find(|d| d.source_file == doc.source_file)
                {
                    *existing = doc.clone();
                } else {
                    stored.push(doc.clone());
                }
            }
            Ok(())
        }

        async fn get_document_hashes(&self) -> Result<HashMap<String, String>> {
            let docs = self.documents.lock().unwrap();
            Ok(docs
                .iter()
                .map(|d| (d.source_file.clone(), d.content_hash.clone()))
                .collect())
        }

        async fn delete_by_paths(&self, paths: &[String]) -> Result<()> {
            self.documents
                .lock()
                .unwrap()
                .retain(|d| !paths.contains(&d.source_file));
            Ok(())
        }

        async fn fts_search(&self, _query: &str, limit: usize) -> Result<Vec<FtsHit>> {
            let results = self.fts_results.lock().unwrap();
            Ok(results.iter().take(limit).cloned().collect())
        }

        async fn create_fts_index(&self) -> Result<()> {
            Ok(())
        }

        async fn get_index_stats(&self) -> Result<IndexStats> {
            let docs = self.documents.lock().unwrap();
            Ok(IndexStats {
                total_documents: docs.len(),
                total_chunks: 0,
                last_indexed: Some(chrono::Utc::now()),
            })
        }
    }
}
