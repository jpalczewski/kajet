use kajet_core::search::SearchResult;
use kajet_core::types::Document;
use kajet_writer::{CreateNoteResult, EditNoteResult};

pub fn format_results(query: &str, results: &[SearchResult]) -> String {
    if results.is_empty() {
        return t!("no_results", query = query).to_string();
    }

    let formatted = results
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let mut parts = format!(
                "{}\n{}\n{}\n\n{}",
                t!(
                    "result_header",
                    index = i + 1,
                    score = format!("{:.4}", r.score)
                ),
                t!("result_path", path = &r.note_path),
                t!("result_section", breadcrumb = &r.breadcrumb),
                r.content,
            );
            if !r.links.is_empty() {
                let link_list: Vec<String> = r
                    .links
                    .iter()
                    .map(|l| match (&l.alias, &l.resolved_path) {
                        (Some(alias), Some(path)) => {
                            format!("  - {} → {} ({})", l.target, path, alias)
                        }
                        (None, Some(path)) => format!("  - {} → {}", l.target, path),
                        (Some(alias), None) => format!("  - {} ({})", l.target, alias),
                        (None, None) => format!("  - {}", l.target),
                    })
                    .collect();
                parts.push_str(&format!(
                    "\n\n{}\n{}",
                    t!("result_links"),
                    link_list.join("\n")
                ));
            }
            parts
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    format!(
        "{}\n\n{}",
        t!("found_results", count = results.len(), query = query),
        formatted
    )
}

pub fn format_examine_result(
    doc: &Document,
    content_mode: &str,
    offset: usize,
    length: usize,
) -> String {
    let mut parts = Vec::new();

    // Title
    parts.push(t!("examine_title", title = &doc.title).to_string());

    // Tags
    if doc.tags.is_empty() {
        parts.push(t!("examine_no_tags").to_string());
    } else {
        parts.push(t!("examine_tags", tags = doc.tags.join(", ")).to_string());
    }

    // Outgoing links
    let no_links = t!("examine_no_links").to_string();
    if doc.outgoing_links.is_empty() {
        parts.push(format!(
            "{}\n  {}",
            t!("examine_outgoing_links", count = 0),
            no_links
        ));
    } else {
        let links_list = doc
            .outgoing_links
            .iter()
            .map(|l| format!("  - {l}"))
            .collect::<Vec<_>>()
            .join("\n");
        parts.push(format!(
            "{}\n{}",
            t!("examine_outgoing_links", count = doc.outgoing_links.len()),
            links_list
        ));
    }

    // Backlinks
    if doc.backlinks.is_empty() {
        parts.push(format!(
            "{}\n  {}",
            t!("examine_backlinks", count = 0),
            no_links
        ));
    } else {
        let bl_list = doc
            .backlinks
            .iter()
            .map(|l| format!("  - {l}"))
            .collect::<Vec<_>>()
            .join("\n");
        parts.push(format!(
            "{}\n{}",
            t!("examine_backlinks", count = doc.backlinks.len()),
            bl_list
        ));
    }

    // Content
    let total = doc.full_text.len();
    let text_slice = match content_mode {
        "full" => doc.full_text.clone(),
        "slice" => {
            let start = offset.min(total);
            let end = (start + length).min(total);
            let start = doc.full_text.floor_char_boundary(start);
            let end = doc.full_text.floor_char_boundary(end);
            doc.full_text[start..end].to_string()
        }
        _ => {
            // summary: first 500 chars
            let end = total.min(500);
            let end = doc.full_text.floor_char_boundary(end);
            let slice = &doc.full_text[..end];
            if end < total {
                format!("{slice}...")
            } else {
                slice.to_string()
            }
        }
    };

    parts.push(
        t!(
            "examine_content_info",
            shown = text_slice.len(),
            total = total
        )
        .to_string(),
    );
    parts.push(text_slice);

    parts.join("\n")
}

pub fn format_list_tags(
    tag_map: &[(String, Vec<String>)],
    detail: &str,
    folder: Option<&str>,
) -> String {
    if tag_map.is_empty() {
        return t!("list_tags_empty").to_string();
    }

    let header = match folder {
        Some(f) => t!("list_tags_folder", folder = f, count = tag_map.len()).to_string(),
        None => t!("list_tags_header", count = tag_map.len()).to_string(),
    };

    let body = match detail {
        "names" => tag_map
            .iter()
            .map(|(tag, _)| format!("#{tag}"))
            .collect::<Vec<_>>()
            .join(", "),
        "full" => tag_map
            .iter()
            .map(|(tag, paths)| {
                let files = paths.join(", ");
                format!("#{tag} ({} notes): {files}", paths.len())
            })
            .collect::<Vec<_>>()
            .join("\n"),
        _ => tag_map
            .iter()
            .map(|(tag, paths)| format!("#{tag}: {} notes", paths.len()))
            .collect::<Vec<_>>()
            .join("\n"),
    };

    format!("{header}\n{body}")
}

pub fn format_create_result(result: &CreateNoteResult) -> String {
    t!(
        "create_note_success",
        path = &result.path,
        bytes = result.bytes_written
    )
    .to_string()
}

pub fn format_edit_result(result: &EditNoteResult) -> String {
    let mut text = t!(
        "edit_note_success",
        path = &result.path,
        mode = &result.mode,
        bytes = result.bytes_written
    )
    .to_string();

    if result.backup_path.is_some() {
        text.push_str(&format!("\n{}", t!("edit_note_backup_created")));
    }

    text
}

pub fn format_edit_tags_result(
    path: &str,
    added: &Option<Vec<String>>,
    removed: &Option<Vec<String>>,
) -> String {
    let mut text = t!("edit_tags_success", path = path).to_string();

    if let Some(tags) = added
        && !tags.is_empty()
    {
        let tags_str = tags.join(", ");
        text.push_str(&format!("\n  {}", t!("edit_tags_added", tags = tags_str)));
    }

    if let Some(tags) = removed
        && !tags.is_empty()
    {
        let tags_str = tags.join(", ");
        text.push_str(&format!("\n  {}", t!("edit_tags_removed", tags = tags_str)));
    }

    text
}

#[cfg(test)]
mod tests {
    use super::*;

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
            },
            SearchResult {
                note_path: "b.md".into(),
                breadcrumb: "b.md".into(),
                content: "BBB".into(),
                raw_content: "BBB".into(),
                links: vec![],
                score: 0.5,
                search_type: kajet_core::types::SearchType::Fts,
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
        let result = format_list_tags(&[], "counts", None);
        assert_eq!(result, "No tags found");
    }

    #[test]
    fn format_list_tags_names_mode() {
        setup_locale();
        let tags = vec![
            ("rust".into(), vec!["a.md".into(), "b.md".into()]),
            ("python".into(), vec!["c.md".into()]),
        ];
        let result = format_list_tags(&tags, "names", None);
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
        let result = format_list_tags(&tags, "counts", None);
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
        let result = format_list_tags(&tags, "full", None);
        assert!(result.contains("#rust (2 notes): a.md, b.md"));
        assert!(result.contains("#python (1 notes): c.md"));
    }

    #[test]
    fn format_list_tags_with_folder() {
        setup_locale();
        let tags = vec![("journal".into(), vec!["journal/2025-01.md".into()])];
        let result = format_list_tags(&tags, "counts", Some("journal"));
        assert!(result.contains("Tags in 'journal' (1 unique):"));
    }

    #[test]
    fn format_list_tags_deduplication() {
        setup_locale();
        let tags = vec![("rust".into(), vec!["a.md".into(), "b.md".into()])];
        let result = format_list_tags(&tags, "counts", None);
        assert!(result.contains("#rust: 2 notes"));
    }
}
