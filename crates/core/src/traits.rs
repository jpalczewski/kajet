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
    pub raw_content: String,
    pub vector: Vec<f32>,
    pub chunk_index: u32,
    pub content_hash: String,
    pub links: Vec<kajet_parser::Link>,
}

/// Stored metadata for a file, used for change detection.
#[derive(Debug, Clone)]
pub struct StoredFileInfo {
    pub content_hash: String,
    pub last_modified: f64,
}

/// A hit returned from vector similarity search.
#[derive(Debug, Clone)]
pub struct SearchHit {
    pub note_path: String,
    pub breadcrumb: String,
    pub content: String,
    pub raw_content: String,
    pub links: Vec<kajet_parser::Link>,
    pub distance: f32,
    pub chunk_index: u32,
}

/// Abstraction over an embedding model.
#[async_trait]
pub trait Embedder: Send + Sync {
    /// Embed a batch of texts, returns one vector per input.
    async fn embed(&self, texts: Vec<&str>) -> Result<Vec<Vec<f32>>>;
    /// Dimensionality of the embedding vectors.
    fn dimension(&self) -> usize;
    /// Optional upper bound for a single embed call batch.
    ///
    /// Workers can use this to avoid pre-splitting with hardcoded limits and let
    /// backend-specific configuration (e.g. remote provider batch size) drive
    /// chunking behavior.
    fn max_batch_size_hint(&self) -> Option<usize> {
        None
    }
}

#[async_trait]
impl<T: Embedder> Embedder for std::sync::Arc<T> {
    async fn embed(&self, texts: Vec<&str>) -> Result<Vec<Vec<f32>>> {
        (**self).embed(texts).await
    }
    fn dimension(&self) -> usize {
        (**self).dimension()
    }
    fn max_batch_size_hint(&self) -> Option<usize> {
        (**self).max_batch_size_hint()
    }
}

/// Abstraction over a vector store (chunks table).
#[async_trait]
pub trait VectorStore: Send + Sync {
    async fn store_chunks(&self, chunks: &[StoredChunk]) -> Result<()>;
    async fn search(&self, vector: &[f32], limit: usize) -> Result<Vec<SearchHit>>;
    async fn upsert_chunks(&self, chunks: &[StoredChunk]) -> Result<()>;
    async fn delete_chunks_by_paths(&self, paths: &[String]) -> Result<()>;
    async fn get_chunks_by_path(&self, note_path: &str) -> Result<Vec<StoredChunk>>;
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
    async fn get_chunks_by_path(&self, note_path: &str) -> Result<Vec<StoredChunk>> {
        (**self).get_chunks_by_path(note_path).await
    }
}

/// Abstraction over a document store (documents table, FTS).
#[async_trait]
pub trait DocumentStore: Send + Sync {
    async fn store_documents(&self, docs: &[Document]) -> Result<()>;
    async fn get_document_hashes(&self) -> Result<HashMap<String, StoredFileInfo>>;
    async fn delete_by_paths(&self, paths: &[String]) -> Result<()>;
    async fn fts_search(&self, query: &str, limit: usize) -> Result<Vec<FtsHit>>;
    async fn create_fts_index(&self) -> Result<()>;
    async fn get_index_stats(&self) -> Result<IndexStats>;
    async fn get_all_documents(&self) -> Result<Vec<Document>>;
    async fn get_document_by_path(&self, path: &str) -> Result<Option<Document>>;
    async fn update_backlinks(&self, backlinks: &HashMap<String, Vec<String>>) -> Result<()>;

    /// Query documents by temporal and folder filters.
    ///
    /// This method is designed for browse mode queries where documents are filtered by
    /// timestamp range and/or folder path, then sorted chronologically (oldest first).
    ///
    /// # Parameters
    ///
    /// - `from`: Optional minimum timestamp (Unix epoch, seconds). Documents with
    ///   `last_modified >= from` are included.
    /// - `to`: Optional maximum timestamp (Unix epoch, seconds). Documents with
    ///   `last_modified <= to` are included.
    /// - `folder`: Optional folder path prefix (e.g., `"journal/2025"`). Matches documents
    ///   whose `source_file` starts with `"{folder}/"`. Trailing slash is normalized internally.
    /// - `limit`: Maximum number of documents to return after filtering and sorting.
    ///
    /// # Returns
    ///
    /// A vector of documents matching the filters, sorted chronologically (oldest first),
    /// truncated to `limit`. Returns empty vector if no matches or table doesn't exist.
    ///
    /// # Implementation Notes
    ///
    /// - Implementations should push filtering to the database layer where possible
    /// - Tag filtering is NOT performed here (done in caller via post-filtering)
    /// - Over-fetching is recommended to account for tag post-filtering
    async fn query_documents(
        &self,
        from: Option<f64>,
        to: Option<f64>,
        folder: Option<&str>,
        limit: usize,
    ) -> Result<Vec<Document>>;
}

#[async_trait]
impl<T: DocumentStore> DocumentStore for std::sync::Arc<T> {
    async fn store_documents(&self, docs: &[Document]) -> Result<()> {
        (**self).store_documents(docs).await
    }
    async fn get_document_hashes(&self) -> Result<HashMap<String, StoredFileInfo>> {
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
    async fn get_all_documents(&self) -> Result<Vec<Document>> {
        (**self).get_all_documents().await
    }
    async fn get_document_by_path(&self, path: &str) -> Result<Option<Document>> {
        (**self).get_document_by_path(path).await
    }
    async fn update_backlinks(&self, backlinks: &HashMap<String, Vec<String>>) -> Result<()> {
        (**self).update_backlinks(backlinks).await
    }
    async fn query_documents(
        &self,
        from: Option<f64>,
        to: Option<f64>,
        folder: Option<&str>,
        limit: usize,
    ) -> Result<Vec<Document>> {
        (**self).query_documents(from, to, folder, limit).await
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

    #[async_trait]
    impl Embedder for MockEmbedder {
        async fn embed(&self, texts: Vec<&str>) -> Result<Vec<Vec<f32>>> {
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

        async fn get_chunks_by_path(&self, note_path: &str) -> Result<Vec<StoredChunk>> {
            let stored = self.stored.lock().unwrap();
            let mut chunks: Vec<StoredChunk> = stored
                .iter()
                .filter(|c| c.note_path == note_path)
                .cloned()
                .collect();
            // Sort by chunk_index (ascending) for correct document order
            chunks.sort_by_key(|c| c.chunk_index);
            Ok(chunks)
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

        async fn get_document_hashes(&self) -> Result<HashMap<String, StoredFileInfo>> {
            let docs = self.documents.lock().unwrap();
            Ok(docs
                .iter()
                .map(|d| {
                    (
                        d.source_file.clone(),
                        StoredFileInfo {
                            content_hash: d.content_hash.clone(),
                            last_modified: d.last_modified,
                        },
                    )
                })
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

        async fn get_all_documents(&self) -> Result<Vec<Document>> {
            Ok(self.documents.lock().unwrap().clone())
        }

        async fn get_document_by_path(&self, path: &str) -> Result<Option<Document>> {
            let docs = self.documents.lock().unwrap();
            Ok(docs.iter().find(|d| d.source_file == path).cloned())
        }

        async fn update_backlinks(&self, backlinks: &HashMap<String, Vec<String>>) -> Result<()> {
            let mut docs = self.documents.lock().unwrap();
            for doc in docs.iter_mut() {
                if let Some(bl) = backlinks.get(&doc.source_file) {
                    doc.backlinks = bl.clone();
                }
            }
            Ok(())
        }

        async fn query_documents(
            &self,
            from: Option<f64>,
            to: Option<f64>,
            folder: Option<&str>,
            limit: usize,
        ) -> Result<Vec<Document>> {
            let docs = self.documents.lock().unwrap();
            let mut filtered: Vec<Document> = docs
                .iter()
                .filter(|d| {
                    // Filter by date range
                    if let Some(from_ts) = from {
                        if d.last_modified < from_ts {
                            return false;
                        }
                    }
                    if let Some(to_ts) = to {
                        if d.last_modified > to_ts {
                            return false;
                        }
                    }
                    // Filter by folder
                    if let Some(folder_prefix) = folder {
                        let prefix = crate::path_utils::normalize_folder_prefix(folder_prefix);
                        if !d.source_file.starts_with(&prefix) {
                            return false;
                        }
                    }
                    true
                })
                .cloned()
                .collect();

            // Sort chronologically (oldest first)
            // Note: partial_cmp handles NaN gracefully by treating as equal
            filtered.sort_by(|a, b| {
                a.last_modified
                    .partial_cmp(&b.last_modified)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            // Apply limit
            filtered.truncate(limit);

            Ok(filtered)
        }
    }
}

#[cfg(test)]
mod trait_tests {
    use super::mocks::*;
    use super::*;

    #[tokio::test]
    async fn mock_vector_store_get_chunks_by_path() {
        let store = MockVectorStore::new();
        store.stored.lock().unwrap().push(StoredChunk {
            note_path: "a.md".into(),
            breadcrumb: "a.md > Title".into(),
            content: "Hello".into(),
            raw_content: "Hello".into(),
            vector: vec![0.0; 4],
            chunk_index: 0,
            content_hash: "abc".into(),
            links: vec![],
        });
        store.stored.lock().unwrap().push(StoredChunk {
            note_path: "b.md".into(),
            breadcrumb: "b.md".into(),
            content: "World".into(),
            raw_content: "World".into(),
            vector: vec![0.0; 4],
            chunk_index: 0,
            content_hash: "def".into(),
            links: vec![],
        });

        let chunks = store.get_chunks_by_path("a.md").await.unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].note_path, "a.md");
    }
}
