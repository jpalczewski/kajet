use crate::filters::TagStats;
use kajet_core::path_utils::{normalize_folder_prefix, path_matches_folder_prefix};
use kajet_core::types::Document;
use std::collections::HashMap;

/// Aggregate tags from documents, optionally filtering by folder.
///
/// Returns sorted by count descending, then tag name ascending.
pub fn aggregate_tags(docs: &[Document], folder: Option<&str>, recursive: bool) -> Vec<TagStats> {
    let folder_prefix = folder.map(normalize_folder_prefix);

    let mut tag_map: HashMap<String, Vec<String>> = HashMap::new();

    for doc in docs {
        if !path_matches_folder_prefix(&doc.source_file, folder_prefix.as_deref(), recursive) {
            continue;
        }

        for tag in &doc.tags {
            tag_map
                .entry(tag.clone())
                .or_default()
                .push(doc.source_file.clone());
        }
    }

    let mut result: Vec<TagStats> = tag_map
        .into_iter()
        .map(|(tag, files)| TagStats { tag, files })
        .collect();

    result.sort_by(|a, b| {
        b.files
            .len()
            .cmp(&a.files.len())
            .then_with(|| a.tag.cmp(&b.tag))
    });

    result
}
