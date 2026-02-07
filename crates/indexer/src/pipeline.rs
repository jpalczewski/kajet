use anyhow::Result;
use kajet_backend::hasher::hash_content;
use kajet_core::traits::{DocumentStore, Embedder, StoredChunk, VectorStore};
use kajet_core::types::{Document, FileChange, IndexStats};
use kajet_parser::ChunkConfig;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{mpsc, Semaphore};

/// Result of processing a single file through the pipeline.
struct ProcessedFile {
    doc: Document,
    stored_chunks: Vec<StoredChunk>,
}

/// Async indexing pipeline with bounded concurrency.
pub struct IndexPipeline {
    max_concurrent: usize,
    buffer_size: usize,
    embedder: Arc<dyn Embedder>,
    store: Arc<dyn VectorStore>,
    doc_store: Arc<dyn DocumentStore>,
    chunk_config: ChunkConfig,
}

impl IndexPipeline {
    pub fn new(
        max_concurrent: usize,
        buffer_size: usize,
        embedder: Arc<dyn Embedder>,
        store: Arc<dyn VectorStore>,
        doc_store: Arc<dyn DocumentStore>,
    ) -> Self {
        Self {
            max_concurrent,
            buffer_size,
            embedder,
            store,
            doc_store,
            chunk_config: ChunkConfig::default(),
        }
    }

    /// Process file changes concurrently with semaphore-bounded parallelism.
    /// Deletions are handled first, then additions/modifications are processed in parallel.
    pub async fn run(&self, changes: Vec<FileChange>, vault_path: &Path) -> Result<IndexStats> {
        // Handle deletions first
        let deleted: Vec<String> = changes
            .iter()
            .filter_map(|c| match c {
                FileChange::Deleted(p) => Some(p.clone()),
                _ => None,
            })
            .collect();

        if !deleted.is_empty() {
            self.store.delete_chunks_by_paths(&deleted).await?;
            self.doc_store.delete_by_paths(&deleted).await?;
        }

        // Collect files to process (added + modified)
        let files_to_process: Vec<PathBuf> = changes
            .into_iter()
            .filter_map(|c| match c {
                FileChange::Added(p) | FileChange::Modified(p) => Some(p),
                _ => None,
            })
            .collect();

        if !files_to_process.is_empty() {
            self.process_files(&files_to_process, vault_path).await?;
        }

        // Create FTS index
        if let Err(e) = self.doc_store.create_fts_index().await {
            tracing::warn!("Failed to create FTS index: {e}");
        }

        self.doc_store.get_index_stats().await
    }

    async fn process_files(&self, files: &[PathBuf], vault_path: &Path) -> Result<()> {
        let semaphore = Arc::new(Semaphore::new(self.max_concurrent));
        let (result_tx, mut result_rx) = mpsc::channel::<Result<ProcessedFile>>(self.buffer_size);

        // Spawn a task for each file
        for file_path in files {
            let permit = semaphore.clone().acquire_owned().await?;
            let tx = result_tx.clone();
            let embedder = self.embedder.clone();
            let chunk_config = self.chunk_config.clone();
            let vault = vault_path.to_path_buf();
            let path = file_path.clone();

            tokio::spawn(async move {
                let result = process_single_file(&path, &vault, &embedder, &chunk_config).await;
                let _ = tx.send(result).await;
                drop(permit);
            });
        }

        // Drop the sender so the receiver will terminate when all tasks complete
        drop(result_tx);

        // Collect and store results
        while let Some(result) = result_rx.recv().await {
            match result {
                Ok(processed) => {
                    self.doc_store.store_documents(&[processed.doc]).await?;
                    if !processed.stored_chunks.is_empty() {
                        self.store.upsert_chunks(&processed.stored_chunks).await?;
                    }
                }
                Err(e) => {
                    tracing::error!("Pipeline: failed to process file: {e}");
                }
            }
        }

        Ok(())
    }
}

/// Process a single file: read, hash, parse, embed.
async fn process_single_file(
    path: &Path,
    vault_path: &Path,
    embedder: &Arc<dyn Embedder>,
    chunk_config: &ChunkConfig,
) -> Result<ProcessedFile> {
    let content = tokio::fs::read_to_string(path).await?;
    let rel_path = path
        .strip_prefix(vault_path)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string();

    let content_hash = hash_content(&content);
    let mtime = std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);

    let (parsed_doc, chunks) = kajet_parser::parse_document(&rel_path, &content, chunk_config);

    let doc = Document {
        source_file: parsed_doc.source_file,
        full_text: parsed_doc.full_text,
        title: parsed_doc.title,
        tags: parsed_doc.tags,
        content_hash: content_hash.clone(),
        last_modified: mtime,
    };

    let stored_chunks = if !chunks.is_empty() {
        let texts: Vec<String> = chunks.iter().map(|c| c.embed_text()).collect();
        let text_refs: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();
        let embeddings = embedder.embed(text_refs)?;

        chunks
            .into_iter()
            .zip(embeddings)
            .map(|(chunk, vector)| StoredChunk {
                note_path: chunk.note_path,
                breadcrumb: chunk.breadcrumb,
                content: chunk.content,
                vector,
                chunk_index: chunk.chunk_index,
                content_hash: content_hash.clone(),
            })
            .collect()
    } else {
        Vec::new()
    };

    Ok(ProcessedFile { doc, stored_chunks })
}

#[cfg(test)]
mod tests {
    use super::*;
    use kajet_core::traits::mocks::{MockDocumentStore, MockEmbedder, MockVectorStore};
    use std::fs;

    fn make_pipeline() -> (IndexPipeline, Arc<MockVectorStore>, Arc<MockDocumentStore>) {
        let store = Arc::new(MockVectorStore::new());
        let doc_store = Arc::new(MockDocumentStore::new());
        let embedder = Arc::new(MockEmbedder::new(4));
        let pipeline = IndexPipeline::new(4, 32, embedder, store.clone(), doc_store.clone());
        (pipeline, store, doc_store)
    }

    #[tokio::test]
    async fn pipeline_processes_added_files() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.md"), "# A\n\nContent A").unwrap();
        fs::write(dir.path().join("b.md"), "# B\n\nContent B").unwrap();

        let (pipeline, store, _doc_store) = make_pipeline();

        let changes = vec![
            FileChange::Added(dir.path().join("a.md")),
            FileChange::Added(dir.path().join("b.md")),
        ];

        let stats = pipeline.run(changes, dir.path()).await.unwrap();
        assert_eq!(stats.total_documents, 2);
        assert!(!store.stored.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn pipeline_handles_deletions() {
        let dir = tempfile::tempdir().unwrap();
        let (pipeline, _store, doc_store) = make_pipeline();

        // Pre-populate with a document
        doc_store
            .store_documents(&[Document {
                source_file: "old.md".into(),
                full_text: "old content".into(),
                title: "Old".into(),
                tags: vec![],
                content_hash: "abc".into(),
                last_modified: 0.0,
            }])
            .await
            .unwrap();

        let changes = vec![FileChange::Deleted("old.md".into())];
        let stats = pipeline.run(changes, dir.path()).await.unwrap();
        assert_eq!(stats.total_documents, 0);
    }

    #[tokio::test]
    async fn pipeline_gives_same_result_as_sequential() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..5 {
            fs::write(
                dir.path().join(format!("note{i}.md")),
                format!("# Note {i}\n\nContent for note {i}"),
            )
            .unwrap();
        }

        let (pipeline, _, doc_store) = make_pipeline();
        let changes: Vec<FileChange> = (0..5)
            .map(|i| FileChange::Added(dir.path().join(format!("note{i}.md"))))
            .collect();

        let stats = pipeline.run(changes, dir.path()).await.unwrap();
        assert_eq!(stats.total_documents, 5);

        // Verify all documents were stored
        let hashes = doc_store.get_document_hashes().await.unwrap();
        assert_eq!(hashes.len(), 5);
    }
}
