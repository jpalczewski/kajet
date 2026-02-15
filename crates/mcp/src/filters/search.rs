use crate::filters::DocumentFilters;
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
    let filters = DocumentFilters::new(folder, from_ts, to_ts, tags);

    results.retain(|result| {
        let Some(doc) = doc_map.get(result.note_path.as_str()) else {
            return false;
        };

        filters.matches(doc)
    });

    results.truncate(limit);
}
