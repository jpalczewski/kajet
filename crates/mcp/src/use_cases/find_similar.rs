use crate::domain::find_similar_input::{
    FindSimilarInput, SimilarAggregation, SimilarLinkExclusion, SimilarSort,
};
use crate::filters::DocumentFilters;
use crate::format::format_find_similar;
use crate::ports::FindSimilarPorts;
use kajet_core::search::resolve_examine_document;
use kajet_core::traits::{MultiSearchHit, StoredChunk};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

const FIND_SIMILAR_FETCH_MULTIPLIER: usize = 20;
const FIND_SIMILAR_MIN_FETCH_PER_QUERY: usize = 200;
const FIND_SIMILAR_MAX_FETCH_PER_QUERY: usize = 20_000;

#[derive(Debug, Clone)]
pub(crate) struct FindSimilarMatchView {
    pub note_path: String,
    pub similarity: f32,
    pub source_chunk_index: u32,
    pub source_chunk_total: usize,
    pub matched_source_chunks: usize,
    pub similarity_min: f32,
    pub similarity_max: f32,
    pub similarity_avg: f32,
    pub source_section: Option<String>,
    pub target_section: Option<String>,
    pub last_modified: f64,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct FindSimilarView {
    pub source_path: String,
    pub threshold: f32,
    pub aggregation: SimilarAggregation,
    pub sort: SimilarSort,
    pub exclude_linked: SimilarLinkExclusion,
    pub limit: usize,
    pub total_found: usize,
    pub excluded_by_link_count: usize,
    pub source_chunk_total: usize,
    pub fetch_limit_per_query: usize,
    pub fetch_truncated: bool,
    pub results: Vec<FindSimilarMatchView>,
    pub source_chunks_missing: bool,
}

pub(crate) struct FindSimilarOutput {
    pub summary: String,
}

#[derive(Debug, Clone)]
struct PerQueryBest {
    similarity: f32,
    target_section: Option<String>,
}

pub(crate) async fn execute_find_similar(
    ports: &impl FindSimilarPorts,
    input: FindSimilarInput,
) -> anyhow::Result<FindSimilarOutput> {
    let all_docs = ports.get_all_documents().await?;
    let source_doc = resolve_examine_document(&all_docs, &input.path)?;
    let source_path = source_doc.source_file.clone();

    let source_chunks = ports.get_chunks_by_path(&source_path).await?;
    if source_chunks.is_empty() {
        let view = FindSimilarView {
            source_path,
            threshold: input.threshold,
            aggregation: input.aggregation,
            sort: input.sort,
            exclude_linked: input.exclude_linked,
            limit: input.limit,
            total_found: 0,
            excluded_by_link_count: 0,
            source_chunk_total: 0,
            fetch_limit_per_query: 0,
            fetch_truncated: false,
            results: Vec::new(),
            source_chunks_missing: true,
        };
        return Ok(FindSimilarOutput {
            summary: format_find_similar(&view),
        });
    }

    let filters = DocumentFilters::new(
        input.folder.as_deref(),
        input.from_ts,
        input.to_ts,
        input.tags.as_deref(),
    );
    let docs_by_path: HashMap<&str, &kajet_core::types::Document> = all_docs
        .iter()
        .map(|doc| (doc.source_file.as_str(), doc))
        .collect();
    let excluded_paths = build_excluded_paths(source_doc, input.exclude_linked);

    let source_vectors: Vec<Vec<f32>> = source_chunks.iter().map(|c| c.vector.clone()).collect();
    let (raw_hits, fetch_limit_per_query, fetch_truncated) =
        fetch_multi_hits_with_adaptive_limit(ports, &source_vectors, all_docs.len(), input.limit)
            .await?;
    let (grouped, excluded_by_link_count) = group_hits_by_note(
        &raw_hits,
        &source_path,
        &excluded_paths,
        &docs_by_path,
        &filters,
    );

    let source_sections = source_sections_map(&source_chunks);
    let source_chunk_total = source_chunks.len();
    let mut results: Vec<FindSimilarMatchView> = grouped
        .into_iter()
        .filter_map(|(note_path, per_query)| {
            let doc = docs_by_path.get(note_path.as_str())?;
            let agg = aggregate_similarity(&per_query, input.aggregation);
            if agg.similarity < input.threshold {
                return None;
            }

            Some(FindSimilarMatchView {
                note_path,
                similarity: agg.similarity,
                source_chunk_index: agg.source_chunk_index,
                source_chunk_total,
                matched_source_chunks: agg.matched_source_chunks,
                similarity_min: agg.similarity_min,
                similarity_max: agg.similarity_max,
                similarity_avg: agg.similarity_avg,
                source_section: source_sections
                    .get(&agg.source_chunk_index)
                    .cloned()
                    .flatten(),
                target_section: agg.target_section,
                last_modified: doc.last_modified,
                tags: doc.tags.clone(),
            })
        })
        .collect();

    sort_results(&mut results, input.sort);

    let total_found = results.len();
    results.truncate(input.limit);

    let view = FindSimilarView {
        source_path,
        threshold: input.threshold,
        aggregation: input.aggregation,
        sort: input.sort,
        exclude_linked: input.exclude_linked,
        limit: input.limit,
        total_found,
        excluded_by_link_count,
        source_chunk_total,
        fetch_limit_per_query,
        fetch_truncated,
        results,
        source_chunks_missing: false,
    };

    Ok(FindSimilarOutput {
        summary: format_find_similar(&view),
    })
}

fn group_hits_by_note(
    hits: &[MultiSearchHit],
    source_path: &str,
    excluded_paths: &HashSet<&str>,
    docs_by_path: &HashMap<&str, &kajet_core::types::Document>,
    filters: &DocumentFilters<'_>,
) -> (HashMap<String, HashMap<u32, PerQueryBest>>, usize) {
    let mut grouped: HashMap<String, HashMap<u32, PerQueryBest>> = HashMap::new();
    let mut excluded_notes = HashSet::new();

    for hit in hits {
        if hit.note_path == source_path {
            continue;
        }
        if excluded_paths.contains(hit.note_path.as_str()) {
            excluded_notes.insert(hit.note_path.clone());
            continue;
        }

        let Some(doc) = docs_by_path.get(hit.note_path.as_str()) else {
            continue;
        };
        if !filters.matches(doc) {
            continue;
        }

        let similarity = distance_to_similarity(hit.distance);
        let per_note = grouped.entry(hit.note_path.clone()).or_default();
        let entry = per_note.entry(hit.query_index).or_insert(PerQueryBest {
            similarity,
            target_section: section_from_breadcrumb(&hit.breadcrumb),
        });
        if similarity > entry.similarity {
            entry.similarity = similarity;
            entry.target_section = section_from_breadcrumb(&hit.breadcrumb);
        }
    }

    (grouped, excluded_notes.len())
}

struct AggregatedScore {
    similarity: f32,
    source_chunk_index: u32,
    target_section: Option<String>,
    matched_source_chunks: usize,
    similarity_min: f32,
    similarity_max: f32,
    similarity_avg: f32,
}

fn aggregate_similarity(
    per_query: &HashMap<u32, PerQueryBest>,
    mode: SimilarAggregation,
) -> AggregatedScore {
    let mut best_idx = 0u32;
    let mut best_sim = f32::MIN;
    let mut best_target = None;
    let mut min_sim = f32::MAX;
    let mut max_sim = f32::MIN;
    let mut sum = 0.0_f32;
    let mut count = 0usize;

    for (query_index, best) in per_query {
        sum += best.similarity;
        count += 1;
        min_sim = min_sim.min(best.similarity);
        max_sim = max_sim.max(best.similarity);
        if best.similarity > best_sim {
            best_sim = best.similarity;
            best_idx = *query_index;
            best_target = best.target_section.clone();
        }
    }

    if count == 0 {
        return AggregatedScore {
            similarity: 0.0,
            source_chunk_index: 0,
            target_section: None,
            matched_source_chunks: 0,
            similarity_min: 0.0,
            similarity_max: 0.0,
            similarity_avg: 0.0,
        };
    }

    let similarity_avg = sum / count as f32;
    let similarity = match mode {
        SimilarAggregation::Max => best_sim,
        SimilarAggregation::Avg => similarity_avg,
    };

    AggregatedScore {
        similarity,
        source_chunk_index: best_idx,
        target_section: best_target,
        matched_source_chunks: count,
        similarity_min: min_sim,
        similarity_max: max_sim,
        similarity_avg,
    }
}

fn distance_to_similarity(distance: f32) -> f32 {
    let d = distance.max(0.0);
    1.0 / (1.0 + d)
}

fn source_sections_map(source_chunks: &[StoredChunk]) -> HashMap<u32, Option<String>> {
    source_chunks
        .iter()
        .map(|chunk| {
            (
                chunk.chunk_index,
                section_from_breadcrumb(&chunk.breadcrumb),
            )
        })
        .collect()
}

fn section_from_breadcrumb(breadcrumb: &str) -> Option<String> {
    let last = breadcrumb.rsplit(" > ").next()?.trim();
    if last.is_empty() {
        return None;
    }
    Some(last.to_string())
}

fn build_excluded_paths(
    source_doc: &kajet_core::types::Document,
    mode: SimilarLinkExclusion,
) -> HashSet<&str> {
    match mode {
        SimilarLinkExclusion::None => HashSet::new(),
        SimilarLinkExclusion::Outgoing => source_doc
            .outgoing_links
            .iter()
            .map(|path| path.as_str())
            .collect(),
        SimilarLinkExclusion::Both => source_doc
            .outgoing_links
            .iter()
            .chain(source_doc.backlinks.iter())
            .map(|path| path.as_str())
            .collect(),
    }
}

fn sort_results(results: &mut [FindSimilarMatchView], sort: SimilarSort) {
    match sort {
        SimilarSort::Similarity => results.sort_by(|a, b| {
            b.similarity
                .partial_cmp(&a.similarity)
                .unwrap_or(Ordering::Equal)
                .then_with(|| a.note_path.cmp(&b.note_path))
        }),
        SimilarSort::Recent => results.sort_by(|a, b| {
            b.last_modified
                .partial_cmp(&a.last_modified)
                .unwrap_or(Ordering::Equal)
                .then_with(|| {
                    b.similarity
                        .partial_cmp(&a.similarity)
                        .unwrap_or(Ordering::Equal)
                })
                .then_with(|| a.note_path.cmp(&b.note_path))
        }),
        SimilarSort::Path => {
            results.sort_by(|a, b| a.note_path.cmp(&b.note_path));
        }
    }
}

async fn fetch_multi_hits_with_adaptive_limit(
    ports: &impl FindSimilarPorts,
    source_vectors: &[Vec<f32>],
    doc_count: usize,
    requested_limit: usize,
) -> anyhow::Result<(Vec<MultiSearchHit>, usize, bool)> {
    if source_vectors.is_empty() {
        return Ok((Vec::new(), 0, false));
    }

    let initial = requested_limit
        .saturating_mul(FIND_SIMILAR_FETCH_MULTIPLIER)
        .max(FIND_SIMILAR_MIN_FETCH_PER_QUERY);
    let max_fetch = doc_count
        .saturating_mul(16)
        .max(initial)
        .min(FIND_SIMILAR_MAX_FETCH_PER_QUERY);

    let mut fetch_limit = initial.min(max_fetch);
    loop {
        let hits = ports.search_multi(source_vectors, fetch_limit).await?;
        let counts = query_hit_counts(&hits, source_vectors.len());
        let reached_cap = counts.iter().any(|&count| count >= fetch_limit);
        if !reached_cap || fetch_limit >= max_fetch {
            return Ok((hits, fetch_limit, reached_cap && fetch_limit >= max_fetch));
        }

        fetch_limit = fetch_limit.saturating_mul(2).min(max_fetch);
    }
}

fn query_hit_counts(hits: &[MultiSearchHit], query_count: usize) -> Vec<usize> {
    let mut counts = vec![0usize; query_count];
    for hit in hits {
        let idx = hit.query_index as usize;
        if let Some(value) = counts.get_mut(idx) {
            *value += 1;
        }
    }
    counts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::FindSimilarPorts;
    use kajet_core::traits::{MultiSearchHit, StoredChunk};
    use kajet_core::types::Document;

    #[derive(Clone)]
    struct FakeFindSimilarPorts {
        docs: Vec<Document>,
        chunks: HashMap<String, Vec<StoredChunk>>,
        hits: Vec<MultiSearchHit>,
    }

    impl FindSimilarPorts for FakeFindSimilarPorts {
        async fn get_all_documents(&self) -> anyhow::Result<Vec<Document>> {
            Ok(self.docs.clone())
        }

        async fn get_chunks_by_path(&self, note_path: &str) -> anyhow::Result<Vec<StoredChunk>> {
            Ok(self.chunks.get(note_path).cloned().unwrap_or_default())
        }

        async fn search_multi(
            &self,
            _vectors: &[Vec<f32>],
            _limit: usize,
        ) -> anyhow::Result<Vec<MultiSearchHit>> {
            Ok(self.hits.clone())
        }
    }

    fn doc(path: &str, tags: &[&str]) -> Document {
        Document {
            source_file: path.to_string(),
            full_text: String::new(),
            title: path.to_string(),
            tags: tags.iter().map(|v| v.to_string()).collect(),
            content_hash: "h".to_string(),
            last_modified: 100.0,
            outgoing_links: vec![],
            backlinks: vec![],
        }
    }

    #[tokio::test]
    async fn execute_find_similar_excludes_source_and_applies_threshold() {
        let ports = FakeFindSimilarPorts {
            docs: vec![doc("a.md", &[]), doc("b.md", &["x"])],
            chunks: HashMap::from([(
                "a.md".to_string(),
                vec![StoredChunk {
                    note_path: "a.md".to_string(),
                    breadcrumb: "a.md > Intro".to_string(),
                    content: String::new(),
                    raw_content: String::new(),
                    vector: vec![0.1, 0.2],
                    chunk_index: 0,
                    content_hash: "h".to_string(),
                    links: vec![],
                }],
            )]),
            hits: vec![
                MultiSearchHit {
                    query_index: 0,
                    note_path: "a.md".to_string(),
                    breadcrumb: "a.md > Intro".to_string(),
                    content: String::new(),
                    raw_content: String::new(),
                    links: vec![],
                    distance: 0.01,
                    chunk_index: 0,
                },
                MultiSearchHit {
                    query_index: 0,
                    note_path: "b.md".to_string(),
                    breadcrumb: "b.md > Target".to_string(),
                    content: String::new(),
                    raw_content: String::new(),
                    links: vec![],
                    distance: 0.2,
                    chunk_index: 1,
                },
            ],
        };

        let out = execute_find_similar(
            &ports,
            FindSimilarInput {
                path: "a.md".to_string(),
                limit: 10,
                threshold: 0.8,
                aggregation: SimilarAggregation::Max,
                sort: SimilarSort::Similarity,
                exclude_linked: SimilarLinkExclusion::None,
                from_ts: None,
                to_ts: None,
                folder: None,
                tags: None,
            },
        )
        .await
        .expect("ok");

        assert!(out.summary.contains("b.md"));
        assert!(!out.summary.contains("a.md (similarity"));
    }

    #[tokio::test]
    async fn execute_find_similar_supports_fuzzy_source_path() {
        let ports = FakeFindSimilarPorts {
            docs: vec![doc("journal/6-letni ja.md", &[]), doc("other.md", &[])],
            chunks: HashMap::from([(
                "journal/6-letni ja.md".to_string(),
                vec![StoredChunk {
                    note_path: "journal/6-letni ja.md".to_string(),
                    breadcrumb: "journal/6-letni ja.md > Intro".to_string(),
                    content: String::new(),
                    raw_content: String::new(),
                    vector: vec![0.1, 0.2],
                    chunk_index: 0,
                    content_hash: "h".to_string(),
                    links: vec![],
                }],
            )]),
            hits: vec![MultiSearchHit {
                query_index: 0,
                note_path: "other.md".to_string(),
                breadcrumb: "other.md > Match".to_string(),
                content: String::new(),
                raw_content: String::new(),
                links: vec![],
                distance: 0.2,
                chunk_index: 0,
            }],
        };

        let out = execute_find_similar(
            &ports,
            FindSimilarInput {
                path: "6-letni ja".to_string(),
                limit: 10,
                threshold: 0.5,
                aggregation: SimilarAggregation::Max,
                sort: SimilarSort::Similarity,
                exclude_linked: SimilarLinkExclusion::None,
                from_ts: None,
                to_ts: None,
                folder: None,
                tags: None,
            },
        )
        .await
        .expect("ok");

        assert!(out.summary.contains("other.md"));
        assert!(out.summary.contains("chunk 0/1"));
    }

    #[tokio::test]
    async fn execute_find_similar_exclude_linked_outgoing_filters_known_connections() {
        let mut source = doc("source.md", &[]);
        source.outgoing_links = vec!["linked.md".to_string()];

        let ports = FakeFindSimilarPorts {
            docs: vec![source, doc("linked.md", &[]), doc("new.md", &[])],
            chunks: HashMap::from([(
                "source.md".to_string(),
                vec![StoredChunk {
                    note_path: "source.md".to_string(),
                    breadcrumb: "source.md > Intro".to_string(),
                    content: String::new(),
                    raw_content: String::new(),
                    vector: vec![0.1, 0.2],
                    chunk_index: 0,
                    content_hash: "h".to_string(),
                    links: vec![],
                }],
            )]),
            hits: vec![
                MultiSearchHit {
                    query_index: 0,
                    note_path: "linked.md".to_string(),
                    breadcrumb: "linked.md > Match".to_string(),
                    content: String::new(),
                    raw_content: String::new(),
                    links: vec![],
                    distance: 0.2,
                    chunk_index: 0,
                },
                MultiSearchHit {
                    query_index: 0,
                    note_path: "new.md".to_string(),
                    breadcrumb: "new.md > Match".to_string(),
                    content: String::new(),
                    raw_content: String::new(),
                    links: vec![],
                    distance: 0.3,
                    chunk_index: 0,
                },
            ],
        };

        let out = execute_find_similar(
            &ports,
            FindSimilarInput {
                path: "source.md".to_string(),
                limit: 10,
                threshold: 0.0,
                aggregation: SimilarAggregation::Max,
                sort: SimilarSort::Similarity,
                exclude_linked: SimilarLinkExclusion::Outgoing,
                from_ts: None,
                to_ts: None,
                folder: None,
                tags: None,
            },
        )
        .await
        .expect("ok");

        assert!(!out.summary.contains("linked.md"));
        assert!(out.summary.contains("new.md"));
    }
}
