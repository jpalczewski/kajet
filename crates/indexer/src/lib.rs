pub mod changes;
mod embedding_worker;
pub mod pipeline;
pub mod watcher;

use anyhow::Result;
use kajet_core::traits::{DocumentStore, Embedder, VectorStore};
use kajet_core::types::{FileChange, IndexStats, IndexerHandle};
use pipeline::IndexPipeline;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

pub struct Indexer {
    embedder: Arc<dyn Embedder>,
    store: Arc<dyn VectorStore>,
    doc_store: Arc<dyn DocumentStore>,
    max_concurrent: usize,
    buffer_size: usize,
    progress_percent_step: u8,
    created_date_field: Option<String>,
    modified_date_field: Option<String>,
}

impl Indexer {
    pub fn new(
        embedder: Arc<dyn Embedder>,
        store: Arc<dyn VectorStore>,
        doc_store: Arc<dyn DocumentStore>,
    ) -> Self {
        Self {
            embedder,
            store,
            doc_store,
            max_concurrent: 16,
            buffer_size: 256,
            progress_percent_step: 5,
            created_date_field: None,
            modified_date_field: None,
        }
    }

    pub fn with_concurrency(mut self, max_concurrent: usize, buffer_size: usize) -> Self {
        self.max_concurrent = max_concurrent;
        self.buffer_size = buffer_size;
        self
    }

    pub fn with_progress_step(mut self, step: u8) -> Self {
        self.progress_percent_step = step;
        self
    }

    pub fn with_date_fields(
        mut self,
        created_field: Option<String>,
        modified_field: Option<String>,
    ) -> Self {
        self.created_date_field = created_field;
        self.modified_date_field = modified_field;
        self
    }

    fn pipeline(&self) -> IndexPipeline {
        IndexPipeline::new(
            self.max_concurrent,
            self.buffer_size,
            self.embedder.clone(),
            self.store.clone(),
            self.doc_store.clone(),
        )
        .with_progress_step(self.progress_percent_step)
        .with_date_fields(
            self.created_date_field.clone(),
            self.modified_date_field.clone(),
        )
    }

    /// Incremental index: only process added/modified/deleted files.
    pub async fn incremental_index(
        &self,
        vault_path: &Path,
        exclude_folders: &[String],
    ) -> Result<IndexStats> {
        let start = Instant::now();
        let stored_hashes = self.doc_store.get_document_hashes().await?;
        let changes = changes::detect_changes(vault_path, exclude_folders, &stored_hashes)?;

        if changes.is_empty() {
            tracing::info!("No changes detected");
            return self.doc_store.get_index_stats().await;
        }

        let (added, modified, deleted) = categorize_changes(&changes);
        tracing::info!(
            added = added.len(),
            modified = modified.len(),
            deleted = deleted.len(),
            "{} added, {} modified, {} deleted",
            added.len(),
            modified.len(),
            deleted.len()
        );

        let stats = self.pipeline().run(changes, vault_path).await?;
        tracing::info!(
            elapsed_s = format!("{:.1}", start.elapsed().as_secs_f64()),
            "Incremental indexing finished in {:.1}s",
            start.elapsed().as_secs_f64()
        );
        Ok(stats)
    }

    /// Full reindex: drop everything and reindex all files.
    pub async fn full_reindex(
        &self,
        vault_path: &Path,
        exclude_folders: &[String],
    ) -> Result<IndexStats> {
        let start = Instant::now();
        let changes = changes::detect_changes(
            vault_path,
            exclude_folders,
            &std::collections::HashMap::new(),
        )?;
        let stats = self.pipeline().run(changes, vault_path).await?;
        tracing::info!(
            elapsed_s = format!("{:.1}", start.elapsed().as_secs_f64()),
            "Full reindex finished in {:.1}s",
            start.elapsed().as_secs_f64()
        );
        Ok(stats)
    }

    /// Reindex specific files by their relative paths.
    pub async fn reindex_files(&self, vault_path: &Path, rel_paths: &[String]) -> Result<()> {
        // Delete old data for these files
        self.store.delete_chunks_by_paths(rel_paths).await?;
        self.doc_store.delete_by_paths(rel_paths).await?;

        // Build changes for files that still exist
        let changes: Vec<FileChange> = rel_paths
            .iter()
            .filter_map(|rel_path| {
                let full_path = vault_path.join(rel_path);
                full_path.exists().then_some(FileChange::Added(full_path))
            })
            .collect();

        if !changes.is_empty() {
            self.pipeline().run(changes, vault_path).await?;
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl IndexerHandle for Indexer {
    async fn full_reindex(
        &self,
        vault_path: &Path,
        exclude_folders: &[String],
    ) -> Result<IndexStats> {
        self.full_reindex(vault_path, exclude_folders).await
    }

    async fn reindex_files(&self, vault_path: &Path, rel_paths: &[String]) -> Result<()> {
        self.reindex_files(vault_path, rel_paths).await
    }

    async fn get_index_stats(&self) -> Result<IndexStats> {
        self.doc_store.get_index_stats().await
    }
}

fn categorize_changes(changes: &[FileChange]) -> (Vec<&Path>, Vec<&Path>, Vec<&str>) {
    let mut added = Vec::new();
    let mut modified = Vec::new();
    let mut deleted = Vec::new();

    for change in changes {
        match change {
            FileChange::Added(p) => added.push(p.as_path()),
            FileChange::Modified(p) => modified.push(p.as_path()),
            FileChange::Deleted(p) => deleted.push(p.as_str()),
        }
    }

    (added, modified, deleted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kajet_core::traits::mocks::{MockDocumentStore, MockEmbedder, MockVectorStore};
    use std::fs;

    fn make_indexer() -> (Indexer, Arc<MockVectorStore>, Arc<MockDocumentStore>) {
        let store = Arc::new(MockVectorStore::new());
        let doc_store = Arc::new(MockDocumentStore::new());
        let embedder = Arc::new(MockEmbedder::new(4));
        let indexer = Indexer::new(embedder, store.clone(), doc_store.clone());
        (indexer, store, doc_store)
    }

    #[tokio::test]
    async fn incremental_index_first_run() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("note.md"),
            "# Hello\n\nLorem ipsum dolor sit amet, consectetur adipiscing elit sed do eiusmod.",
        )
        .unwrap();

        let (indexer, store, _doc_store) = make_indexer();
        let stats = indexer.incremental_index(dir.path(), &[]).await.unwrap();

        assert_eq!(stats.total_documents, 1);
        assert!(!store.stored.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn incremental_index_no_changes() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("note.md"),
            "# Hello\n\nLorem ipsum dolor sit amet, consectetur adipiscing elit sed do eiusmod.",
        )
        .unwrap();

        let (indexer, _, _) = make_indexer();
        // First index
        indexer.incremental_index(dir.path(), &[]).await.unwrap();
        // Second index — should detect no changes since hashes match
        let stats = indexer.incremental_index(dir.path(), &[]).await.unwrap();
        assert_eq!(stats.total_documents, 1);
    }

    #[tokio::test]
    async fn reindex_files_updates_specific_file() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("note.md"),
            "# Hello\n\nLorem ipsum dolor sit amet, consectetur adipiscing elit sed do eiusmod.",
        )
        .unwrap();

        let (indexer, _, _) = make_indexer();
        indexer
            .reindex_files(dir.path(), &["note.md".to_string()])
            .await
            .unwrap();
    }
}
