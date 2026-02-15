use crate::domain::explore_input::{ExploreConnectionsInput, ExploreFilterMode};
use crate::filters::DocumentFilters;
use crate::format::format_explore_connections;
use crate::ports::ExploreConnectionsPorts;
use kajet_core::search::resolve_examine_document;
use kajet_core::traits::StoredChunk;
use kajet_core::types::Document;
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConnectionDirection {
    Backlink,
    Outgoing,
    Both,
}

impl ConnectionDirection {
    fn merge(self, other: ConnectionDirection) -> ConnectionDirection {
        match (self, other) {
            (Self::Both, _) | (_, Self::Both) => Self::Both,
            (Self::Backlink, Self::Outgoing) | (Self::Outgoing, Self::Backlink) => Self::Both,
            (same, _) => same,
        }
    }

    pub(crate) fn arrow(self) -> &'static str {
        match self {
            Self::Backlink => "←",
            Self::Outgoing => "→",
            Self::Both => "↔",
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ExploreNodeView {
    pub path: String,
    pub depth: usize,
    pub degree: usize,
    pub direction: Option<ConnectionDirection>,
    pub context: Option<String>,
    pub cycle: bool,
    pub children: Vec<ExploreNodeView>,
}

#[derive(Debug, Clone)]
pub(crate) struct ExploreConnectionsView {
    pub start_path: String,
    pub depth_limit: usize,
    pub node_limit: usize,
    pub filter_mode: ExploreFilterMode,
    pub dedup: bool,
    pub root: ExploreNodeView,
    pub unique_nodes: usize,
    pub shown_nodes: usize,
    pub max_depth_reached: usize,
    pub truncated: bool,
    pub filtered_results_empty: bool,
}

pub(crate) struct ExploreConnectionsOutput {
    pub summary: String,
}

#[derive(Debug, Clone)]
struct TempNode {
    path: String,
    depth: usize,
    degree: usize,
    direction: Option<ConnectionDirection>,
    context: Option<String>,
    cycle: bool,
    children: Vec<usize>,
}

#[derive(Debug, Clone)]
struct NeighborEdge {
    path: String,
    direction: ConnectionDirection,
    degree: usize,
}

#[derive(Debug, Clone)]
struct QueueItem {
    path: String,
    depth: usize,
    output_idx: Option<usize>,
    anchor_idx: usize,
    ancestors: Vec<String>,
}

pub(crate) async fn execute_explore_connections(
    ports: &impl ExploreConnectionsPorts,
    input: ExploreConnectionsInput,
) -> anyhow::Result<ExploreConnectionsOutput> {
    let view = build_explore_connections_view(ports, &input).await?;
    Ok(ExploreConnectionsOutput {
        summary: format_explore_connections(&view),
    })
}

async fn build_explore_connections_view(
    ports: &impl ExploreConnectionsPorts,
    input: &ExploreConnectionsInput,
) -> anyhow::Result<ExploreConnectionsView> {
    let docs = ports.get_all_documents().await?;
    let start_doc = resolve_examine_document(&docs, &input.path)?;
    let start_path = start_doc.source_file.clone();

    let docs_by_path: HashMap<&str, &Document> = docs
        .iter()
        .map(|doc| (doc.source_file.as_str(), doc))
        .collect();
    let known_paths: HashSet<&str> = docs_by_path.keys().copied().collect();
    let degree_map = build_degree_map(&docs, &known_paths);

    let root_degree = degree_map.get(start_path.as_str()).copied().unwrap_or(0);
    let mut nodes = vec![TempNode {
        path: start_path.clone(),
        depth: 0,
        degree: root_degree,
        direction: None,
        context: None,
        cycle: false,
        children: Vec::new(),
    }];

    let filters = DocumentFilters::new(
        input.folder.as_deref(),
        input.from_ts,
        input.to_ts,
        input.tags.as_deref(),
    );
    let has_filters = input.folder.is_some()
        || input.from_ts.is_some()
        || input.to_ts.is_some()
        || input
            .tags
            .as_ref()
            .map(|tags| !tags.is_empty())
            .unwrap_or(false);
    let root_matches_filter = filters.matches(start_doc);

    if matches!(input.filter_mode, ExploreFilterMode::Traverse)
        && has_filters
        && !root_matches_filter
    {
        let root = materialize_tree(&nodes, 0);
        return Ok(ExploreConnectionsView {
            start_path,
            depth_limit: input.depth,
            node_limit: input.limit,
            filter_mode: input.filter_mode,
            dedup: input.dedup,
            root,
            unique_nodes: 1,
            shown_nodes: 1,
            max_depth_reached: 0,
            truncated: false,
            filtered_results_empty: true,
        });
    }

    let mut shown_nodes = 1usize;
    let mut unique_nodes: HashSet<String> = HashSet::from([start_path.clone()]);
    let mut matched_visible_nodes: HashSet<String> = HashSet::new();
    if root_matches_filter {
        matched_visible_nodes.insert(start_path.clone());
    }
    let mut max_depth_reached = 0usize;
    let mut truncated = false;
    let mut seen_global: HashSet<String> = HashSet::new();
    if input.dedup {
        seen_global.insert(start_path.clone());
    }

    let mut queue = VecDeque::new();
    queue.push_back(QueueItem {
        path: start_path.clone(),
        depth: 0,
        output_idx: Some(0),
        anchor_idx: 0,
        ancestors: vec![start_path.clone()],
    });

    let mut context_cache = EdgeContextCache::new(ports);

    while let Some(item) = queue.pop_front() {
        if item.depth >= input.depth {
            continue;
        }

        let Some(current_doc) = docs_by_path.get(item.path.as_str()) else {
            continue;
        };

        let neighbors = build_neighbors(current_doc, &known_paths, &degree_map);
        for neighbor in neighbors {
            if truncated {
                break;
            }

            let Some(candidate_doc) = docs_by_path.get(neighbor.path.as_str()) else {
                continue;
            };
            let matches_filter = filters.matches(candidate_doc);
            if matches!(input.filter_mode, ExploreFilterMode::Traverse) && !matches_filter {
                continue;
            }

            let cycle_on_path = item.ancestors.iter().any(|path| path == &neighbor.path);
            let revisited = input.dedup && seen_global.contains(&neighbor.path);
            let cycle = cycle_on_path || revisited;

            let should_display = if neighbor.path == start_path {
                let root_cycle_allowed = !has_filters || root_matches_filter;
                item.output_idx.is_some() && root_cycle_allowed
            } else {
                match input.filter_mode {
                    ExploreFilterMode::Display => matches_filter,
                    ExploreFilterMode::Traverse => true,
                }
            };

            let parent_idx = item.output_idx.unwrap_or(item.anchor_idx);
            let mut next_output_idx = None;
            let mut next_anchor_idx = item.anchor_idx;

            if should_display {
                if shown_nodes >= input.limit {
                    truncated = true;
                    break;
                }

                let context = if input.include_context {
                    context_cache
                        .context_for_edge(&item.path, &neighbor.path, neighbor.direction)
                        .await
                } else {
                    None
                };

                let idx = nodes.len();
                nodes.push(TempNode {
                    path: neighbor.path.clone(),
                    depth: item.depth + 1,
                    degree: neighbor.degree,
                    direction: Some(neighbor.direction),
                    context,
                    cycle,
                    children: Vec::new(),
                });
                nodes[parent_idx].children.push(idx);
                shown_nodes += 1;
                unique_nodes.insert(neighbor.path.clone());
                if matches_filter {
                    matched_visible_nodes.insert(neighbor.path.clone());
                }
                max_depth_reached = max_depth_reached.max(item.depth + 1);

                next_output_idx = Some(idx);
                next_anchor_idx = idx;
            } else if !cycle {
                max_depth_reached = max_depth_reached.max(item.depth + 1);
            }

            if cycle {
                continue;
            }

            if input.dedup {
                seen_global.insert(neighbor.path.clone());
            }

            if item.depth + 1 < input.depth {
                let mut next_ancestors = item.ancestors.clone();
                next_ancestors.push(neighbor.path.clone());
                queue.push_back(QueueItem {
                    path: neighbor.path,
                    depth: item.depth + 1,
                    output_idx: next_output_idx,
                    anchor_idx: next_anchor_idx,
                    ancestors: next_ancestors,
                });
            }
        }
    }

    let root = materialize_tree(&nodes, 0);
    Ok(ExploreConnectionsView {
        start_path,
        depth_limit: input.depth,
        node_limit: input.limit,
        filter_mode: input.filter_mode,
        dedup: input.dedup,
        root,
        unique_nodes: unique_nodes.len(),
        shown_nodes,
        max_depth_reached,
        truncated,
        filtered_results_empty: has_filters && matched_visible_nodes.is_empty(),
    })
}

fn materialize_tree(nodes: &[TempNode], idx: usize) -> ExploreNodeView {
    let node = &nodes[idx];
    ExploreNodeView {
        path: node.path.clone(),
        depth: node.depth,
        degree: node.degree,
        direction: node.direction,
        context: node.context.clone(),
        cycle: node.cycle,
        children: node
            .children
            .iter()
            .map(|child_idx| materialize_tree(nodes, *child_idx))
            .collect(),
    }
}

fn build_degree_map(docs: &[Document], known_paths: &HashSet<&str>) -> HashMap<String, usize> {
    docs.iter()
        .map(|doc| {
            let mut neighbors = HashSet::new();
            for target in &doc.outgoing_links {
                if known_paths.contains(target.as_str()) {
                    neighbors.insert(target.as_str());
                }
            }
            for source in &doc.backlinks {
                if known_paths.contains(source.as_str()) {
                    neighbors.insert(source.as_str());
                }
            }
            (doc.source_file.clone(), neighbors.len())
        })
        .collect()
}

fn build_neighbors(
    doc: &Document,
    known_paths: &HashSet<&str>,
    degree_map: &HashMap<String, usize>,
) -> Vec<NeighborEdge> {
    let mut relation_map: HashMap<&str, ConnectionDirection> = HashMap::new();

    for target in &doc.outgoing_links {
        if !known_paths.contains(target.as_str()) {
            continue;
        }
        relation_map
            .entry(target.as_str())
            .and_modify(|existing| *existing = existing.merge(ConnectionDirection::Outgoing))
            .or_insert(ConnectionDirection::Outgoing);
    }

    for source in &doc.backlinks {
        if !known_paths.contains(source.as_str()) {
            continue;
        }
        relation_map
            .entry(source.as_str())
            .and_modify(|existing| *existing = existing.merge(ConnectionDirection::Backlink))
            .or_insert(ConnectionDirection::Backlink);
    }

    let mut out: Vec<NeighborEdge> = relation_map
        .into_iter()
        .map(|(path, direction)| NeighborEdge {
            path: path.to_string(),
            direction,
            degree: degree_map.get(path).copied().unwrap_or(0),
        })
        .collect();

    out.sort_by(|a, b| b.degree.cmp(&a.degree).then_with(|| a.path.cmp(&b.path)));
    out
}

struct EdgeContextCache<'a, P: ExploreConnectionsPorts> {
    ports: &'a P,
    by_source: HashMap<String, HashMap<String, String>>,
}

impl<'a, P: ExploreConnectionsPorts> EdgeContextCache<'a, P> {
    fn new(ports: &'a P) -> Self {
        Self {
            ports,
            by_source: HashMap::new(),
        }
    }

    async fn context_for_edge(
        &mut self,
        from: &str,
        to: &str,
        direction: ConnectionDirection,
    ) -> Option<String> {
        let (source, target) = match direction {
            ConnectionDirection::Outgoing | ConnectionDirection::Both => (from, to),
            ConnectionDirection::Backlink => (to, from),
        };

        self.ensure_loaded(source).await;
        self.by_source
            .get(source)
            .and_then(|m| m.get(target))
            .cloned()
    }

    async fn ensure_loaded(&mut self, source: &str) {
        if self.by_source.contains_key(source) {
            return;
        }

        let chunks = match self.ports.get_chunks_by_path(source).await {
            Ok(chunks) => chunks,
            Err(error) => {
                tracing::warn!(path = source, error = %error, "Failed to load chunks for edge context");
                Vec::new()
            }
        };

        let mut map = HashMap::new();
        for chunk in chunks {
            let Some(section) = section_from_breadcrumb(&chunk) else {
                continue;
            };
            for link in chunk.links {
                if let Some(target) = link.resolved_path {
                    map.entry(target).or_insert_with(|| section.clone());
                }
            }
        }

        self.by_source.insert(source.to_string(), map);
    }
}

fn section_from_breadcrumb(chunk: &StoredChunk) -> Option<String> {
    let last = chunk.breadcrumb.rsplit(" > ").next()?.trim();
    if last.is_empty() || last == chunk.note_path {
        return None;
    }
    Some(last.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::explore_input::ExploreConnectionsInput;
    use crate::domain::explore_input::ExploreFilterMode;
    use crate::ports::ExploreConnectionsPorts;
    use kajet_core::traits::StoredChunk;

    #[derive(Clone)]
    struct FakeExplorePorts {
        docs: Vec<Document>,
        chunks: HashMap<String, Vec<StoredChunk>>,
    }

    impl ExploreConnectionsPorts for FakeExplorePorts {
        async fn get_all_documents(&self) -> anyhow::Result<Vec<Document>> {
            Ok(self.docs.clone())
        }

        async fn get_chunks_by_path(&self, note_path: &str) -> anyhow::Result<Vec<StoredChunk>> {
            Ok(self.chunks.get(note_path).cloned().unwrap_or_default())
        }
    }

    fn doc(path: &str, tags: &[&str], outgoing: &[&str], backlinks: &[&str], ts: f64) -> Document {
        Document {
            source_file: path.to_string(),
            full_text: String::new(),
            title: path.to_string(),
            tags: tags.iter().map(|tag| tag.to_string()).collect(),
            content_hash: "hash".to_string(),
            last_modified: ts,
            outgoing_links: outgoing.iter().map(|p| p.to_string()).collect(),
            backlinks: backlinks.iter().map(|p| p.to_string()).collect(),
        }
    }

    fn input(path: &str) -> ExploreConnectionsInput {
        ExploreConnectionsInput {
            path: path.to_string(),
            depth: 3,
            limit: 50,
            dedup: true,
            include_context: false,
            filter_mode: ExploreFilterMode::Display,
            from_ts: None,
            to_ts: None,
            folder: None,
            tags: None,
        }
    }

    #[tokio::test]
    async fn traversal_orders_neighbors_by_degree() {
        let ports = FakeExplorePorts {
            docs: vec![
                doc("a.md", &[], &["b.md"], &["c.md"], 100.0),
                doc("b.md", &[], &[], &["a.md", "d.md"], 100.0),
                doc("c.md", &[], &["a.md"], &[], 100.0),
                doc("d.md", &[], &["b.md"], &[], 100.0),
            ],
            chunks: HashMap::new(),
        };
        let out = execute_explore_connections(&ports, input("a.md"))
            .await
            .expect("ok");

        let b_pos = out.summary.find("b.md").expect("contains b.md");
        let c_pos = out.summary.find("c.md").expect("contains c.md");
        assert!(b_pos < c_pos, "higher-degree node should be shown first");
    }

    #[tokio::test]
    async fn display_mode_traverses_through_filtered_nodes() {
        let ports = FakeExplorePorts {
            docs: vec![
                doc("a.md", &["keep"], &["b.md"], &[], 100.0),
                doc("b.md", &["skip"], &["c.md"], &["a.md"], 100.0),
                doc("c.md", &["keep"], &[], &["b.md"], 100.0),
            ],
            chunks: HashMap::new(),
        };
        let mut req = input("a.md");
        req.depth = 2;
        req.tags = Some(vec!["keep".to_string()]);
        req.filter_mode = ExploreFilterMode::Display;

        let out = execute_explore_connections(&ports, req).await.expect("ok");

        assert!(out.summary.contains("a.md"));
        assert!(!out.summary.contains("b.md"));
        assert!(out.summary.contains("c.md"));
    }

    #[tokio::test]
    async fn include_context_uses_chunk_breadcrumbs() {
        let ports = FakeExplorePorts {
            docs: vec![
                doc("a.md", &[], &["b.md"], &[], 100.0),
                doc("b.md", &[], &[], &["a.md"], 100.0),
            ],
            chunks: HashMap::from([(
                "a.md".to_string(),
                vec![StoredChunk {
                    note_path: "a.md".to_string(),
                    breadcrumb: "a.md > Core concepts".to_string(),
                    content: String::new(),
                    raw_content: String::new(),
                    vector: vec![0.0, 0.0],
                    chunk_index: 0,
                    content_hash: "hash".to_string(),
                    links: vec![kajet_parser::Link {
                        target: "b".to_string(),
                        alias: None,
                        resolved_path: Some("b.md".to_string()),
                    }],
                }],
            )]),
        };

        let mut req = input("a.md");
        req.depth = 1;
        req.include_context = true;

        let out = execute_explore_connections(&ports, req).await.expect("ok");
        assert!(out.summary.contains("Core concepts"));
    }

    #[tokio::test]
    async fn dedup_false_shows_node_on_multiple_paths() {
        let ports = FakeExplorePorts {
            docs: vec![
                doc("a.md", &[], &["b.md", "c.md"], &[], 100.0),
                doc("b.md", &[], &["d.md"], &["a.md"], 100.0),
                doc("c.md", &[], &["d.md"], &["a.md"], 100.0),
                doc("d.md", &[], &[], &["b.md", "c.md"], 100.0),
            ],
            chunks: HashMap::new(),
        };

        let mut req = input("a.md");
        req.depth = 2;
        req.dedup = false;

        let out = execute_explore_connections(&ports, req).await.expect("ok");
        let d_count = out.summary.match_indices("d.md").count();
        assert!(d_count >= 2, "d.md should appear on both branches");
    }

    #[tokio::test]
    async fn display_mode_with_non_matching_filters_has_no_phantom_root_cycles() {
        let ports = FakeExplorePorts {
            docs: vec![
                doc("a.md", &["one"], &["b.md"], &["b.md"], 100.0),
                doc("b.md", &["two"], &["a.md"], &["a.md"], 100.0),
            ],
            chunks: HashMap::new(),
        };

        let mut req = input("a.md");
        req.depth = 2;
        req.tags = Some(vec!["missing".to_string()]);
        req.filter_mode = ExploreFilterMode::Display;

        let out = execute_explore_connections(&ports, req).await.expect("ok");
        assert!(out.summary.contains("No connections match active filters."));
        assert!(!out.summary.contains("← a.md"));
        assert!(!out.summary.contains("→ a.md"));
        assert!(!out.summary.contains("↔ a.md"));
    }
}
