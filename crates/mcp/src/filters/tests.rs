use super::*;
use kajet_core::path_utils::{normalize_folder_prefix, path_matches_folder_prefix};
use kajet_core::search::SearchResult;
use kajet_core::types::{Document, SearchType};
use proptest::prelude::*;

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

fn noisy_tag(tag: &str, index: usize) -> String {
    let base = tag.strip_prefix('#').unwrap_or(tag);
    let cased = if index % 2 == 0 {
        base.to_uppercase()
    } else {
        base.to_lowercase()
    };

    if index % 3 == 0 {
        format!("#{cased}")
    } else {
        cased
    }
}

proptest! {
    #[test]
    fn tags_match_invariant_under_case_and_hash_noise(
        doc_tags in proptest::collection::vec("[a-zA-Z]{1,12}", 0..20),
        required in proptest::collection::vec("[a-zA-Z]{1,12}", 0..10),
    ) {
        let baseline = tags_match(&doc_tags, &required);

        let noisy_doc: Vec<String> = doc_tags
            .iter()
            .enumerate()
            .map(|(i, tag)| noisy_tag(tag, i))
            .collect();
        let noisy_required: Vec<String> = required
            .iter()
            .enumerate()
            .map(|(i, tag)| noisy_tag(tag, i + 7))
            .collect();

        prop_assert_eq!(baseline, tags_match(&noisy_doc, &noisy_required));
    }

    #[test]
    fn recursive_folder_match_equals_prefix_check(
        folder_segments in proptest::collection::vec("[a-z]{1,8}", 1..5),
        path_segments in proptest::collection::vec("[a-z]{1,8}", 1..8),
    ) {
        let folder = folder_segments.join("/");
        let prefix = normalize_folder_prefix(&folder);
        let path = format!("{}.md", path_segments.join("/"));

        prop_assert_eq!(
            path_matches_folder_prefix(&path, Some(&prefix), true),
            path.starts_with(&prefix)
        );
    }

    #[test]
    fn non_recursive_folder_match_accepts_only_direct_children(
        folder_segments in proptest::collection::vec("[a-z]{1,8}", 1..4),
        file in "[a-z]{1,8}",
        nested in "[a-z]{1,8}",
    ) {
        let folder = folder_segments.join("/");
        let prefix = normalize_folder_prefix(&folder);

        let direct_path = format!("{prefix}{file}.md");
        let nested_path = format!("{prefix}{nested}/{file}.md");

        prop_assert!(path_matches_folder_prefix(&direct_path, Some(&prefix), false));
        prop_assert!(!path_matches_folder_prefix(&nested_path, Some(&prefix), false));
    }
}
