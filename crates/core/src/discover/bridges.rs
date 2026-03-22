use crate::link_graph::LinkGraph;
use crate::path_utils::{normalize_folder_prefix, path_matches_folder_prefix};
use crate::similarity_graph::SimilarityGraph;
use anyhow::bail;
use serde::Serialize;
use std::collections::HashMap;

pub struct BridgesParams {
    pub limit: usize,
    pub mode: BridgesMode,
    pub min_similarity: f32,
    pub min_graph_distance: u32,
    pub cross_layer_bonus: f32,
    pub folder: Option<String>,
    pub required_tags: Vec<String>,
    pub doc_tags: HashMap<String, Vec<String>>,
}

pub enum BridgesMode {
    Similarity,
    Surprise,
}

#[derive(Debug, Serialize)]
pub struct Bridge {
    pub doc_a: String,
    pub doc_b: String,
    pub chunk_a_breadcrumb: String,
    pub chunk_b_breadcrumb: String,
    pub chunk_a_excerpt: String,
    pub chunk_b_excerpt: String,
    pub similarity: f32,
    pub graph_distance: Option<u32>,
    pub score: f32,
    pub layers: [String; 2],
}

#[derive(Debug, Serialize)]
pub struct BridgesMeta {
    pub mode: String,
    pub total_pairs_scanned: usize,
    pub unique_doc_pairs: usize,
    pub min_similarity_used: f32,
    pub min_graph_distance_used: u32,
}

#[derive(Debug, Serialize)]
pub struct BridgesResult {
    pub bridges: Vec<Bridge>,
    pub meta: BridgesMeta,
}

struct BestChunkPair {
    /// Dense chunk index belonging to key_a (the lexicographically smaller doc).
    chunk_i: u32,
    /// Dense chunk index belonging to key_b.
    chunk_j: u32,
    sim_score: f32,
}

pub fn find_bridges(
    sim: &dyn SimilarityGraph,
    links: &dyn LinkGraph,
    params: &BridgesParams,
) -> anyhow::Result<BridgesResult> {
    if !sim.has_identity() {
        bail!("Similarity graph format is outdated. Run full reindex to rebuild.");
    }
    if sim.is_empty() {
        bail!("Similarity graph is empty.");
    }

    // Normalize folder prefix once.
    let folder_prefix = params.folder.as_deref().map(normalize_folder_prefix);
    let folder_prefix_ref = folder_prefix.as_deref();

    // Step 2-4: Scan chunk pairs, deduplicate to doc pairs.
    let mut dedup: HashMap<(String, String), BestChunkPair> = HashMap::new();
    let mut total_pairs_scanned: usize = 0;

    for i in 0..sim.len() {
        for &(j, sim_score) in sim.neighbors(i) {
            // Process each unordered pair only once.
            if i >= j {
                continue;
            }
            if sim_score < params.min_similarity {
                continue;
            }

            let doc_a = sim.chunk_note_path(i);
            let doc_b = sim.chunk_note_path(j);

            if doc_a == doc_b {
                continue;
            }

            // Both docs must be inside the requested folder.
            if !path_matches_folder_prefix(doc_a, folder_prefix_ref, true) {
                continue;
            }
            if !path_matches_folder_prefix(doc_b, folder_prefix_ref, true) {
                continue;
            }

            total_pairs_scanned += 1;

            // Build a canonical ordered key so (A,B) and (B,A) map to the same entry.
            // chunk_i → lexicographically smaller doc, chunk_j → larger doc.
            let (key_a, key_b, ki, kj) = if doc_a <= doc_b {
                (doc_a.to_string(), doc_b.to_string(), i, j)
            } else {
                (doc_b.to_string(), doc_a.to_string(), j, i)
            };

            let entry = dedup.entry((key_a, key_b)).or_insert(BestChunkPair {
                chunk_i: ki,
                chunk_j: kj,
                sim_score,
            });
            if sim_score > entry.sim_score {
                entry.chunk_i = ki;
                entry.chunk_j = kj;
                entry.sim_score = sim_score;
            }
        }
    }

    let unique_doc_pairs = dedup.len();

    // Step 5-7: Apply link-distance and tags filters, compute scores.
    let mut scored: Vec<(f32, Bridge)> = Vec::new();

    for ((doc_a, doc_b), best) in &dedup {
        // Link distance filter.
        if params.min_graph_distance > 0 {
            // Skip if structurally too close (reachable within min_graph_distance-1 hops).
            if links
                .distance(doc_a, doc_b, params.min_graph_distance - 1)
                .is_some()
            {
                continue;
            }
        }

        // Actual link distance (up to 8 hops); None means disconnected.
        let actual_dist = links.distance(doc_a, doc_b, 8);

        // Tags filter: both docs must have ALL required tags.
        if !params.required_tags.is_empty() {
            let tags_a = params
                .doc_tags
                .get(doc_a.as_str())
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            let tags_b = params
                .doc_tags
                .get(doc_b.as_str())
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            let all_match = params
                .required_tags
                .iter()
                .all(|req| tags_a.iter().any(|t| t == req) && tags_b.iter().any(|t| t == req));
            if !all_match {
                continue;
            }
        }

        let sim_score = best.sim_score;
        let root_a = doc_a.split('/').next().unwrap_or("");
        let root_b = doc_b.split('/').next().unwrap_or("");
        let cross_mult = if root_a != root_b {
            1.0 + params.cross_layer_bonus
        } else {
            1.0_f32
        };
        let eff_dist = actual_dist.unwrap_or(9) as f32;
        let score = match params.mode {
            BridgesMode::Surprise => sim_score * (eff_dist + 1.0).log2() * cross_mult,
            BridgesMode::Similarity => sim_score * cross_mult,
        };

        scored.push((
            score,
            Bridge {
                doc_a: doc_a.clone(),
                doc_b: doc_b.clone(),
                chunk_a_breadcrumb: sim.chunk_breadcrumb(best.chunk_i).to_string(),
                chunk_b_breadcrumb: sim.chunk_breadcrumb(best.chunk_j).to_string(),
                chunk_a_excerpt: sim.chunk_excerpt(best.chunk_i).to_string(),
                chunk_b_excerpt: sim.chunk_excerpt(best.chunk_j).to_string(),
                similarity: sim_score,
                graph_distance: actual_dist,
                score,
                layers: [root_a.to_string(), root_b.to_string()],
            },
        ));
    }

    // Sort descending by score.
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(params.limit);

    let mode_str = match params.mode {
        BridgesMode::Surprise => "surprise",
        BridgesMode::Similarity => "similarity",
    }
    .to_string();

    Ok(BridgesResult {
        bridges: scored.into_iter().map(|(_, b)| b).collect(),
        meta: BridgesMeta {
            mode: mode_str,
            total_pairs_scanned,
            unique_doc_pairs,
            min_similarity_used: params.min_similarity,
            min_graph_distance_used: params.min_graph_distance,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::link_graph::InMemoryLinkGraph;
    use crate::similarity_graph::{ChunkEntry, CsrGraph};
    use crate::types::Document;

    fn doc(path: &str, links: Vec<&str>) -> Document {
        Document {
            source_file: path.to_string(),
            full_text: String::new(),
            title: String::new(),
            tags: vec![],
            content_hash: String::new(),
            last_modified: 0.0,
            outgoing_links: links.into_iter().map(String::from).collect(),
            backlinks: vec![],
        }
    }

    /// Build a graph where chunk pairs are: 0↔1, 2↔3, 4↔5, etc.
    /// Each pair shares the given similarity. Chunks at even indices
    /// belong to `paths[i/2 * 2]`, odd to `paths[i/2 * 2 + 1]`.
    fn make_pair_graph(pairs: &[(&str, &str, f32)]) -> CsrGraph {
        // pairs: (doc_a_path, doc_b_path, similarity)
        let n = pairs.len() * 2; // 2 chunks per pair
        let k = 1_u32; // 1 neighbor per chunk (the partner)

        let mut offsets = Vec::with_capacity(n + 1);
        for i in 0..=n {
            offsets.push(i as u32); // each chunk has exactly 1 neighbor
        }

        let mut adj = Vec::with_capacity(n);
        for (idx, &(_, _, sim)) in pairs.iter().enumerate() {
            let a = (idx * 2) as u32;
            let b = (idx * 2 + 1) as u32;
            adj.push((b, sim)); // chunk a → b
            adj.push((a, sim)); // chunk b → a
        }

        let mut chunk_to_doc = Vec::with_capacity(n);
        for i in 0..pairs.len() {
            chunk_to_doc.push((i * 2) as u16); // chunk a → doc index a
            chunk_to_doc.push((i * 2 + 1) as u16); // chunk b → doc index b
        }

        let mut string_table: Vec<u8> = Vec::new();
        let mut entries: Vec<ChunkEntry> = Vec::new();

        for (idx, &(doc_a, doc_b, _)) in pairs.iter().enumerate() {
            for (doc_path, chunk_idx) in [(doc_a, 0u16), (doc_b, 0u16)] {
                let _ = chunk_idx;
                let breadcrumb = format!("{} > Overview", doc_path);
                let excerpt = format!("Content from {}", doc_path);

                let np_off = string_table.len() as u32;
                string_table.extend_from_slice(doc_path.as_bytes());

                let bc_off = string_table.len() as u32;
                string_table.extend_from_slice(breadcrumb.as_bytes());

                let ct_off = string_table.len() as u32;
                string_table.extend_from_slice(excerpt.as_bytes());

                entries.push(ChunkEntry {
                    note_path_offset: np_off,
                    note_path_len: doc_path.len() as u16,
                    breadcrumb_offset: bc_off,
                    breadcrumb_len: breadcrumb.len() as u16,
                    chunk_index: idx as u16,
                    content_offset: ct_off,
                    content_len: excerpt.len() as u16,
                });
            }
        }

        CsrGraph::new_fixed_k(k, n as u32, 384, offsets, adj, chunk_to_doc)
            .unwrap()
            .with_identity(string_table, entries)
            .unwrap()
    }

    fn default_params() -> BridgesParams {
        BridgesParams {
            limit: 10,
            mode: BridgesMode::Surprise,
            min_similarity: 0.7,
            min_graph_distance: 3,
            cross_layer_bonus: 0.3,
            folder: None,
            required_tags: vec![],
            doc_tags: HashMap::new(),
        }
    }

    #[test]
    fn bridges_guard_no_identity() {
        // A v1-style graph without identity (no chunk_entries)
        let graph = CsrGraph::new_fixed_k(
            1,
            2,
            384,
            vec![0, 1, 2],
            vec![(1_u32, 0.9_f32), (0, 0.9)],
            vec![0_u16, 1],
        )
        .unwrap();

        let docs: Vec<Document> = vec![];
        let links = InMemoryLinkGraph::from_documents(&docs);
        let err = find_bridges(&graph, &links, &default_params()).unwrap_err();
        assert!(
            err.to_string().contains("outdated"),
            "expected 'outdated' in: {err}"
        );
    }

    #[test]
    fn bridges_guard_empty() {
        // CsrGraph with n_chunks=0 and no identity → has_identity()=false → first guard triggers.
        // (The empty guard is only reachable if has_identity() is true for n_chunks=0, which
        // is impossible with CsrGraph since has_identity() = !chunk_entries.is_empty().)
        // So both the no-identity and effectively-empty cases are covered by the same path.
        let graph = CsrGraph::new_fixed_k(0, 0, 384, vec![0], vec![], vec![])
            .unwrap()
            .with_identity(vec![], vec![])
            .unwrap();
        let docs: Vec<Document> = vec![];
        let links = InMemoryLinkGraph::from_documents(&docs);
        let err = find_bridges(&graph, &links, &default_params()).unwrap_err();
        assert!(
            err.to_string().contains("outdated"),
            "expected 'outdated' in: {err}"
        );
    }

    #[test]
    fn bridges_finds_unlinked_similar_pair() {
        // martinaise/suspect.md and rcm/report.md are similar (0.9) but not linked.
        let graph = make_pair_graph(&[("martinaise/suspect.md", "rcm/report.md", 0.9)]);
        let docs = vec![
            doc("martinaise/suspect.md", vec![]),
            doc("rcm/report.md", vec![]),
        ];
        let links = InMemoryLinkGraph::from_documents(&docs);
        let result = find_bridges(&graph, &links, &default_params()).unwrap();
        assert_eq!(result.bridges.len(), 1, "expected 1 bridge");
        assert_eq!(result.meta.total_pairs_scanned, 1);
        let b = &result.bridges[0];
        let pair: (&str, &str) = if b.doc_a < b.doc_b {
            (&b.doc_a, &b.doc_b)
        } else {
            (&b.doc_b, &b.doc_a)
        };
        assert_eq!(pair, ("martinaise/suspect.md", "rcm/report.md"));
        assert!((b.similarity - 0.9).abs() < 1e-5);
        assert_eq!(b.graph_distance, None); // disconnected
    }

    #[test]
    fn bridges_skips_linked_pair() {
        // Same semantic similarity but A is linked to B (distance=1 < min_graph_distance=3).
        let graph = make_pair_graph(&[("martinaise/suspect.md", "rcm/report.md", 0.9)]);
        let docs = vec![
            doc("martinaise/suspect.md", vec!["rcm/report.md"]),
            doc("rcm/report.md", vec![]),
        ];
        let links = InMemoryLinkGraph::from_documents(&docs);
        let result = find_bridges(&graph, &links, &default_params()).unwrap();
        assert_eq!(result.bridges.len(), 0, "linked pair should be excluded");
    }

    #[test]
    fn bridges_surprise_mode_ranks_disconnected_higher() {
        // Two pairs with same similarity. One is disconnected, one is far but connected (dist=5).
        // Disconnected (eff_dist=9) should rank higher in surprise mode.
        let graph = make_pair_graph(&[
            ("revachol/tribunal.md", "anglers/village.md", 0.8), // disconnected
            ("martinaise/whirling.md", "rcm/precinct.md", 0.8),  // far-connected (dist=5)
        ]);

        // Build link graph: whirling → A → B → C → D → precinct (5 hops)
        let docs = vec![
            doc("revachol/tribunal.md", vec![]),
            doc("anglers/village.md", vec![]),
            doc("martinaise/whirling.md", vec!["inter/a.md"]),
            doc("inter/a.md", vec!["inter/b.md"]),
            doc("inter/b.md", vec!["inter/c.md"]),
            doc("inter/c.md", vec!["inter/d.md"]),
            doc("inter/d.md", vec!["rcm/precinct.md"]),
            doc("rcm/precinct.md", vec![]),
        ];
        let links = InMemoryLinkGraph::from_documents(&docs);

        let params = BridgesParams {
            mode: BridgesMode::Surprise,
            min_graph_distance: 3, // connected pair is at dist=5, passes filter
            ..default_params()
        };
        let result = find_bridges(&graph, &links, &params).unwrap();
        assert_eq!(result.bridges.len(), 2);

        // Disconnected pair (eff_dist=9) should outrank connected (eff_dist=5) in Surprise mode.
        let scores: Vec<f32> = result.bridges.iter().map(|b| b.score).collect();
        assert!(
            scores[0] > scores[1],
            "disconnected should score higher in surprise mode: {:?}",
            scores
        );
        // The disconnected pair has graph_distance=None.
        assert_eq!(result.bridges[0].graph_distance, None);
    }

    #[test]
    fn bridges_cross_layer_bonus_applied() {
        // Two pairs, same similarity. One is cross-folder (different root), one is same folder.
        // Cross-folder pair should score higher due to cross_layer_bonus.
        let graph = make_pair_graph(&[
            ("martinaise/suspect.md", "rcm/report.md", 0.8), // different roots
            ("martinaise/harbor.md", "martinaise/whirling.md", 0.8), // same root
        ]);
        let docs = vec![
            doc("martinaise/suspect.md", vec![]),
            doc("rcm/report.md", vec![]),
            doc("martinaise/harbor.md", vec![]),
            doc("martinaise/whirling.md", vec![]),
        ];
        let links = InMemoryLinkGraph::from_documents(&docs);

        let params = BridgesParams {
            mode: BridgesMode::Similarity,
            cross_layer_bonus: 0.3,
            ..default_params()
        };
        let result = find_bridges(&graph, &links, &params).unwrap();
        assert_eq!(result.bridges.len(), 2);

        // Cross-folder pair (martinaise/ vs rcm/) should come first.
        let first = &result.bridges[0];
        assert!(
            first.layers[0] != first.layers[1],
            "first bridge should be cross-folder"
        );
        assert!(
            result.bridges[0].score > result.bridges[1].score,
            "cross-folder score should be higher"
        );
    }

    #[test]
    fn bridges_limit_respected() {
        // 4 unlinked pairs, but limit=2 → only 2 results.
        let graph = make_pair_graph(&[
            ("martinaise/suspect.md", "rcm/report.md", 0.9),
            ("revachol/tribunal.md", "anglers/village.md", 0.85),
            ("rcm/harbor.md", "martinaise/port.md", 0.80),
            ("revachol/pawn.md", "anglers/docks.md", 0.75),
        ]);
        let docs: Vec<Document> = vec![
            doc("martinaise/suspect.md", vec![]),
            doc("rcm/report.md", vec![]),
            doc("revachol/tribunal.md", vec![]),
            doc("anglers/village.md", vec![]),
            doc("rcm/harbor.md", vec![]),
            doc("martinaise/port.md", vec![]),
            doc("revachol/pawn.md", vec![]),
            doc("anglers/docks.md", vec![]),
        ];
        let links = InMemoryLinkGraph::from_documents(&docs);
        let params = BridgesParams {
            limit: 2,
            ..default_params()
        };
        let result = find_bridges(&graph, &links, &params).unwrap();
        assert_eq!(result.bridges.len(), 2, "limit should be respected");
        assert_eq!(
            result.meta.unique_doc_pairs, 4,
            "all 4 unique pairs scanned"
        );
    }

    #[test]
    fn bridges_folder_filter() {
        // Two pairs: one fully in martinaise/, one spanning martinaise/ and rcm/.
        // With folder="martinaise", only the pair fully inside martinaise/ should appear.
        let graph = make_pair_graph(&[
            ("martinaise/harbor.md", "martinaise/whirling.md", 0.9), // both inside
            ("martinaise/suspect.md", "rcm/precinct.md", 0.85),      // one outside
        ]);
        let docs = vec![
            doc("martinaise/harbor.md", vec![]),
            doc("martinaise/whirling.md", vec![]),
            doc("martinaise/suspect.md", vec![]),
            doc("rcm/precinct.md", vec![]),
        ];
        let links = InMemoryLinkGraph::from_documents(&docs);
        let params = BridgesParams {
            folder: Some("martinaise".to_string()),
            ..default_params()
        };
        let result = find_bridges(&graph, &links, &params).unwrap();
        assert_eq!(
            result.bridges.len(),
            1,
            "only fully-inside-folder pair should appear"
        );
        let b = &result.bridges[0];
        assert!(
            b.doc_a.starts_with("martinaise/") && b.doc_b.starts_with("martinaise/"),
            "both docs should be in martinaise/"
        );
    }

    #[test]
    fn bridges_dedup_keeps_best_chunk_pair() {
        // Doc A has 2 chunks (0 and 2), doc B has 2 chunks (1 and 3).
        // Chunk 0↔1: sim=0.75, chunk 2↔3: sim=0.92.
        // After dedup, the pair (doc_a, doc_b) should use the best sim=0.92.
        let n_chunks = 4_u32;
        let k = 1_u32;
        let offsets = vec![0_u32, 1, 2, 3, 4];
        // 0→1 (0.75), 1→0 (0.75), 2→3 (0.92), 3→2 (0.92)
        let adj = vec![
            (1_u32, 0.75_f32),
            (0_u32, 0.75_f32),
            (3_u32, 0.92_f32),
            (2_u32, 0.92_f32),
        ];
        let chunk_to_doc = vec![0_u16, 1, 0, 1]; // chunks 0,2 → doc 0; chunks 1,3 → doc 1

        let chunks_data: &[(&str, &str, &str, u16)] = &[
            (
                "martinaise/suspect.md",
                "martinaise/suspect.md > Intro",
                "First chunk from suspect",
                0,
            ),
            (
                "rcm/report.md",
                "rcm/report.md > Summary",
                "First chunk from report",
                0,
            ),
            (
                "martinaise/suspect.md",
                "martinaise/suspect.md > Evidence",
                "Second chunk from suspect",
                1,
            ),
            (
                "rcm/report.md",
                "rcm/report.md > Details",
                "Second chunk from report",
                1,
            ),
        ];

        let mut string_table: Vec<u8> = Vec::new();
        let mut entries: Vec<ChunkEntry> = Vec::new();
        for &(np, bc, ct, ci) in chunks_data {
            let np_off = string_table.len() as u32;
            string_table.extend_from_slice(np.as_bytes());
            let bc_off = string_table.len() as u32;
            string_table.extend_from_slice(bc.as_bytes());
            let ct_off = string_table.len() as u32;
            string_table.extend_from_slice(ct.as_bytes());
            entries.push(ChunkEntry {
                note_path_offset: np_off,
                note_path_len: np.len() as u16,
                breadcrumb_offset: bc_off,
                breadcrumb_len: bc.len() as u16,
                chunk_index: ci,
                content_offset: ct_off,
                content_len: ct.len() as u16,
            });
        }

        let graph = CsrGraph::new_fixed_k(k, n_chunks, 384, offsets, adj, chunk_to_doc)
            .unwrap()
            .with_identity(string_table, entries)
            .unwrap();

        let docs = vec![
            doc("martinaise/suspect.md", vec![]),
            doc("rcm/report.md", vec![]),
        ];
        let links = InMemoryLinkGraph::from_documents(&docs);

        let result = find_bridges(&graph, &links, &default_params()).unwrap();
        assert_eq!(
            result.bridges.len(),
            1,
            "should produce exactly one doc-pair bridge"
        );
        assert_eq!(result.meta.unique_doc_pairs, 1, "dedup to 1 unique pair");

        let b = &result.bridges[0];
        // Best pair is chunks 2↔3 with sim=0.92.
        assert!(
            (b.similarity - 0.92).abs() < 1e-5,
            "should use best sim: {}",
            b.similarity
        );
        assert!(
            b.chunk_a_breadcrumb.contains("Evidence") || b.chunk_b_breadcrumb.contains("Evidence"),
            "best chunk breadcrumb should reference Evidence section"
        );
    }
}
