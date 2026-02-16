use crate::embedding_input::apply_prefix;
use crate::path_utils::note_path_fuzzy_matches;
use crate::traits::{DocumentStore, Embedder, VectorStore};
use crate::types::{Document, SearchType};
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub struct ExamineResult {
    pub document: Document,
}

#[derive(Debug, Clone)]
pub enum ExamineManyResult {
    Success {
        requested_path: String,
        document: Document,
    },
    Error {
        requested_path: String,
        error: String,
    },
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SearchResult {
    pub note_path: String,
    pub breadcrumb: String,
    pub content: String,
    pub raw_content: String,
    pub links: Vec<kajet_parser::Link>,
    pub score: f32,
    pub search_type: SearchType,
    pub chunk_index: u32,
}

pub struct SearchEngine {
    embedder: Arc<RwLock<Arc<dyn Embedder>>>,
    store: Arc<dyn VectorStore>,
    doc_store: Arc<dyn DocumentStore>,
    query_prefix: String,
}

impl SearchEngine {
    pub fn new(
        embedder: Arc<dyn Embedder>,
        store: Arc<dyn VectorStore>,
        doc_store: Arc<dyn DocumentStore>,
    ) -> Self {
        Self {
            embedder: Arc::new(RwLock::new(embedder)),
            store,
            doc_store,
            query_prefix: String::new(),
        }
    }

    /// Swap the embedder at runtime (e.g., when config changes).
    /// After swapping, caller should trigger full reindex.
    pub async fn swap_embedder(&self, new_embedder: Arc<dyn Embedder>) {
        *self.embedder.write().await = new_embedder;
    }

    pub async fn embedder(&self) -> Arc<dyn Embedder> {
        self.embedder.read().await.clone()
    }

    pub fn store(&self) -> &Arc<dyn VectorStore> {
        &self.store
    }

    pub fn doc_store(&self) -> &Arc<dyn DocumentStore> {
        &self.doc_store
    }

    pub fn with_query_prefix(mut self, prefix: String) -> Self {
        self.query_prefix = prefix;
        self
    }

    #[cfg(any(test, feature = "test-utils"))]
    pub fn __test_new() -> Self {
        use crate::traits::mocks::{MockDocumentStore, MockEmbedder, MockVectorStore};
        Self::new(
            Arc::new(MockEmbedder::new(384)),
            Arc::new(MockVectorStore::new()),
            Arc::new(MockDocumentStore::new()),
        )
    }

    /// Hybrid search: vector similarity + FTS, merged by weighted scoring.
    #[tracing::instrument(level = "debug", skip(self), fields(vector_hits, fts_hits))]
    pub async fn hybrid_search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let overall_start = std::time::Instant::now();
        let fetch_limit = limit * 2;

        tracing::debug!(query_len = query.len(), limit, "hybrid_search start");

        // Run vector and FTS searches in parallel
        let (vector_results, fts_results) = tokio::join!(
            self.vector_search(query, fetch_limit),
            self.fts_search(query, fetch_limit)
        );

        let vector_results = vector_results?;
        let fts_results = fts_results?;

        let span = tracing::Span::current();
        span.record("vector_hits", vector_results.len());
        span.record("fts_hits", fts_results.len());

        // If one fails or is empty, just return the other
        if vector_results.is_empty() {
            return Ok(fts_results.into_iter().take(limit).collect());
        }
        if fts_results.is_empty() {
            return Ok(vector_results.into_iter().take(limit).collect());
        }

        // Merge results
        let merged = merge_results(&vector_results, &fts_results, 0.6, 0.4);

        tracing::debug!(
            score_min = merged.last().map(|r| r.score),
            score_max = merged.first().map(|r| r.score),
            elapsed_ms = overall_start.elapsed().as_millis() as u64,
            results = merged.len().min(limit),
            "hybrid merge complete"
        );

        Ok(merged.into_iter().take(limit).collect())
    }

    /// Pure vector similarity search.
    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn vector_search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let embed_start = std::time::Instant::now();
        let embedder = self.embedder.read().await.clone();
        let prefixed = apply_prefix(&self.query_prefix, query);
        let query_emb = embedder.embed(vec![prefixed.as_str()]).await?;

        tracing::debug!(
            embed_ms = embed_start.elapsed().as_millis() as u64,
            "query embedded"
        );

        let hits = self.store.search(&query_emb[0], limit).await?;

        Ok(hits
            .into_iter()
            .map(|hit| SearchResult {
                note_path: hit.note_path,
                breadcrumb: hit.breadcrumb,
                content: hit.content,
                raw_content: hit.raw_content,
                links: hit.links,
                score: hit.distance,
                search_type: SearchType::Vector,
                chunk_index: hit.chunk_index,
            })
            .collect())
    }

    /// Examine a document by path (exact or fuzzy match).
    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn examine(&self, path: &str) -> Result<ExamineResult> {
        // 1. Exact match
        if let Some(doc) = self.doc_store.get_document_by_path(path).await? {
            return Ok(ExamineResult { document: doc });
        }

        // 2. Fuzzy: suffix match on all documents
        let all_docs = self.doc_store.get_all_documents().await?;
        let doc = resolve_examine_document(&all_docs, path)?;
        Ok(ExamineResult {
            document: doc.clone(),
        })
    }

    /// Batch examine multiple documents by path with a single document fetch.
    #[tracing::instrument(level = "debug", skip(self, paths), fields(paths_count = paths.len()))]
    pub async fn examine_many(&self, paths: &[String]) -> Result<Vec<ExamineManyResult>> {
        let all_docs = self.doc_store.get_all_documents().await?;
        let exact_map: HashMap<&str, &Document> = all_docs
            .iter()
            .map(|doc| (doc.source_file.as_str(), doc))
            .collect();

        let mut out = Vec::with_capacity(paths.len());
        for path in paths {
            if let Some(doc) = exact_map.get(path.as_str()) {
                out.push(ExamineManyResult::Success {
                    requested_path: path.clone(),
                    document: (*doc).clone(),
                });
                continue;
            }

            match resolve_examine_document(&all_docs, path) {
                Ok(doc) => out.push(ExamineManyResult::Success {
                    requested_path: path.clone(),
                    document: doc.clone(),
                }),
                Err(error) => out.push(ExamineManyResult::Error {
                    requested_path: path.clone(),
                    error: error.to_string(),
                }),
            }
        }

        Ok(out)
    }

    /// Pure full-text search.
    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn fts_search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let hits = self.doc_store.fts_search(query, limit).await?;

        Ok(hits
            .into_iter()
            .map(|hit| {
                let links = kajet_parser::extract_wikilinks(&hit.content_snippet);
                let content = kajet_parser::resolve_wikilinks_in_text(&hit.content_snippet);
                SearchResult {
                    note_path: hit.source_file,
                    breadcrumb: hit.title,
                    content,
                    raw_content: hit.content_snippet,
                    links,
                    score: hit.score,
                    search_type: SearchType::Fts,
                    chunk_index: 0,
                }
            })
            .collect())
    }
}

pub fn resolve_examine_document<'a>(all_docs: &'a [Document], path: &str) -> Result<&'a Document> {
    let matches: Vec<&Document> = all_docs
        .iter()
        .filter(|d| note_path_fuzzy_matches(&d.source_file, path, true))
        .collect();

    match matches.len() {
        0 => anyhow::bail!("{}", t!("examine_not_found", path = path)),
        1 => Ok(matches[0]),
        _ => {
            let candidates = matches
                .iter()
                .map(|d| format!("  - {}", d.source_file))
                .collect::<Vec<_>>()
                .join("\n");
            anyhow::bail!(
                "{}",
                t!("examine_ambiguous", path = path, candidates = candidates)
            )
        }
    }
}

/// Normalize scores to 0.0..1.0 range using min-max normalization.
fn normalize_scores(results: &[SearchResult]) -> Vec<f32> {
    if results.is_empty() {
        return Vec::new();
    }
    if results.len() == 1 {
        return vec![1.0];
    }

    let min = results.iter().map(|r| r.score).fold(f32::MAX, f32::min);
    let max = results.iter().map(|r| r.score).fold(f32::MIN, f32::max);

    if (max - min).abs() < f32::EPSILON {
        return vec![1.0; results.len()];
    }

    results
        .iter()
        .map(|r| (r.score - min) / (max - min))
        .collect()
}

/// Merge vector and FTS results with weighted scoring, dedup by note_path.
fn merge_results(
    vector: &[SearchResult],
    fts: &[SearchResult],
    vector_weight: f32,
    fts_weight: f32,
) -> Vec<SearchResult> {
    let vector_norm = normalize_scores(vector);
    let fts_norm = normalize_scores(fts);

    // For vector search, lower distance = better. Invert so higher = better.
    let vector_scores: HashMap<String, (f32, &SearchResult)> = vector
        .iter()
        .zip(vector_norm.iter())
        .map(|(r, &norm)| {
            let inverted = 1.0 - norm; // lower distance = higher score
            (r.note_path.clone(), (inverted * vector_weight, r))
        })
        .collect();

    let fts_scores: HashMap<String, (f32, &SearchResult)> = fts
        .iter()
        .zip(fts_norm.iter())
        .map(|(r, &norm)| (r.note_path.clone(), (norm * fts_weight, r)))
        .collect();

    // Combine scores
    let mut combined: HashMap<String, (f32, SearchResult)> = HashMap::new();

    for (path, (score, result)) in &vector_scores {
        combined.insert(
            path.clone(),
            (
                *score,
                SearchResult {
                    search_type: SearchType::Hybrid,
                    ..(*result).clone()
                },
            ),
        );
    }

    for (path, (score, result)) in &fts_scores {
        match combined.get_mut(path) {
            Some((existing_score, existing_result)) => {
                *existing_score += score;
                existing_result.search_type = SearchType::Hybrid;
            }
            None => {
                combined.insert(
                    path.clone(),
                    (
                        *score,
                        SearchResult {
                            search_type: SearchType::Hybrid,
                            ..(*result).clone()
                        },
                    ),
                );
            }
        }
    }

    let mut sorted: Vec<(f32, SearchResult)> = combined.into_values().collect();
    sorted.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    sorted
        .into_iter()
        .map(|(score, mut result)| {
            result.score = score;
            result
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::SearchHit;
    use crate::traits::mocks::{MockDocumentStore, MockEmbedder, MockVectorStore};
    use crate::types::{Document, FtsHit};

    fn make_search_engine_with_embedder(
        embedder: Arc<dyn Embedder>,
        vector_results: Vec<SearchHit>,
        fts_results: Vec<FtsHit>,
    ) -> SearchEngine {
        SearchEngine::new(
            embedder,
            Arc::new(MockVectorStore::with_search_results(vector_results)),
            Arc::new(MockDocumentStore::with_fts_results(fts_results)),
        )
    }

    fn make_search_engine(
        vector_results: Vec<SearchHit>,
        fts_results: Vec<FtsHit>,
    ) -> SearchEngine {
        SearchEngine::new(
            Arc::new(MockEmbedder::new(4)),
            Arc::new(MockVectorStore::with_search_results(vector_results)),
            Arc::new(MockDocumentStore::with_fts_results(fts_results)),
        )
    }

    fn make_search_engine_with_docs(docs: Vec<Document>) -> SearchEngine {
        let doc_store = Arc::new(MockDocumentStore::new());
        doc_store.documents.lock().unwrap().extend(docs);

        SearchEngine::new(
            Arc::new(MockEmbedder::new(4)),
            Arc::new(MockVectorStore::new()),
            doc_store,
        )
    }

    fn sample_doc(path: &str, title: &str) -> Document {
        Document {
            source_file: path.to_string(),
            full_text: format!("content for {title}"),
            title: title.to_string(),
            tags: vec![],
            content_hash: "hash".to_string(),
            last_modified: 0.0,
            outgoing_links: vec![],
            backlinks: vec![],
        }
    }

    #[tokio::test]
    async fn vector_search_maps_results() {
        let engine = make_search_engine(
            vec![SearchHit {
                note_path: "a.md".into(),
                breadcrumb: "a.md > Title".into(),
                content: "Hello".into(),
                raw_content: "Hello".into(),
                links: vec![],
                distance: 0.1,
                chunk_index: 0,
            }],
            vec![],
        );
        let results = engine.vector_search("hello", 5).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].note_path, "a.md");
        assert!(matches!(results[0].search_type, SearchType::Vector));
    }

    #[tokio::test]
    async fn fts_search_maps_results() {
        let engine = make_search_engine(
            vec![],
            vec![FtsHit {
                source_file: "b.md".into(),
                title: "B Title".into(),
                content_snippet: "Some text".into(),
                score: 0.8,
            }],
        );
        let results = engine.fts_search("text", 5).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].note_path, "b.md");
        assert!(matches!(results[0].search_type, SearchType::Fts));
    }

    #[tokio::test]
    async fn hybrid_returns_fts_only_when_no_vector() {
        let engine = make_search_engine(
            vec![],
            vec![FtsHit {
                source_file: "b.md".into(),
                title: "B".into(),
                content_snippet: "text".into(),
                score: 0.5,
            }],
        );
        let results = engine.hybrid_search("query", 5).await.unwrap();
        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn hybrid_returns_vector_only_when_no_fts() {
        let engine = make_search_engine(
            vec![SearchHit {
                note_path: "a.md".into(),
                breadcrumb: "a.md".into(),
                content: "A".into(),
                raw_content: "A".into(),
                links: vec![],
                distance: 0.1,
                chunk_index: 0,
            }],
            vec![],
        );
        let results = engine.hybrid_search("query", 5).await.unwrap();
        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn hybrid_merges_and_deduplicates() {
        let engine = make_search_engine(
            vec![
                SearchHit {
                    note_path: "a.md".into(),
                    breadcrumb: "a.md".into(),
                    content: "A vector".into(),
                    raw_content: "A vector".into(),
                    links: vec![],
                    distance: 0.1,
                    chunk_index: 0,
                },
                SearchHit {
                    note_path: "b.md".into(),
                    breadcrumb: "b.md".into(),
                    content: "B vector".into(),
                    raw_content: "B vector".into(),
                    links: vec![],
                    distance: 0.5,
                    chunk_index: 0,
                },
            ],
            vec![
                FtsHit {
                    source_file: "a.md".into(),
                    title: "A".into(),
                    content_snippet: "A fts".into(),
                    score: 0.9,
                },
                FtsHit {
                    source_file: "c.md".into(),
                    title: "C".into(),
                    content_snippet: "C fts".into(),
                    score: 0.3,
                },
            ],
        );

        let results = engine.hybrid_search("query", 10).await.unwrap();
        // a.md appears in both → should be deduped, with combined score
        let paths: Vec<&str> = results.iter().map(|r| r.note_path.as_str()).collect();
        assert!(paths.contains(&"a.md"));
        assert!(paths.contains(&"b.md"));
        assert!(paths.contains(&"c.md"));

        // a.md should score highest (appears in both)
        assert_eq!(results[0].note_path, "a.md");
    }

    #[tokio::test]
    async fn examine_many_returns_partial_success_in_order() {
        let engine = make_search_engine_with_docs(vec![sample_doc("journal/demo.md", "Demo")]);
        let results = engine
            .examine_many(&["demo".to_string(), "missing.md".to_string()])
            .await
            .unwrap();

        assert_eq!(results.len(), 2);
        match &results[0] {
            ExamineManyResult::Success {
                requested_path,
                document,
            } => {
                assert_eq!(requested_path, "demo");
                assert_eq!(document.source_file, "journal/demo.md");
            }
            ExamineManyResult::Error { .. } => panic!("expected success"),
        }
        match &results[1] {
            ExamineManyResult::Error {
                requested_path,
                error,
            } => {
                assert_eq!(requested_path, "missing.md");
                assert!(error.contains("Document not found"));
            }
            ExamineManyResult::Success { .. } => panic!("expected error"),
        }
    }

    #[tokio::test]
    async fn examine_many_reports_ambiguous_path() {
        let engine = make_search_engine_with_docs(vec![
            sample_doc("journal/note.md", "Journal"),
            sample_doc("projects/note.md", "Projects"),
        ]);
        let results = engine.examine_many(&["note.md".to_string()]).await.unwrap();

        assert_eq!(results.len(), 1);
        match &results[0] {
            ExamineManyResult::Error { error, .. } => {
                assert!(error.contains("Multiple matches"));
            }
            ExamineManyResult::Success { .. } => panic!("expected ambiguous error"),
        }
    }

    #[tokio::test]
    async fn examine_many_matches_case_insensitive_query() {
        let engine = make_search_engine_with_docs(vec![sample_doc("journal/demo.md", "Demo")]);
        let results = engine.examine_many(&["DEMO".to_string()]).await.unwrap();

        assert_eq!(results.len(), 1);
        match &results[0] {
            ExamineManyResult::Success { document, .. } => {
                assert_eq!(document.source_file, "journal/demo.md");
            }
            ExamineManyResult::Error { .. } => panic!("expected success"),
        }
    }

    #[tokio::test]
    async fn examine_many_matches_windows_style_path() {
        let engine = make_search_engine_with_docs(vec![sample_doc("journal/2026/demo.md", "Demo")]);
        let results = engine
            .examine_many(&["journal\\2026\\demo".to_string()])
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
        match &results[0] {
            ExamineManyResult::Success { document, .. } => {
                assert_eq!(document.source_file, "journal/2026/demo.md");
            }
            ExamineManyResult::Error { .. } => panic!("expected success"),
        }
    }

    #[test]
    fn normalize_scores_handles_single() {
        let results = vec![SearchResult {
            note_path: "a.md".into(),
            breadcrumb: "a.md".into(),
            content: "A".into(),
            raw_content: "A".into(),
            links: vec![],
            score: 0.5,
            search_type: SearchType::Vector,
            chunk_index: 0,
        }];
        let normalized = normalize_scores(&results);
        assert_eq!(normalized, vec![1.0]);
    }

    #[test]
    fn normalize_scores_handles_equal() {
        let results = vec![
            SearchResult {
                note_path: "a.md".into(),
                breadcrumb: "a.md".into(),
                content: "A".into(),
                raw_content: "A".into(),
                links: vec![],
                score: 0.5,
                search_type: SearchType::Vector,
                chunk_index: 0,
            },
            SearchResult {
                note_path: "b.md".into(),
                breadcrumb: "b.md".into(),
                content: "B".into(),
                raw_content: "B".into(),
                links: vec![],
                score: 0.5,
                search_type: SearchType::Vector,
                chunk_index: 1,
            },
        ];
        let normalized = normalize_scores(&results);
        assert_eq!(normalized, vec![1.0, 1.0]);
    }

    #[tokio::test]
    async fn vector_search_prepends_query_prefix() {
        let embedder = Arc::new(MockEmbedder::new(4));
        let embedder_ref = embedder.clone();
        let engine = make_search_engine_with_embedder(embedder, vec![], vec![])
            .with_query_prefix("search_query: ".into());

        let _ = engine.vector_search("hello", 5).await.unwrap();

        let calls = embedder_ref.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0][0], "search_query: hello");
    }

    #[tokio::test]
    async fn vector_search_no_prefix_by_default() {
        let embedder = Arc::new(MockEmbedder::new(4));
        let embedder_ref = embedder.clone();
        let engine = make_search_engine_with_embedder(embedder, vec![], vec![]);

        let _ = engine.vector_search("hello", 5).await.unwrap();

        let calls = embedder_ref.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0][0], "hello");
    }
}
