pub mod changes;
mod embedding_worker;
pub mod pipeline;
pub mod watcher;

use anyhow::{Context, Result};
use ignore::WalkBuilder;
use kajet_backend::similarity_graph::{SgemmGraphBuilder, load_all_chunk_embeddings};
use kajet_core::config::SimilarityGraphConfig;
use kajet_core::path_utils::{normalize_folder_prefix, path_matches_folder_prefix};
use kajet_core::similarity_graph::{ChunkEntry, GraphBuilder, HeadingRegexFilter};
use kajet_core::traits::{DocumentStore, Embedder, VectorStore};
use kajet_core::types::{FileChange, IndexStats, IndexerHandle};
use pipeline::IndexPipeline;
use std::cmp::Ordering;
use std::collections::{BTreeSet, HashSet};
use std::path::{Component, Path};
use std::sync::Arc;
use std::time::Instant;
use unicode_normalization::UnicodeNormalization;

pub struct Indexer {
    embedder: Arc<dyn Embedder>,
    store: Arc<dyn VectorStore>,
    doc_store: Arc<dyn DocumentStore>,
    db_path: Option<std::path::PathBuf>,
    max_concurrent: usize,
    buffer_size: usize,
    progress_percent_step: u8,
    created_date_field: Option<String>,
    modified_date_field: Option<String>,
    document_prefix: String,
    similarity_graph: SimilarityGraphConfig,
}

struct ReindexPlan {
    delete_paths: Vec<String>,
    reindex_paths: Vec<String>,
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
            db_path: None,
            max_concurrent: 16,
            buffer_size: 256,
            progress_percent_step: 5,
            created_date_field: None,
            modified_date_field: None,
            document_prefix: String::new(),
            similarity_graph: SimilarityGraphConfig::default(),
        }
    }

    pub fn with_db_path(mut self, db_path: std::path::PathBuf) -> Self {
        self.db_path = Some(db_path);
        self
    }

    pub fn with_similarity_graph_config(mut self, config: SimilarityGraphConfig) -> Self {
        self.similarity_graph = config;
        self
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

    pub fn with_document_prefix(mut self, prefix: String) -> Self {
        self.document_prefix = prefix;
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
        .with_document_prefix(self.document_prefix.clone())
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
        if let Err(e) = self.rebuild_similarity_graph().await {
            tracing::warn!("Failed to rebuild similarity graph after incremental reindex: {e}");
        }
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
        let existing_paths: Vec<String> = self
            .doc_store
            .get_document_hashes()
            .await?
            .into_keys()
            .collect();
        if !existing_paths.is_empty() {
            self.store.delete_chunks_by_paths(&existing_paths).await?;
            self.doc_store.delete_by_paths(&existing_paths).await?;
        }

        let changes = changes::detect_changes(
            vault_path,
            exclude_folders,
            &std::collections::HashMap::new(),
        )?;
        let stats = self.pipeline().run(changes, vault_path).await?;
        if let Err(e) = self.rebuild_similarity_graph().await {
            tracing::warn!("Failed to build similarity graph: {e}");
        }
        tracing::info!(
            elapsed_s = format!("{:.1}", start.elapsed().as_secs_f64()),
            "Full reindex finished in {:.1}s",
            start.elapsed().as_secs_f64()
        );
        Ok(stats)
    }

    /// Reindex specific files by their relative paths.
    pub async fn reindex_files(&self, vault_path: &Path, rel_paths: &[String]) -> Result<()> {
        let indexed_paths: HashSet<String> = self
            .doc_store
            .get_document_hashes()
            .await?
            .into_keys()
            .collect();

        let vault = vault_path.to_path_buf();
        let requested_paths = rel_paths.to_vec();
        let plan = tokio::task::spawn_blocking(move || {
            build_reindex_plan(&vault, &requested_paths, &indexed_paths)
        })
        .await
        .context("Failed to join reindex planning task")??;

        tracing::info!(
            requested = rel_paths.len(),
            to_delete = plan.delete_paths.len(),
            to_reindex = plan.reindex_paths.len(),
            "Reindex paths resolved"
        );

        if !plan.delete_paths.is_empty() {
            self.store
                .delete_chunks_by_paths(&plan.delete_paths)
                .await?;
            self.doc_store.delete_by_paths(&plan.delete_paths).await?;
        }

        let changes: Vec<FileChange> = plan
            .reindex_paths
            .iter()
            .map(|rel_path| FileChange::Added(vault_path.join(rel_path)))
            .collect();

        if !changes.is_empty() {
            self.pipeline().run(changes, vault_path).await?;
            if let Err(e) = self.rebuild_similarity_graph().await {
                tracing::warn!("Failed to rebuild similarity graph after partial reindex: {e}");
            }
        }

        Ok(())
    }
}

impl Indexer {
    async fn rebuild_similarity_graph(&self) -> Result<()> {
        if !self.similarity_graph.enabled {
            return Ok(());
        }
        let Some(db_path) = self.db_path.clone() else {
            tracing::debug!("Similarity graph build skipped: db_path not configured");
            return Ok(());
        };

        let graph_path = db_path.join("similarity_graph.kjsg");
        let start = std::time::Instant::now();

        let (dim, mut rows) = load_all_chunk_embeddings(&db_path).await?;
        if rows.is_empty() {
            self.try_remove_graph_file(&graph_path).await;
            anyhow::bail!("Cannot build similarity graph: chunks table is empty");
        }

        let filter = HeadingRegexFilter::new(&self.similarity_graph.boilerplate_patterns)?;
        rows.retain(|row| filter.include_breadcrumb(&row.breadcrumb));

        if rows.is_empty() {
            self.try_remove_graph_file(&graph_path).await;
            anyhow::bail!("No chunks left after similarity graph boilerplate filtering");
        }

        rows.sort_by(|a, b| match a.note_path.cmp(&b.note_path) {
            Ordering::Equal => a.chunk_index.cmp(&b.chunk_index),
            other => other,
        });

        let n_chunks = rows.len() as u32;
        let dim_usize = dim as usize;
        let mut embeddings = Vec::with_capacity(rows.len() * dim_usize);
        let mut chunk_to_doc: Vec<u16> = Vec::with_capacity(rows.len());

        let mut current_doc: Option<String> = None;
        let mut current_doc_idx: u16 = 0;

        for row in &rows {
            match current_doc.as_deref() {
                None => current_doc = Some(row.note_path.clone()),
                Some(doc) if doc != row.note_path => {
                    current_doc = Some(row.note_path.clone());
                    current_doc_idx = current_doc_idx
                        .checked_add(1)
                        .context("Too many documents for u16 doc index")?;
                }
                _ => {}
            }

            chunk_to_doc.push(current_doc_idx);
            anyhow::ensure!(
                row.vector.len() == dim_usize,
                "Embedding dimension mismatch for chunk {}:{}",
                row.note_path,
                row.chunk_index
            );
            embeddings.extend_from_slice(&row.vector);
        }

        normalize_embeddings_in_place(&mut embeddings, rows.len(), dim_usize)?;

        // Build KJSG v2 identity: string table + chunk entries
        const MAX_EXCERPT_CHARS: usize = 200;
        let mut string_table: Vec<u8> = Vec::new();
        let mut chunk_entries: Vec<ChunkEntry> = Vec::with_capacity(rows.len());

        for row in &rows {
            let np_offset = string_table.len() as u32;
            string_table.extend_from_slice(row.note_path.as_bytes());
            let np_len = row.note_path.len() as u16;

            let bc_offset = string_table.len() as u32;
            string_table.extend_from_slice(row.breadcrumb.as_bytes());
            let bc_len = row.breadcrumb.len() as u16;

            let excerpt = truncate_excerpt(&row.content, MAX_EXCERPT_CHARS);
            let ct_offset = string_table.len() as u32;
            string_table.extend_from_slice(excerpt.as_bytes());
            let ct_len = excerpt.len() as u16;

            chunk_entries.push(ChunkEntry {
                note_path_offset: np_offset,
                note_path_len: np_len,
                breadcrumb_offset: bc_offset,
                breadcrumb_len: bc_len,
                chunk_index: row.chunk_index as u16,
                content_offset: ct_offset,
                content_len: ct_len,
            });
        }

        let graph_path_clone = graph_path.clone();
        let k = self.similarity_graph.k;
        let chunk_to_doc_clone = chunk_to_doc.clone();

        let result = tokio::task::spawn_blocking(move || -> Result<(u32, u32)> {
            let builder = SgemmGraphBuilder;
            let graph = builder
                .build(&embeddings, n_chunks, dim, k, &chunk_to_doc_clone)?
                .with_identity(string_table, chunk_entries)?;
            let effective_k = graph.k;
            graph.save_to_path(&graph_path_clone)?;
            Ok((n_chunks, effective_k))
        })
        .await
        .context("Similarity graph build task panicked")??;

        tracing::info!(
            chunks = result.0,
            k = result.1,
            dim,
            elapsed_s = format!("{:.2}", start.elapsed().as_secs_f64()),
            "Similarity graph built"
        );

        Ok(())
    }

    async fn try_remove_graph_file(&self, path: &std::path::Path) {
        let _ = tokio::fs::remove_file(path).await;
    }
}

/// Truncate content to at most `max_chars` Unicode characters, cutting at a char boundary.
fn truncate_excerpt(content: &str, max_chars: usize) -> &str {
    match content.char_indices().nth(max_chars) {
        Some((byte_idx, _)) => &content[..byte_idx],
        None => content,
    }
}

fn normalize_embeddings_in_place(embeddings: &mut [f32], n_rows: usize, dim: usize) -> Result<()> {
    anyhow::ensure!(
        embeddings.len() == n_rows.saturating_mul(dim),
        "Invalid embeddings buffer size"
    );

    for row_idx in 0..n_rows {
        let start = row_idx * dim;
        let end = start + dim;
        let row = &mut embeddings[start..end];
        let norm_sq: f32 = row.iter().map(|v| v * v).sum();
        let norm = norm_sq.sqrt();
        anyhow::ensure!(norm > 1e-12, "Zero-norm embedding at row {row_idx}");
        if (norm - 1.0).abs() > 1e-3 {
            for v in row {
                *v /= norm;
            }
        }
    }

    Ok(())
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

fn build_reindex_plan(
    vault_path: &Path,
    requested_paths: &[String],
    indexed_paths: &HashSet<String>,
) -> Result<ReindexPlan> {
    let mut delete_paths: BTreeSet<String> = BTreeSet::new();
    let mut reindex_paths: BTreeSet<String> = BTreeSet::new();

    for raw_path in requested_paths {
        let requested_path = raw_path.trim();
        anyhow::ensure!(!requested_path.is_empty(), "Reindex path cannot be empty");
        validate_relative_path(requested_path)?;

        let normalized_requested = normalize_requested_path(requested_path);
        let absolute_path = vault_path.join(requested_path);

        if absolute_path.is_dir() {
            for indexed in indexed_paths {
                if is_within_folder(indexed, &normalized_requested) {
                    delete_paths.insert(indexed.clone());
                }
            }

            for rel in collect_markdown_paths(&absolute_path, vault_path)? {
                delete_paths.insert(rel.clone());
                reindex_paths.insert(rel);
            }

            continue;
        }

        if absolute_path.is_file() {
            if absolute_path.extension().is_none_or(|ext| ext != "md") {
                tracing::debug!(path = requested_path, "Skipping non-markdown reindex path");
                continue;
            }

            let rel = normalize_relative_path(&absolute_path, vault_path)?;
            delete_paths.insert(rel.clone());
            reindex_paths.insert(rel);
            continue;
        }

        let mut matched_index_prefix = false;
        for indexed in indexed_paths {
            if is_within_folder(indexed, &normalized_requested) {
                delete_paths.insert(indexed.clone());
                matched_index_prefix = true;
            }
        }

        if !matched_index_prefix {
            delete_paths.insert(normalized_requested);
        }
    }

    Ok(ReindexPlan {
        delete_paths: delete_paths.into_iter().collect(),
        reindex_paths: reindex_paths.into_iter().collect(),
    })
}

fn collect_markdown_paths(root: &Path, vault_path: &Path) -> Result<Vec<String>> {
    let mut builder = WalkBuilder::new(root);
    builder.standard_filters(true);

    let mut rel_paths = BTreeSet::new();
    for entry in builder.build() {
        let entry = entry?;
        if !entry.file_type().is_some_and(|ft| ft.is_file()) {
            continue;
        }

        let path = entry.path();
        if path.extension().is_none_or(|ext| ext != "md") {
            continue;
        }

        rel_paths.insert(normalize_relative_path(path, vault_path)?);
    }

    Ok(rel_paths.into_iter().collect())
}

fn validate_relative_path(path: &str) -> Result<()> {
    let candidate = Path::new(path);
    anyhow::ensure!(
        !candidate.is_absolute(),
        "Reindex path must be relative to vault root: {path}"
    );

    for component in candidate.components() {
        match component {
            Component::ParentDir => anyhow::bail!(
                "Reindex path cannot contain parent directory traversal ('..'): {path}"
            ),
            Component::Prefix(_) | Component::RootDir => {
                anyhow::bail!("Reindex path must be relative to vault root: {path}")
            }
            Component::CurDir | Component::Normal(_) => {}
        }
    }

    Ok(())
}

fn normalize_requested_path(path: &str) -> String {
    Path::new(path)
        .components()
        .filter_map(|component| match component {
            Component::CurDir => None,
            Component::Normal(seg) => Some(seg.to_string_lossy().into_owned()),
            Component::ParentDir | Component::Prefix(_) | Component::RootDir => None,
        })
        .collect::<Vec<_>>()
        .join("/")
        .nfc()
        .collect()
}

fn normalize_relative_path(path: &Path, vault_path: &Path) -> Result<String> {
    let rel = path
        .strip_prefix(vault_path)
        .with_context(|| {
            format!(
                "Path '{}' is outside vault '{}'",
                path.display(),
                vault_path.display()
            )
        })?
        .to_string_lossy()
        .nfc()
        .collect::<String>();

    Ok(rel)
}

fn is_within_folder(path: &str, folder: &str) -> bool {
    if folder.is_empty() {
        return true;
    }
    if path == folder {
        return true;
    }

    let prefix = normalize_folder_prefix(folder);
    path_matches_folder_prefix(path, Some(prefix.as_str()), true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kajet_core::traits::StoredChunk;
    use kajet_core::traits::mocks::{MockDocumentStore, MockEmbedder, MockVectorStore};
    use kajet_core::types::Document;
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

    #[tokio::test]
    async fn reindex_files_supports_directory_targets() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("journal/sub")).unwrap();
        fs::write(
            dir.path().join("journal/day1.md"),
            "# Day 1\n\nLorem ipsum dolor sit amet.",
        )
        .unwrap();
        fs::write(
            dir.path().join("journal/sub/day2.md"),
            "# Day 2\n\nConsectetur adipiscing elit.",
        )
        .unwrap();
        fs::write(dir.path().join("journal/skip.txt"), "skip").unwrap();

        let (indexer, _, doc_store) = make_indexer();
        indexer
            .reindex_files(dir.path(), &["journal".to_string()])
            .await
            .unwrap();

        let mut paths: Vec<String> = doc_store
            .documents
            .lock()
            .unwrap()
            .iter()
            .map(|d| d.source_file.clone())
            .collect();
        paths.sort();

        assert_eq!(
            paths,
            vec![
                "journal/day1.md".to_string(),
                "journal/sub/day2.md".to_string()
            ]
        );
    }

    #[tokio::test]
    async fn reindex_directory_removes_stale_documents() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("journal")).unwrap();
        fs::write(
            dir.path().join("journal/keep.md"),
            "# Keep\n\nLorem ipsum dolor sit amet.",
        )
        .unwrap();

        let (indexer, store, doc_store) = make_indexer();
        doc_store.documents.lock().unwrap().push(Document {
            source_file: "journal/deleted.md".to_string(),
            full_text: "deleted".to_string(),
            title: "deleted".to_string(),
            tags: Vec::new(),
            content_hash: "deleted".to_string(),
            last_modified: 0.0,
            outgoing_links: Vec::new(),
            backlinks: Vec::new(),
        });
        store.stored.lock().unwrap().push(StoredChunk {
            note_path: "journal/deleted.md".to_string(),
            breadcrumb: "deleted".to_string(),
            content: "deleted".to_string(),
            raw_content: "deleted".to_string(),
            vector: vec![0.0; 4],
            chunk_index: 0,
            content_hash: "deleted".to_string(),
            links: Vec::new(),
        });

        indexer
            .reindex_files(dir.path(), &["journal".to_string()])
            .await
            .unwrap();

        let docs = doc_store.documents.lock().unwrap();
        assert!(docs.iter().all(|d| d.source_file != "journal/deleted.md"));
        assert!(docs.iter().any(|d| d.source_file == "journal/keep.md"));

        let chunks = store.stored.lock().unwrap();
        assert!(chunks.iter().all(|c| c.note_path != "journal/deleted.md"));
    }

    #[tokio::test]
    async fn reindex_files_rejects_parent_directory_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let (indexer, _, _) = make_indexer();

        let err = indexer
            .reindex_files(dir.path(), &["../outside.md".to_string()])
            .await
            .unwrap_err();

        assert!(
            err.to_string()
                .contains("cannot contain parent directory traversal")
        );
    }

    #[tokio::test]
    async fn full_reindex_removes_stale_documents_before_rebuild() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("new.md"),
            "# New\n\nLorem ipsum dolor sit amet, consectetur adipiscing elit sed do eiusmod.",
        )
        .unwrap();

        let (indexer, store, doc_store) = make_indexer();
        doc_store.documents.lock().unwrap().push(Document {
            source_file: "old.md".to_string(),
            full_text: "old".to_string(),
            title: "old".to_string(),
            tags: Vec::new(),
            content_hash: "old".to_string(),
            last_modified: 0.0,
            outgoing_links: Vec::new(),
            backlinks: Vec::new(),
        });
        store.stored.lock().unwrap().push(StoredChunk {
            note_path: "old.md".to_string(),
            breadcrumb: "old".to_string(),
            content: "old".to_string(),
            raw_content: "old".to_string(),
            vector: vec![0.0; 4],
            chunk_index: 0,
            content_hash: "old".to_string(),
            links: Vec::new(),
        });

        indexer.full_reindex(dir.path(), &[]).await.unwrap();

        let docs = doc_store.documents.lock().unwrap();
        assert!(docs.iter().all(|d| d.source_file != "old.md"));
        assert!(docs.iter().any(|d| d.source_file == "new.md"));

        let chunks = store.stored.lock().unwrap();
        assert!(chunks.iter().all(|c| c.note_path != "old.md"));
    }
}
