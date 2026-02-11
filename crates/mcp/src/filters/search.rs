use crate::filters::tags_match;
use kajet_core::path_utils::{normalize_folder_prefix, path_matches_folder_prefix};
use kajet_core::search::SearchResult;
use kajet_core::types::Document;
use std::collections::HashMap;

/// Filter search results by folder, date range, and tags.
///
/// Builds a doc lookup from `docs`, then retains only results whose note_path
/// exists in the doc map and passes all active filters. Truncates to `limit`.
pub fn filter_search_results(
    results: &mut Vec<SearchResult>,
    docs: &[Document],
    folder: Option<&str>,
    from_ts: Option<f64>,
    to_ts: Option<f64>,
    tags: Option<&[String]>,
    limit: usize,
) {
    let doc_map: HashMap<&str, &Document> =
        docs.iter().map(|d| (d.source_file.as_str(), d)).collect();
    let folder_prefix = folder.map(normalize_folder_prefix);

    results.retain(|result| {
        if !path_matches_folder_prefix(&result.note_path, folder_prefix.as_deref(), true) {
            return false;
        }

        let Some(doc) = doc_map.get(result.note_path.as_str()) else {
            return false;
        };

        if let Some(from) = from_ts
            && doc.last_modified < from
        {
            return false;
        }

        if let Some(to) = to_ts
            && doc.last_modified > to
        {
            return false;
        }

        if let Some(required_tags) = tags
            && !tags_match(&doc.tags, required_tags)
        {
            return false;
        }

        true
    });

    results.truncate(limit);
}
