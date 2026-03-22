use crate::types::Document;
use std::collections::{HashMap, HashSet, VecDeque};

/// Reusable BFS distance on the vault's wikilink graph.
pub trait LinkGraph: Send + Sync {
    /// BFS distance between two documents by note_path.
    /// Returns None if no path exists or distance > max_depth.
    fn distance(&self, doc_a: &str, doc_b: &str, max_depth: u32) -> Option<u32>;

    /// Convenience: are two documents connected within max_depth?
    fn are_connected(&self, doc_a: &str, doc_b: &str, max_depth: u32) -> bool {
        self.distance(doc_a, doc_b, max_depth).is_some()
    }
}

#[derive(Debug)]
pub struct InMemoryLinkGraph {
    adjacency: HashMap<String, HashSet<String>>,
}

impl InMemoryLinkGraph {
    /// Build bidirectional adjacency from document outgoing_links.
    /// Each link A→B adds edges in both directions (A↔B).
    pub fn from_documents(docs: &[Document]) -> Self {
        let mut adjacency: HashMap<String, HashSet<String>> = HashMap::with_capacity(docs.len());

        // Ensure every document has an entry (even if no links)
        for doc in docs {
            adjacency.entry(doc.source_file.clone()).or_default();
        }

        // Add bidirectional edges from outgoing_links
        for doc in docs {
            for target in &doc.outgoing_links {
                adjacency
                    .entry(doc.source_file.clone())
                    .or_default()
                    .insert(target.clone());
                adjacency
                    .entry(target.clone())
                    .or_default()
                    .insert(doc.source_file.clone());
            }
        }

        Self { adjacency }
    }

    /// Number of nodes in the link graph.
    ///
    /// Note: this may exceed `docs.len()` passed to `from_documents` because
    /// link targets that were not in the `docs` slice (e.g. excluded or
    /// unindexed files) are also inserted as adjacency entries.
    pub fn doc_count(&self) -> usize {
        self.adjacency.len()
    }
}

impl LinkGraph for InMemoryLinkGraph {
    fn distance(&self, doc_a: &str, doc_b: &str, max_depth: u32) -> Option<u32> {
        if doc_a == doc_b {
            return Some(0);
        }
        if !self.adjacency.contains_key(doc_a) || !self.adjacency.contains_key(doc_b) {
            return None;
        }

        let mut visited: HashSet<&str> = HashSet::new();
        let mut queue: VecDeque<(&str, u32)> = VecDeque::new();
        visited.insert(doc_a);
        queue.push_back((doc_a, 0));

        while let Some((current, depth)) = queue.pop_front() {
            if depth >= max_depth {
                continue;
            }
            if let Some(neighbors) = self.adjacency.get(current) {
                for neighbor in neighbors {
                    let neighbor_str = neighbor.as_str();
                    if neighbor_str == doc_b {
                        return Some(depth + 1);
                    }
                    if visited.insert(neighbor_str) {
                        queue.push_back((neighbor_str, depth + 1));
                    }
                }
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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

    #[test]
    fn distance_basic_chain() {
        // Revachol RCM network: precinct -> harbor -> whirling-in-rags -> feld building
        let docs = vec![
            doc("rcm/precinct.md", vec!["rcm/harbor.md"]),
            doc("rcm/harbor.md", vec!["martinaise/whirling.md"]),
            doc(
                "martinaise/whirling.md",
                vec!["martinaise/feld-building.md"],
            ),
            doc("martinaise/feld-building.md", vec![]),
            doc("revachol/pawn-shop.md", vec![]),
        ];
        let graph = InMemoryLinkGraph::from_documents(&docs);

        assert_eq!(
            graph.distance("rcm/precinct.md", "rcm/harbor.md", 10),
            Some(1)
        );
        assert_eq!(
            graph.distance("rcm/precinct.md", "martinaise/whirling.md", 10),
            Some(2)
        );
        assert_eq!(
            graph.distance("rcm/precinct.md", "martinaise/feld-building.md", 10),
            Some(3)
        );
        // Bidirectional
        assert_eq!(
            graph.distance("martinaise/feld-building.md", "rcm/precinct.md", 10),
            Some(3)
        );
    }

    #[test]
    fn distance_same_node() {
        let docs = vec![
            doc("rcm/precinct.md", vec!["rcm/harbor.md"]),
            doc("rcm/harbor.md", vec![]),
        ];
        let graph = InMemoryLinkGraph::from_documents(&docs);
        assert_eq!(
            graph.distance("rcm/precinct.md", "rcm/precinct.md", 10),
            Some(0)
        );
    }

    #[test]
    fn distance_max_depth_exceeded() {
        let docs = vec![
            doc("a.md", vec!["b.md"]),
            doc("b.md", vec!["c.md"]),
            doc("c.md", vec!["d.md"]),
            doc("d.md", vec![]),
        ];
        let graph = InMemoryLinkGraph::from_documents(&docs);
        assert_eq!(graph.distance("a.md", "d.md", 2), None);
        assert_eq!(graph.distance("a.md", "d.md", 3), Some(3));
    }

    #[test]
    fn distance_disconnected_components() {
        // Two disconnected islands: RCM (Revachol) and Anglers (elsewhere)
        let docs = vec![
            doc("rcm/precinct.md", vec!["rcm/harbor.md"]),
            doc("rcm/harbor.md", vec![]),
            doc("anglers/fishing-village.md", vec!["anglers/docks.md"]),
            doc("anglers/docks.md", vec![]),
        ];
        let graph = InMemoryLinkGraph::from_documents(&docs);
        assert_eq!(
            graph.distance("rcm/precinct.md", "anglers/fishing-village.md", 100),
            None
        );
        assert!(!graph.are_connected("rcm/precinct.md", "anglers/fishing-village.md", 100));
        assert!(graph.are_connected("rcm/precinct.md", "rcm/harbor.md", 100));
    }

    #[test]
    fn distance_nonexistent_node() {
        let docs = vec![doc("rcm/precinct.md", vec![])];
        let graph = InMemoryLinkGraph::from_documents(&docs);
        assert_eq!(
            graph.distance("rcm/precinct.md", "does-not-exist.md", 10),
            None
        );
    }

    #[test]
    fn distance_max_depth_zero() {
        let docs = vec![
            doc("rcm/precinct.md", vec!["rcm/harbor.md"]),
            doc("rcm/harbor.md", vec![]),
        ];
        let graph = InMemoryLinkGraph::from_documents(&docs);
        // max_depth=0 means "same document only"
        assert_eq!(
            graph.distance("rcm/precinct.md", "rcm/precinct.md", 0),
            Some(0)
        );
        assert_eq!(graph.distance("rcm/precinct.md", "rcm/harbor.md", 0), None);
    }
}
