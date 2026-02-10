use anyhow::Result;
use kajet_core::search::SearchResult;
use kajet_core::types::Document;
use std::collections::HashMap;

/// Tag with the list of files that contain it.
pub struct TagStats {
    pub tag: String,
    pub files: Vec<String>,
}

/// Result of applying tag edits to a note.
pub struct TagEditResult {
    pub content: String,
    #[allow(dead_code)]
    pub tags_added: usize,
    #[allow(dead_code)]
    pub tags_removed: usize,
    pub timestamp_updated: bool,
}

/// Check if document tags contain all required tags (case-insensitive, `#` prefix ignored).
pub fn tags_match(doc_tags: &[String], required: &[String]) -> bool {
    let doc_tags_normalized: Vec<String> = doc_tags
        .iter()
        .map(|t| t.strip_prefix('#').unwrap_or(t).to_lowercase())
        .collect();

    required.iter().all(|req_tag| {
        let normalized = req_tag.strip_prefix('#').unwrap_or(req_tag).to_lowercase();
        doc_tags_normalized.contains(&normalized)
    })
}

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

    let folder_prefix = folder.map(|f| {
        if f.ends_with('/') {
            f.to_string()
        } else {
            format!("{f}/")
        }
    });

    results.retain(|r| {
        if let Some(ref prefix) = folder_prefix
            && !r.note_path.starts_with(prefix.as_str())
        {
            return false;
        }

        if let Some(doc) = doc_map.get(r.note_path.as_str()) {
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
        } else {
            return false;
        }

        true
    });

    results.truncate(limit);
}

/// Filter browse-mode documents by tags, then truncate to limit.
pub fn filter_browse_results(docs: &mut Vec<Document>, tags: Option<&[String]>, limit: usize) {
    if let Some(required_tags) = tags {
        docs.retain(|d| tags_match(&d.tags, required_tags));
    }
    docs.truncate(limit);
}

/// Aggregate tags from documents, optionally filtering by folder.
///
/// Returns sorted by count descending, then tag name ascending.
pub fn aggregate_tags(docs: &[Document], folder: Option<&str>, recursive: bool) -> Vec<TagStats> {
    let folder_prefix = folder.map(|f| {
        let f = f.strip_suffix('/').unwrap_or(f);
        format!("{f}/")
    });

    let filtered: Vec<&Document> = docs
        .iter()
        .filter(|d| {
            let Some(ref prefix) = folder_prefix else {
                return true;
            };
            if !d.source_file.starts_with(prefix.as_str()) {
                return false;
            }
            if !recursive {
                let rest = &d.source_file[prefix.len()..];
                return !rest.contains('/');
            }
            true
        })
        .collect();

    let mut tag_map: HashMap<String, Vec<String>> = HashMap::new();
    for doc in &filtered {
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

/// Apply tag operations to note content: remove tags first, then add, then optional timestamp update.
///
/// This function orchestrates the sequence of operations that must be applied in order:
/// 1. Remove tags (if any)
/// 2. Add tags (if any)
/// 3. Update timestamp field (if timestamp_update is Some)
///
/// # Parameters
///
/// - `content`: The markdown content with frontmatter
/// - `add`: Optional list of tags to add
/// - `remove`: Optional list of tags to remove
/// - `timestamp_update`: Optional (field_name, timestamp_value) to update in frontmatter
///
/// # Returns
///
/// `TagEditResult` with the modified content and counts of operations performed.
pub fn apply_tag_edits(
    content: &str,
    add: Option<&[String]>,
    remove: Option<&[String]>,
    timestamp_update: Option<(&str, &str)>,
) -> Result<TagEditResult> {
    let mut modified = content.to_string();
    let tags_removed = remove.map(|r| r.len()).unwrap_or(0);
    let tags_added = add.map(|a| a.len()).unwrap_or(0);

    // Step 1: Remove tags
    if let Some(remove_tags) = remove {
        modified = kajet_parser::remove_tags(&modified, remove_tags)?;
    }

    // Step 2: Add tags
    if let Some(add_tags) = add {
        modified = kajet_parser::add_tags(&modified, add_tags)?;
    }

    // Step 3: Update timestamp field if requested
    let timestamp_updated = if let Some((field, value)) = timestamp_update {
        modified = kajet_parser::update_existing_frontmatter_field(&modified, field, value);
        true
    } else {
        false
    };

    Ok(TagEditResult {
        content: modified,
        tags_added,
        tags_removed,
        timestamp_updated,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use kajet_core::search::SearchResult;
    use kajet_core::types::{Document, SearchType};

    fn make_doc(path: &str, tags: &[&str], last_modified: f64) -> Document {
        Document {
            source_file: path.to_string(),
            full_text: String::new(),
            title: path.to_string(),
            tags: tags.iter().map(|t| t.to_string()).collect(),
            content_hash: String::new(),
            last_modified,
            outgoing_links: vec![],
            backlinks: vec![],
        }
    }

    fn make_result(path: &str) -> SearchResult {
        SearchResult {
            note_path: path.to_string(),
            breadcrumb: path.to_string(),
            content: String::new(),
            raw_content: String::new(),
            links: vec![],
            score: 1.0,
            search_type: SearchType::default(),
        }
    }

    // -----------------------------------------------------------------------
    // tags_match
    // -----------------------------------------------------------------------

    #[test]
    fn tags_match_case_insensitive() {
        assert!(tags_match(
            &["Rust".to_string(), "MCP".to_string()],
            &["rust".to_string()]
        ));
    }

    #[test]
    fn tags_match_ignores_hash_prefix() {
        assert!(tags_match(&["#rust".to_string()], &["rust".to_string()]));
        assert!(tags_match(&["rust".to_string()], &["#rust".to_string()]));
    }

    #[test]
    fn tags_match_requires_all() {
        assert!(!tags_match(
            &["rust".to_string()],
            &["rust".to_string(), "mcp".to_string()]
        ));
    }

    #[test]
    fn tags_match_empty_required_matches_anything() {
        assert!(tags_match(&["rust".to_string()], &[]));
        assert!(tags_match(&[], &[]));
    }

    // -----------------------------------------------------------------------
    // filter_search_results
    // -----------------------------------------------------------------------

    #[test]
    fn filter_by_folder() {
        let docs = vec![
            make_doc("journal/2025/note.md", &[], 100.0),
            make_doc("projects/kajet.md", &[], 100.0),
        ];
        let mut results = vec![
            make_result("journal/2025/note.md"),
            make_result("projects/kajet.md"),
        ];

        filter_search_results(&mut results, &docs, Some("journal"), None, None, None, 10);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].note_path, "journal/2025/note.md");
    }

    #[test]
    fn filter_by_folder_with_trailing_slash() {
        let docs = vec![make_doc("journal/note.md", &[], 100.0)];
        let mut results = vec![make_result("journal/note.md")];

        filter_search_results(&mut results, &docs, Some("journal/"), None, None, None, 10);

        assert_eq!(results.len(), 1);
    }

    #[test]
    fn filter_by_date_range_from_only() {
        let docs = vec![
            make_doc("old.md", &[], 50.0),
            make_doc("new.md", &[], 200.0),
        ];
        let mut results = vec![make_result("old.md"), make_result("new.md")];

        filter_search_results(&mut results, &docs, None, Some(100.0), None, None, 10);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].note_path, "new.md");
    }

    #[test]
    fn filter_by_date_range_to_only() {
        let docs = vec![
            make_doc("old.md", &[], 50.0),
            make_doc("new.md", &[], 200.0),
        ];
        let mut results = vec![make_result("old.md"), make_result("new.md")];

        filter_search_results(&mut results, &docs, None, None, Some(100.0), None, 10);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].note_path, "old.md");
    }

    #[test]
    fn filter_by_date_range_both() {
        let docs = vec![
            make_doc("old.md", &[], 50.0),
            make_doc("mid.md", &[], 150.0),
            make_doc("new.md", &[], 300.0),
        ];
        let mut results = vec![
            make_result("old.md"),
            make_result("mid.md"),
            make_result("new.md"),
        ];

        filter_search_results(
            &mut results,
            &docs,
            None,
            Some(100.0),
            Some(200.0),
            None,
            10,
        );

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].note_path, "mid.md");
    }

    #[test]
    fn filter_by_tags() {
        let docs = vec![
            make_doc("tagged.md", &["rust", "mcp"], 100.0),
            make_doc("untagged.md", &["python"], 100.0),
        ];
        let mut results = vec![make_result("tagged.md"), make_result("untagged.md")];
        let tags = vec!["rust".to_string()];

        filter_search_results(&mut results, &docs, None, None, None, Some(&tags), 10);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].note_path, "tagged.md");
    }

    #[test]
    fn filter_removes_results_not_in_doc_map() {
        let docs = vec![make_doc("known.md", &[], 100.0)];
        let mut results = vec![make_result("known.md"), make_result("unknown.md")];

        filter_search_results(&mut results, &docs, None, None, None, None, 10);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].note_path, "known.md");
    }

    #[test]
    fn filter_applies_limit_after_filtering() {
        let docs = vec![
            make_doc("a.md", &[], 100.0),
            make_doc("b.md", &[], 100.0),
            make_doc("c.md", &[], 100.0),
        ];
        let mut results = vec![
            make_result("a.md"),
            make_result("b.md"),
            make_result("c.md"),
        ];

        filter_search_results(&mut results, &docs, None, None, None, None, 2);

        assert_eq!(results.len(), 2);
    }

    #[test]
    fn filter_combined_folder_tags_date() {
        let docs = vec![
            make_doc("journal/a.md", &["daily"], 100.0),
            make_doc("journal/b.md", &["weekly"], 200.0),
            make_doc("projects/c.md", &["daily"], 100.0),
        ];
        let mut results = vec![
            make_result("journal/a.md"),
            make_result("journal/b.md"),
            make_result("projects/c.md"),
        ];
        let tags = vec!["daily".to_string()];

        filter_search_results(
            &mut results,
            &docs,
            Some("journal"),
            Some(50.0),
            Some(150.0),
            Some(&tags),
            10,
        );

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].note_path, "journal/a.md");
    }

    // -----------------------------------------------------------------------
    // filter_browse_results
    // -----------------------------------------------------------------------

    #[test]
    fn browse_filter_by_tags() {
        let mut docs = vec![
            make_doc("a.md", &["rust"], 100.0),
            make_doc("b.md", &["python"], 100.0),
        ];
        let tags = vec!["rust".to_string()];

        filter_browse_results(&mut docs, Some(&tags), 10);

        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].source_file, "a.md");
    }

    #[test]
    fn browse_filter_limit_applied_after_tags() {
        let mut docs = vec![
            make_doc("a.md", &["rust"], 100.0),
            make_doc("b.md", &["rust"], 100.0),
            make_doc("c.md", &["rust"], 100.0),
        ];
        let tags = vec!["rust".to_string()];

        filter_browse_results(&mut docs, Some(&tags), 2);

        assert_eq!(docs.len(), 2);
    }

    #[test]
    fn browse_filter_no_tags_just_limit() {
        let mut docs = vec![make_doc("a.md", &[], 100.0), make_doc("b.md", &[], 100.0)];

        filter_browse_results(&mut docs, None, 1);

        assert_eq!(docs.len(), 1);
    }

    // -----------------------------------------------------------------------
    // aggregate_tags
    // -----------------------------------------------------------------------

    #[test]
    fn aggregate_tags_basic() {
        let docs = vec![
            make_doc("a.md", &["rust", "mcp"], 100.0),
            make_doc("b.md", &["rust"], 100.0),
        ];

        let result = aggregate_tags(&docs, None, true);

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].tag, "rust");
        assert_eq!(result[0].files.len(), 2);
        assert_eq!(result[1].tag, "mcp");
        assert_eq!(result[1].files.len(), 1);
    }

    #[test]
    fn aggregate_tags_sorted_by_count_then_name() {
        let docs = vec![
            make_doc("a.md", &["beta", "alpha"], 100.0),
            make_doc("b.md", &["beta", "gamma"], 100.0),
            make_doc("c.md", &["alpha", "gamma"], 100.0),
        ];

        let result = aggregate_tags(&docs, None, true);

        // alpha=2, beta=2, gamma=2 → sorted alphabetically
        assert_eq!(result[0].tag, "alpha");
        assert_eq!(result[1].tag, "beta");
        assert_eq!(result[2].tag, "gamma");
    }

    #[test]
    fn aggregate_tags_folder_filter() {
        let docs = vec![
            make_doc("journal/a.md", &["daily"], 100.0),
            make_doc("projects/b.md", &["work"], 100.0),
        ];

        let result = aggregate_tags(&docs, Some("journal"), true);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].tag, "daily");
    }

    #[test]
    fn aggregate_tags_non_recursive() {
        let docs = vec![
            make_doc("journal/a.md", &["daily"], 100.0),
            make_doc("journal/2025/b.md", &["nested"], 100.0),
        ];

        let result = aggregate_tags(&docs, Some("journal"), false);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].tag, "daily");
    }

    #[test]
    fn aggregate_tags_docs_without_tags_ignored() {
        let docs = vec![
            make_doc("a.md", &[], 100.0),
            make_doc("b.md", &["rust"], 100.0),
        ];

        let result = aggregate_tags(&docs, None, true);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].tag, "rust");
    }

    #[test]
    fn aggregate_tags_no_folder_includes_all() {
        let docs = vec![
            make_doc("a.md", &["x"], 100.0),
            make_doc("sub/b.md", &["y"], 100.0),
        ];

        let result = aggregate_tags(&docs, None, true);

        assert_eq!(result.len(), 2);
    }

    // -----------------------------------------------------------------------
    // apply_tag_edits
    // -----------------------------------------------------------------------

    #[test]
    fn apply_tag_edits_add_only() {
        let content = "---\ntitle: Test\ntags: []\n---\nContent";
        let result = apply_tag_edits(content, Some(&["rust".to_string()]), None, None).unwrap();

        assert!(result.content.contains("rust"));
        assert_eq!(result.tags_added, 1);
        assert_eq!(result.tags_removed, 0);
        assert!(!result.timestamp_updated);
    }

    #[test]
    fn apply_tag_edits_remove_only() {
        let content = "---\ntitle: Test\ntags: [rust, mcp]\n---\nContent";
        let result = apply_tag_edits(content, None, Some(&["rust".to_string()]), None).unwrap();

        assert!(!result.content.contains("rust"));
        assert!(result.content.contains("mcp"));
        assert_eq!(result.tags_added, 0);
        assert_eq!(result.tags_removed, 1);
        assert!(!result.timestamp_updated);
    }

    #[test]
    fn apply_tag_edits_remove_then_add() {
        let content = "---\ntitle: Test\ntags: [old]\n---\nContent";
        let result = apply_tag_edits(
            content,
            Some(&["new".to_string()]),
            Some(&["old".to_string()]),
            None,
        )
        .unwrap();

        assert!(!result.content.contains("old"));
        assert!(result.content.contains("new"));
        assert_eq!(result.tags_added, 1);
        assert_eq!(result.tags_removed, 1);
    }

    #[test]
    fn apply_tag_edits_same_tag_in_remove_and_add() {
        // Edge case: user removes and adds the same tag (idempotency test)
        let content = "---\ntitle: Test\ntags: [rust]\n---\nContent";
        let result = apply_tag_edits(
            content,
            Some(&["rust".to_string()]),
            Some(&["rust".to_string()]),
            None,
        )
        .unwrap();

        // After remove, tag is gone. Then add re-adds it.
        assert!(result.content.contains("rust"));
        assert_eq!(result.tags_added, 1);
        assert_eq!(result.tags_removed, 1);
    }

    #[test]
    fn apply_tag_edits_with_timestamp() {
        let content = "---\ntitle: Test\ntags: []\nmodified: 2025-01-01\n---\nContent";
        let result = apply_tag_edits(
            content,
            Some(&["rust".to_string()]),
            None,
            Some(("modified", "2026-02-10")),
        )
        .unwrap();

        assert!(result.content.contains("rust"));
        assert!(result.content.contains("2026-02-10"));
        assert!(!result.content.contains("2025-01-01"));
        assert!(result.timestamp_updated);
    }

    #[test]
    fn apply_tag_edits_no_operations() {
        let content = "---\ntitle: Test\ntags: []\n---\nContent";
        let result = apply_tag_edits(content, None, None, None).unwrap();

        assert_eq!(result.content, content);
        assert_eq!(result.tags_added, 0);
        assert_eq!(result.tags_removed, 0);
        assert!(!result.timestamp_updated);
    }

    #[test]
    fn apply_tag_edits_empty_arrays_count_as_operations() {
        let content = "---\ntitle: Test\ntags: []\n---\nContent";
        let result = apply_tag_edits(content, Some(&[]), Some(&[]), None).unwrap();

        assert_eq!(result.tags_added, 0);
        assert_eq!(result.tags_removed, 0);
    }

    #[test]
    fn apply_tag_edits_multiple_tags() {
        let content = "---\ntitle: Test\ntags: [old1, old2]\n---\nContent";
        let result = apply_tag_edits(
            content,
            Some(&["new1".to_string(), "new2".to_string()]),
            Some(&["old1".to_string(), "old2".to_string()]),
            None,
        )
        .unwrap();

        assert!(!result.content.contains("old1"));
        assert!(!result.content.contains("old2"));
        assert!(result.content.contains("new1"));
        assert!(result.content.contains("new2"));
        assert_eq!(result.tags_added, 2);
        assert_eq!(result.tags_removed, 2);
    }
}
