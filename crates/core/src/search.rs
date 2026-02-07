use crate::traits::{DocumentStore, Embedder, VectorStore};
use crate::types::SearchType;
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone, serde::Serialize)]
pub struct SearchResult {
    pub note_path: String,
    pub breadcrumb: String,
    pub content: String,
    pub score: f32,
    pub search_type: SearchType,
}

pub struct SearchEngine {
    embedder: Arc<dyn Embedder>,
    store: Arc<dyn VectorStore>,
    doc_store: Arc<dyn DocumentStore>,
}

impl SearchEngine {
    pub fn new(
        embedder: Arc<dyn Embedder>,
        store: Arc<dyn VectorStore>,
        doc_store: Arc<dyn DocumentStore>,
    ) -> Self {
        Self {
            embedder,
            store,
            doc_store,
        }
    }

    pub fn embedder(&self) -> &Arc<dyn Embedder> {
        &self.embedder
    }

    pub fn store(&self) -> &Arc<dyn VectorStore> {
        &self.store
    }

    pub fn doc_store(&self) -> &Arc<dyn DocumentStore> {
        &self.doc_store
    }

    /// Hybrid search: vector similarity + FTS, merged by weighted scoring.
    pub async fn hybrid_search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let fetch_limit = limit * 2;

        // Run vector and FTS searches in parallel
        let (vector_results, fts_results) = tokio::join!(
            self.vector_search(query, fetch_limit),
            self.fts_search(query, fetch_limit)
        );

        let vector_results = vector_results?;
        let fts_results = fts_results?;

        // If one fails or is empty, just return the other
        if vector_results.is_empty() {
            return Ok(fts_results.into_iter().take(limit).collect());
        }
        if fts_results.is_empty() {
            return Ok(vector_results.into_iter().take(limit).collect());
        }

        // Merge results
        let merged = merge_results(&vector_results, &fts_results, 0.6, 0.4);

        Ok(merged.into_iter().take(limit).collect())
    }

    /// Pure vector similarity search.
    pub async fn vector_search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let query_emb = self.embedder.embed(vec![query])?;
        let hits = self.store.search(&query_emb[0], limit).await?;

        Ok(hits
            .into_iter()
            .map(|hit| SearchResult {
                note_path: hit.note_path,
                breadcrumb: hit.breadcrumb,
                content: hit.content,
                score: hit.distance,
                search_type: SearchType::Vector,
            })
            .collect())
    }

    /// Pure full-text search.
    pub async fn fts_search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let hits = self.doc_store.fts_search(query, limit).await?;

        Ok(hits
            .into_iter()
            .map(|hit| SearchResult {
                note_path: hit.source_file,
                breadcrumb: hit.title,
                content: hit.content_snippet,
                score: hit.score,
                search_type: SearchType::Fts,
            })
            .collect())
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
    use crate::traits::mocks::{MockDocumentStore, MockEmbedder, MockVectorStore};
    use crate::traits::SearchHit;
    use crate::types::FtsHit;

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

    #[tokio::test]
    async fn vector_search_maps_results() {
        let engine = make_search_engine(
            vec![SearchHit {
                note_path: "a.md".into(),
                breadcrumb: "a.md > Title".into(),
                content: "Hello".into(),
                distance: 0.1,
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
                distance: 0.1,
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
                    distance: 0.1,
                },
                SearchHit {
                    note_path: "b.md".into(),
                    breadcrumb: "b.md".into(),
                    content: "B vector".into(),
                    distance: 0.5,
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

    #[test]
    fn normalize_scores_handles_single() {
        let results = vec![SearchResult {
            note_path: "a.md".into(),
            breadcrumb: "a.md".into(),
            content: "A".into(),
            score: 0.5,
            search_type: SearchType::Vector,
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
                score: 0.5,
                search_type: SearchType::Vector,
            },
            SearchResult {
                note_path: "b.md".into(),
                breadcrumb: "b.md".into(),
                content: "B".into(),
                score: 0.5,
                search_type: SearchType::Vector,
            },
        ];
        let normalized = normalize_scores(&results);
        assert_eq!(normalized, vec![1.0, 1.0]);
    }
}
