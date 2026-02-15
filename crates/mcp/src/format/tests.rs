use super::*;
use crate::domain::documents_input::ExamineContentMode;
use crate::domain::tags_input::TagListDetail;
use kajet_core::search::SearchResult;
use kajet_core::text_utils::{slice_by_char_boundary, truncate_with_ellipsis};
use kajet_core::types::Document;
use kajet_core::types::SearchType;
use kajet_parser::Link;
use kajet_writer::{CreateNoteResult, EditNoteResult};
use proptest::prelude::*;

fn setup_locale() {
    rust_i18n::set_locale("en");
}

#[test]
fn format_create_result_basic() {
    setup_locale();
    let result = CreateNoteResult {
        path: "test/note.md".into(),
        absolute_path: "/vault/test/note.md".into(),
        bytes_written: 256,
    };
    let text = format_create_result(&result);
    assert!(text.contains("test/note.md"));
    assert!(text.contains("256"));
}

#[test]
fn format_edit_result_basic() {
    setup_locale();
    let result = EditNoteResult {
        path: "note.md".into(),
        absolute_path: "/vault/note.md".into(),
        mode: "append".into(),
        bytes_written: 128,
        backup_path: None,
    };
    let text = format_edit_result(&result);
    assert!(text.contains("note.md"));
    assert!(text.contains("append"));
    assert!(text.contains("128"));
    assert!(!text.contains("Backup"));
}

#[test]
fn format_edit_result_with_backup() {
    setup_locale();
    let result = EditNoteResult {
        path: "note.md".into(),
        absolute_path: "/vault/note.md".into(),
        mode: "overwrite".into(),
        bytes_written: 512,
        backup_path: Some("/vault/.kajet/backups/20260208T120000_note.md".into()),
    };
    let text = format_edit_result(&result);
    assert!(text.contains("note.md"));
    assert!(text.contains("overwrite"));
    assert!(text.contains("Backup"));
}

#[test]
fn format_results_empty() {
    setup_locale();
    let result = format_results("test query", &[]);
    assert_eq!(result, "No results found for: test query");
}

#[test]
fn format_results_single() {
    setup_locale();
    let results = vec![SearchResult {
        note_path: "note.md".into(),
        breadcrumb: "note.md > Intro".into(),
        content: "Hello world".into(),
        raw_content: "Hello world".into(),
        links: vec![],
        score: 0.1234,
        search_type: kajet_core::types::SearchType::Vector,
        chunk_index: 0,
    }];
    let result = format_results("hello", &results);
    assert!(result.contains("Found 1 results for: \"hello\""));
    assert!(result.contains("Result 1 (score: 0.1234)"));
    assert!(result.contains("Path: note.md"));
    assert!(result.contains("Section: note.md > Intro"));
    assert!(result.contains("Hello world"));
}

#[test]
fn format_results_multiple() {
    setup_locale();
    let results = vec![
        SearchResult {
            note_path: "a.md".into(),
            breadcrumb: "a.md".into(),
            content: "AAA".into(),
            raw_content: "AAA".into(),
            links: vec![],
            score: 0.1,
            search_type: kajet_core::types::SearchType::Vector,
            chunk_index: 0,
        },
        SearchResult {
            note_path: "b.md".into(),
            breadcrumb: "b.md".into(),
            content: "BBB".into(),
            raw_content: "BBB".into(),
            links: vec![],
            score: 0.5,
            search_type: kajet_core::types::SearchType::Fts,
            chunk_index: 0,
        },
    ];
    let result = format_results("query", &results);
    assert!(result.contains("Found 2 results"));
    assert!(result.contains("Result 1"));
    assert!(result.contains("Result 2"));
}

#[test]
fn format_results_polish_content() {
    setup_locale();
    let results = vec![SearchResult {
            note_path: "łódź.md".into(),
            breadcrumb: "łódź.md > Główne zabytki".into(),
            content: "Pałac Izraela Poznańskiego — największy pałac przemysłowca w Europie. Zażółć gęślą jaźń.".into(),
            raw_content: "Pałac Izraela Poznańskiego — największy pałac przemysłowca w Europie. Zażółć gęślą jaźń.".into(),
            links: vec![],
            score: 0.8765,
            search_type: kajet_core::types::SearchType::Vector,
            chunk_index: 0,
        }];
    let result = format_results("pałac", &results);
    assert!(result.contains("Path: łódź.md"));
    assert!(result.contains("Główne zabytki"));
    assert!(result.contains("Poznańskiego"));
    assert!(result.contains("jaźń"));
}

#[test]
fn format_results_polish_query() {
    setup_locale();
    let result = format_results("zażółć gęślą jaźń", &[]);
    assert!(result.contains("zażółć gęślą jaźń"));
}

// -- format_list_tags tests --

#[test]
fn format_list_tags_empty() {
    setup_locale();
    let result = format_list_tags(&[], TagListDetail::Counts, None);
    assert_eq!(result, "No tags found");
}

#[test]
fn format_list_tags_names_mode() {
    setup_locale();
    let tags = vec![
        ("rust".into(), vec!["a.md".into(), "b.md".into()]),
        ("python".into(), vec!["c.md".into()]),
    ];
    let result = format_list_tags(&tags, TagListDetail::Names, None);
    assert!(result.contains("Tags (2 unique):"));
    assert!(result.contains("#rust, #python"));
}

#[test]
fn format_list_tags_counts_mode() {
    setup_locale();
    let tags = vec![
        ("rust".into(), vec!["a.md".into(), "b.md".into()]),
        ("python".into(), vec!["c.md".into()]),
    ];
    let result = format_list_tags(&tags, TagListDetail::Counts, None);
    assert!(result.contains("#rust: 2 notes"));
    assert!(result.contains("#python: 1 notes"));
}

#[test]
fn format_list_tags_full_mode() {
    setup_locale();
    let tags = vec![
        ("rust".into(), vec!["a.md".into(), "b.md".into()]),
        ("python".into(), vec!["c.md".into()]),
    ];
    let result = format_list_tags(&tags, TagListDetail::Full, None);
    assert!(result.contains("#rust (2 notes): a.md, b.md"));
    assert!(result.contains("#python (1 notes): c.md"));
}

#[test]
fn format_list_tags_with_folder() {
    setup_locale();
    let tags = vec![("journal".into(), vec!["journal/2025-01.md".into()])];
    let result = format_list_tags(&tags, TagListDetail::Counts, Some("journal"));
    assert!(result.contains("Tags in 'journal' (1 unique):"));
}

#[test]
fn format_list_tags_deduplication() {
    setup_locale();
    let tags = vec![("rust".into(), vec!["a.md".into(), "b.md".into()])];
    let result = format_list_tags(&tags, TagListDetail::Counts, None);
    assert!(result.contains("#rust: 2 notes"));
}

// -- format_entries tests --

#[test]
fn format_entries_empty() {
    setup_locale();
    let result = format_entries(None, None, &[]);
    assert_eq!(result, "No entries found matching filters");
}

#[test]
fn format_entries_single_doc() {
    setup_locale();
    let doc = Document {
        source_file: "test.md".into(),
        full_text: "This is test content".into(),
        title: "Test Note".into(),
        tags: vec!["test".into(), "example".into()],
        content_hash: "hash123".into(),
        last_modified: 1704067200.0, // 2024-01-01 00:00:00 UTC
        outgoing_links: vec![],
        backlinks: vec![],
    };
    let result = format_entries(None, None, &[doc]);
    assert!(result.contains("Entries — 1 results"));
    assert!(result.contains("## Test Note"));
    assert!(result.contains("Path: test.md"));
    assert!(result.contains("Date: 2024-01-01"));
    assert!(result.contains("#test #example"));
    assert!(result.contains("This is test content"));
}

#[test]
fn format_entries_with_date_range() {
    setup_locale();
    let doc = Document {
        source_file: "journal/2024-01-01.md".into(),
        full_text: "Journal entry".into(),
        title: "Daily Note".into(),
        tags: vec![],
        content_hash: "hash".into(),
        last_modified: 1704067200.0,
        outgoing_links: vec![],
        backlinks: vec![],
    };
    let from = chrono::NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
    let to = chrono::NaiveDate::from_ymd_opt(2024, 1, 31).unwrap();
    let result = format_entries(Some(from), Some(to), &[doc]);
    assert!(result.contains("Entries (2024-01-01 → 2024-01-31)"));
    assert!(result.contains("## Daily Note"));
}

#[test]
fn format_entries_no_tags() {
    setup_locale();
    let doc = Document {
        source_file: "note.md".into(),
        full_text: "Content".into(),
        title: "Note".into(),
        tags: vec![],
        content_hash: "hash".into(),
        last_modified: 1704067200.0,
        outgoing_links: vec![],
        backlinks: vec![],
    };
    let result = format_entries(None, None, &[doc]);
    assert!(result.contains("Date: 2024-01-01 | Tags: none"));
}

#[test]
fn format_entries_long_content_truncation() {
    setup_locale();
    let long_text = "a".repeat(300);
    let doc = Document {
        source_file: "long.md".into(),
        full_text: long_text.clone(),
        title: "Long Note".into(),
        tags: vec![],
        content_hash: "hash".into(),
        last_modified: 1704067200.0,
        outgoing_links: vec![],
        backlinks: vec![],
    };
    let result = format_entries(None, None, &[doc]);
    assert!(result.contains("..."));
    // Should be truncated to ~200 chars
    assert!(!result.contains(&"a".repeat(250)));
}

#[test]
fn format_vault_tree_folders_only() {
    setup_locale();
    let tree = kajet_parser::VaultFolder {
        name: "vault".into(),
        rel_path: "".into(),
        note_count: 2,
        subfolders: vec![kajet_parser::VaultFolder {
            name: "journal".into(),
            rel_path: "journal".into(),
            note_count: 10,
            subfolders: vec![],
            files: vec![],
            total_notes_in_vault: None,
        }],
        files: vec![],
        total_notes_in_vault: None,
    };
    let result = format_vault_tree(&tree);
    assert!(result.contains("12")); // 2 + 10 total notes
    assert!(result.contains("journal/ (10)"));
}

#[test]
fn format_vault_tree_with_files() {
    setup_locale();
    let tree = kajet_parser::VaultFolder {
        name: "vault".into(),
        rel_path: "".into(),
        note_count: 1,
        subfolders: vec![],
        files: vec!["readme.md".into()],
        total_notes_in_vault: None,
    };
    let result = format_vault_tree(&tree);
    assert!(result.contains("readme.md"));
}

#[test]
fn format_tree_too_large_output() {
    setup_locale();
    let result = format_tree_too_large(8000, 5000, 5, false);
    assert!(result.contains("8000"));
    assert!(result.contains("5000"));
}

#[test]
fn snapshot_format_results_with_links() {
    setup_locale();
    let results = vec![SearchResult {
        note_path: "journal/2026-02-11.md".into(),
        breadcrumb: "journal/2026-02-11.md > Notes".into(),
        content: "Worked on MCP formatter refactor.".into(),
        raw_content: "Worked on MCP formatter refactor.".into(),
        links: vec![
            Link {
                target: "Rust".into(),
                alias: None,
                resolved_path: Some("tech/rust.md".into()),
            },
            Link {
                target: "Design".into(),
                alias: Some("ADR".into()),
                resolved_path: None,
            },
        ],
        score: 0.9375,
        search_type: SearchType::Hybrid,
        chunk_index: 0,
    }];

    insta::assert_snapshot!(
        "format_results_with_links",
        format_results("refactor", &results)
    );
}

#[test]
fn snapshot_format_examine_slice() {
    setup_locale();
    let doc = Document {
        source_file: "notes/refactor.md".into(),
        full_text: "Line 1\nLine 2 with unicode: zażółć gęślą jaźń 🚀\nLine 3".into(),
        title: "Refactor Notes".into(),
        tags: vec!["rust".into(), "mcp".into()],
        content_hash: "h".into(),
        last_modified: 1704067200.0,
        outgoing_links: vec!["Architecture".into(), "Tests".into()],
        backlinks: vec!["daily/2026-02-11.md".into()],
    };

    insta::assert_snapshot!(
        "format_examine_slice",
        format_examine_result(&doc, ExamineContentMode::Slice, 7, 45)
    );
}

#[test]
fn snapshot_format_entries_multiple_docs() {
    setup_locale();
    let docs = vec![
        Document {
            source_file: "journal/2026-02-10.md".into(),
            full_text: "First entry about tests and refactor".into(),
            title: "2026-02-10".into(),
            tags: vec!["daily".into(), "work".into()],
            content_hash: "a".into(),
            last_modified: 1770681600.0,
            outgoing_links: vec![],
            backlinks: vec![],
        },
        Document {
            source_file: "journal/2026-02-11.md".into(),
            full_text: "B".repeat(240),
            title: "2026-02-11".into(),
            tags: vec![],
            content_hash: "b".into(),
            last_modified: 1770768000.0,
            outgoing_links: vec![],
            backlinks: vec![],
        },
    ];
    let from = chrono::NaiveDate::from_ymd_opt(2026, 2, 10).unwrap();
    let to = chrono::NaiveDate::from_ymd_opt(2026, 2, 11).unwrap();

    insta::assert_snapshot!(
        "format_entries_multiple_docs",
        format_entries(Some(from), Some(to), &docs)
    );
}

#[test]
fn snapshot_format_vault_tree() {
    setup_locale();
    let tree = kajet_parser::VaultFolder {
        name: "vault".into(),
        rel_path: "".into(),
        note_count: 1,
        subfolders: vec![kajet_parser::VaultFolder {
            name: "journal".into(),
            rel_path: "journal".into(),
            note_count: 2,
            subfolders: vec![kajet_parser::VaultFolder {
                name: "2026".into(),
                rel_path: "journal/2026".into(),
                note_count: 2,
                subfolders: vec![],
                files: vec!["2026-02-10.md".into(), "2026-02-11.md".into()],
                total_notes_in_vault: None,
            }],
            files: vec!["index.md".into()],
            total_notes_in_vault: None,
        }],
        files: vec!["readme.md".into()],
        total_notes_in_vault: Some(4),
    };

    insta::assert_snapshot!("format_vault_tree", format_vault_tree(&tree));
}

proptest! {
    #[test]
    fn truncate_with_ellipsis_is_utf8_safe(
        chars in proptest::collection::vec(any::<char>(), 0..120),
        max in 0usize..240
    ) {
        let text: String = chars.into_iter().collect();
        let out = truncate_with_ellipsis(&text, max);

        if text.len() <= max {
            prop_assert_eq!(out, text);
        } else {
            prop_assert!(out.ends_with("..."));
            let prefix = &out[..out.len() - 3];
            prop_assert!(text.starts_with(prefix));
            prop_assert!(prefix.len() <= max);
        }
    }

    #[test]
    fn slice_by_char_boundary_matches_expected_semantics(
        chars in proptest::collection::vec(any::<char>(), 0..120),
        start in 0usize..240,
        end in 0usize..240,
    ) {
        let text: String = chars.into_iter().collect();
        let out = slice_by_char_boundary(&text, start, end);

        let expected_start = text.floor_char_boundary(start.min(text.len()));
        let mut expected_end = text.floor_char_boundary(end.min(text.len()));
        if expected_end < expected_start {
            expected_end = expected_start;
        }

        prop_assert_eq!(out, &text[expected_start..expected_end]);
    }
}
