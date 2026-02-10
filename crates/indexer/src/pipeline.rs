use crate::embedding_worker::{EmbeddingHandle, EmbeddingWorker};
use anyhow::Result;
use kajet_backend::hasher::hash_content;
use kajet_core::traits::{DocumentStore, Embedder, StoredChunk, VectorStore};
use kajet_core::types::{Document, FileChange, IndexStats};
use kajet_parser::ChunkConfig;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{Semaphore, mpsc};
use unicode_normalization::UnicodeNormalization;

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
    progress_percent_step: u8,
    created_date_field: Option<String>,
    modified_date_field: Option<String>,
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
            progress_percent_step: 5,
            created_date_field: None,
            modified_date_field: None,
        }
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
            for path in &deleted {
                tracing::debug!(path = %path, "file:delete");
            }
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
            // Build filename→path lookup for all files in the vault
            let all_vault_files = scan_vault_files(vault_path)?;
            let filename_lookup = build_filename_lookup(&all_vault_files, vault_path);

            self.process_files(&files_to_process, vault_path, &filename_lookup)
                .await?;
        }

        // Compute backlinks asynchronously
        let doc_store = self.doc_store.clone();
        tokio::spawn(async move {
            if let Err(e) = compute_and_store_backlinks(&*doc_store).await {
                tracing::warn!("Failed to compute backlinks: {e}");
            }
        });

        // Create FTS index
        if let Err(e) = self.doc_store.create_fts_index().await {
            tracing::warn!("Failed to create FTS index: {e}");
        }

        self.doc_store.get_index_stats().await
    }

    async fn process_files(
        &self,
        files: &[PathBuf],
        vault_path: &Path,
        filename_lookup: &HashMap<String, String>,
    ) -> Result<()> {
        let semaphore = Arc::new(Semaphore::new(self.max_concurrent));
        let (result_tx, mut result_rx) = mpsc::channel::<Result<ProcessedFile>>(self.buffer_size);

        // Start the embedding worker with the embedder
        let embedding_handle = EmbeddingWorker::spawn(self.embedder.clone());

        let total = files.len();
        let start = Instant::now();

        // Spawn file processing tasks in a separate task so the receiver loop
        // can drain results concurrently — prevents deadlock when the channel
        // fills up (buffer_size < total files).
        let spawn_files: Vec<PathBuf> = files.to_vec();
        let spawn_vault = vault_path.to_path_buf();
        let spawn_chunk_config = self.chunk_config.clone();
        let spawn_lookup = Arc::new(filename_lookup.clone());
        let spawn_created_field = self.created_date_field.clone();
        let spawn_modified_field = self.modified_date_field.clone();

        tokio::spawn(async move {
            for file_path in &spawn_files {
                let Ok(permit) = semaphore.clone().acquire_owned().await else {
                    break;
                };
                let tx = result_tx.clone();
                let embed_handle = embedding_handle.clone();
                let chunk_config = spawn_chunk_config.clone();
                let vault = spawn_vault.clone();
                let path = file_path.clone();
                let lookup = spawn_lookup.clone();

                let created_field = spawn_created_field.clone();
                let modified_field = spawn_modified_field.clone();
                tokio::spawn(async move {
                    let result = process_single_file(
                        &path,
                        &vault,
                        &embed_handle,
                        &chunk_config,
                        &lookup,
                        created_field.as_deref(),
                        modified_field.as_deref(),
                    )
                    .await;
                    let _ = tx.send(result).await;
                    drop(permit);
                });
            }
            // result_tx (original) dropped here → receiver terminates after all clones are done
        });

        // Collect and store results with progress tracking (runs concurrently with spawning)
        let mut processed_count: usize = 0;
        let mut total_chunks: usize = 0;
        let mut next_threshold = self.progress_percent_step as usize;

        while let Some(result) = result_rx.recv().await {
            match result {
                Ok(processed) => {
                    let chunk_count = processed.stored_chunks.len();
                    self.doc_store.store_documents(&[processed.doc]).await?;
                    if !processed.stored_chunks.is_empty() {
                        self.store.upsert_chunks(&processed.stored_chunks).await?;
                    }
                    processed_count += 1;
                    total_chunks += chunk_count;

                    // Percentage progress reporting
                    if total > 0 && self.progress_percent_step > 0 {
                        let pct = (processed_count * 100) / total;
                        if pct >= next_threshold {
                            tracing::info!(
                                processed = processed_count,
                                total,
                                elapsed_s = format!("{:.1}", start.elapsed().as_secs_f64()),
                                "Indexing: {}% ({}/{} files)",
                                pct,
                                processed_count,
                                total
                            );
                            next_threshold = pct + self.progress_percent_step as usize;
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("Pipeline: failed to process file: {e}");
                    processed_count += 1;
                }
            }
        }

        let elapsed = start.elapsed().as_secs_f64();
        tracing::info!(
            files = processed_count,
            chunks = total_chunks,
            elapsed_s = format!("{elapsed:.1}"),
            "Indexing complete: {} files, {} chunks — total {:.1}s",
            processed_count,
            total_chunks,
            elapsed
        );

        Ok(())
    }
}

/// Process a single file: read, hash, parse, resolve links, embed.
async fn process_single_file(
    path: &Path,
    vault_path: &Path,
    embedding_handle: &EmbeddingHandle,
    chunk_config: &ChunkConfig,
    filename_lookup: &HashMap<String, String>,
    created_field: Option<&str>,
    modified_field: Option<&str>,
) -> Result<ProcessedFile> {
    let file_start = Instant::now();
    let rel_path = path
        .strip_prefix(vault_path)
        .unwrap_or(path)
        .to_string_lossy()
        .nfc()
        .collect::<String>();

    tracing::trace!(path = %rel_path, "file:start");

    let content = tokio::fs::read_to_string(path).await?;
    let content_hash = hash_content(&content);
    let mtime = std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);

    let (parsed_doc, mut chunks) = kajet_parser::parse_document_with_date_fields(
        &rel_path,
        &content,
        chunk_config,
        created_field,
        modified_field,
    );
    let chunk_count = chunks.len();
    tracing::trace!(
        path = %rel_path,
        chunks = chunk_count,
        duration_ms = file_start.elapsed().as_millis() as u64,
        "file:parsed"
    );

    // Resolve wikilink targets to vault-relative paths
    for chunk in &mut chunks {
        for link in &mut chunk.links {
            link.resolved_path = filename_lookup.get(&link.target).cloned();
        }
    }

    let outgoing_links: Vec<String> = chunks
        .iter()
        .flat_map(|c| c.links.iter())
        .filter_map(|l| l.resolved_path.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    // Priority: frontmatter modified > frontmatter created > filesystem mtime
    let effective_date = parsed_doc
        .frontmatter_modified
        .or(parsed_doc.frontmatter_created)
        .unwrap_or(mtime);

    let doc = Document {
        source_file: parsed_doc.source_file,
        full_text: parsed_doc.full_text,
        title: parsed_doc.title,
        tags: parsed_doc.tags,
        content_hash: content_hash.clone(),
        last_modified: effective_date,
        outgoing_links,
        backlinks: Vec::new(),
    };

    let stored_chunks = if !chunks.is_empty() {
        let embed_start = Instant::now();
        let texts: Vec<String> = chunks.iter().map(|c| c.embed_text()).collect();

        // Use the embedding handle instead of calling embed directly
        let embeddings = embedding_handle.embed(texts).await?;

        tracing::trace!(
            path = %rel_path,
            chunks = chunk_count,
            duration_ms = embed_start.elapsed().as_millis() as u64,
            "file:embedded"
        );

        chunks
            .into_iter()
            .zip(embeddings)
            .map(|(chunk, vector)| StoredChunk {
                note_path: chunk.note_path,
                breadcrumb: chunk.breadcrumb,
                content: chunk.content,
                raw_content: chunk.raw_content,
                vector,
                chunk_index: chunk.chunk_index,
                content_hash: content_hash.clone(),
                links: chunk.links,
            })
            .collect()
    } else {
        Vec::new()
    };

    tracing::trace!(
        path = %rel_path,
        total_ms = file_start.elapsed().as_millis() as u64,
        "file:done"
    );

    Ok(ProcessedFile { doc, stored_chunks })
}

/// Scan vault directory for all .md files, returning their paths.
fn scan_vault_files(vault_path: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    fn walk(dir: &Path, files: &mut Vec<PathBuf>) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk(&path, files);
                } else if path.extension().is_some_and(|ext| ext == "md") {
                    files.push(path);
                }
            }
        }
    }
    walk(vault_path, &mut files);
    Ok(files)
}

/// Build a lookup from filename stem → vault-relative path.
/// For duplicate names, picks the shortest path (Obsidian default).
fn build_filename_lookup(files: &[PathBuf], vault_path: &Path) -> HashMap<String, String> {
    let mut lookup: HashMap<String, Vec<String>> = HashMap::new();
    for file in files {
        let rel = file
            .strip_prefix(vault_path)
            .unwrap_or(file)
            .to_string_lossy()
            .nfc()
            .collect::<String>();
        if let Some(stem) = file
            .file_stem()
            .map(|s| s.to_string_lossy().nfc().collect::<String>())
        {
            lookup.entry(stem).or_default().push(rel);
        }
    }
    lookup
        .into_iter()
        .map(|(name, mut paths)| {
            paths.sort_by_key(|p| p.len());
            (name, paths.into_iter().next().unwrap())
        })
        .collect()
}

/// Compute backlinks from all documents' outgoing_links and store them.
async fn compute_and_store_backlinks(doc_store: &dyn DocumentStore) -> Result<()> {
    let docs = doc_store.get_all_documents().await?;
    let mut backlink_map: HashMap<String, Vec<String>> = HashMap::new();

    for doc in &docs {
        for target in &doc.outgoing_links {
            backlink_map
                .entry(target.clone())
                .or_default()
                .push(doc.source_file.clone());
        }
    }

    if !backlink_map.is_empty() {
        doc_store.update_backlinks(&backlink_map).await?;
        tracing::debug!(
            targets = backlink_map.len(),
            "Backlinks computed and stored"
        );
    }

    Ok(())
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
        fs::write(
            dir.path().join("a.md"),
            "# A\n\nLorem ipsum dolor sit amet, consectetur adipiscing elit sed do eiusmod.",
        )
        .unwrap();
        fs::write(
            dir.path().join("b.md"),
            "# B\n\nUt enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi.",
        )
        .unwrap();

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
                outgoing_links: vec![],
                backlinks: vec![],
            }])
            .await
            .unwrap();

        let changes = vec![FileChange::Deleted("old.md".into())];
        let stats = pipeline.run(changes, dir.path()).await.unwrap();
        assert_eq!(stats.total_documents, 0);
    }

    #[test]
    fn build_filename_lookup_normalizes_nfd_to_nfc() {
        use std::path::PathBuf;
        use unicode_normalization::UnicodeNormalization;

        let vault = PathBuf::from("/vault");
        // NFD: e + combining ogonek (U+0328) + s + combining acute (U+0301)
        let nfd_name = "not\u{0328}s\u{0301}.md";
        let nfc_name: String = nfd_name.nfc().collect();
        assert_ne!(nfd_name, nfc_name, "NFD and NFC should differ");

        let files = vec![vault.join(nfd_name)];
        let lookup = build_filename_lookup(&files, &vault);

        // Both key (stem) and value (rel path) should be NFC
        let nfc_stem: String = "not\u{0328}s\u{0301}".nfc().collect();
        assert!(
            lookup.contains_key(&nfc_stem),
            "lookup key should be NFC, got keys: {:?}",
            lookup.keys().collect::<Vec<_>>()
        );
        assert_eq!(lookup[&nfc_stem], nfc_name);
    }

    #[test]
    fn build_filename_lookup_normalizes_polish_nfd() {
        use std::path::PathBuf;
        use unicode_normalization::UnicodeNormalization;

        let vault = PathBuf::from("/vault");
        // "żółć" in NFD — each accented char decomposed
        let nfd = "z\u{0307}o\u{0301}l\u{0142}c\u{0301}";
        let nfc: String = nfd.nfc().collect();

        let files = vec![vault.join("Dzienniki").join(format!("{nfd}.md"))];
        let lookup = build_filename_lookup(&files, &vault);

        let nfc_stem: String = nfd.nfc().collect();
        assert!(lookup.contains_key(&nfc_stem));
        assert_eq!(
            lookup[&nfc_stem],
            format!("Dzienniki/{nfc}.md"),
            "rel path should be NFC"
        );
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
