use crate::domain::documents_input::ExamineContentMode;
use kajet_core::text_utils::{slice_by_char_boundary, truncate_with_ellipsis};
use kajet_core::types::Document;

pub(crate) enum ExamineBatchSection {
    Success {
        requested_path: String,
        body: String,
    },
    Error {
        requested_path: String,
        error: String,
    },
}

fn format_link_section(title: String, links: &[String], no_links: &str) -> String {
    if links.is_empty() {
        format!("{}\n  {}", title, no_links)
    } else {
        let links_list = links
            .iter()
            .map(|link| format!("  - {link}"))
            .collect::<Vec<_>>()
            .join("\n");
        format!("{}\n{}", title, links_list)
    }
}

pub(crate) fn format_examine_result(
    doc: &Document,
    content_mode: ExamineContentMode,
    offset: usize,
    length: usize,
) -> String {
    let mut parts = Vec::new();

    parts.push(t!("examine_title", title = &doc.title).to_string());

    if doc.tags.is_empty() {
        parts.push(t!("examine_no_tags").to_string());
    } else {
        parts.push(t!("examine_tags", tags = doc.tags.join(", ")).to_string());
    }

    let no_links = t!("examine_no_links").to_string();
    parts.push(format_link_section(
        t!("examine_outgoing_links", count = doc.outgoing_links.len()).to_string(),
        &doc.outgoing_links,
        &no_links,
    ));
    parts.push(format_link_section(
        t!("examine_backlinks", count = doc.backlinks.len()).to_string(),
        &doc.backlinks,
        &no_links,
    ));

    let total = doc.full_text.len();
    let text_slice = match content_mode {
        ExamineContentMode::Full => doc.full_text.clone(),
        ExamineContentMode::Slice => {
            let start = offset.min(total);
            let end = (start + length).min(total);
            slice_by_char_boundary(&doc.full_text, start, end).to_string()
        }
        ExamineContentMode::Summary => truncate_with_ellipsis(&doc.full_text, 500),
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

pub(crate) fn format_examine_batch(sections: &[ExamineBatchSection]) -> String {
    let mut parts = vec![t!("examine_batch_header", count = sections.len()).to_string()];

    for section in sections {
        parts.push(String::new());
        match section {
            ExamineBatchSection::Success {
                requested_path,
                body,
            } => {
                parts.push(
                    t!("examine_batch_separator", path = requested_path.as_str()).to_string(),
                );
                parts.push(body.clone());
            }
            ExamineBatchSection::Error {
                requested_path,
                error,
            } => {
                parts.push(
                    t!("examine_batch_separator", path = requested_path.as_str()).to_string(),
                );
                parts.push(t!("examine_batch_error", error = error.as_str()).to_string());
            }
        }
    }

    parts.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batch_formatter_includes_sections_and_errors() {
        rust_i18n::set_locale("en");

        let text = format_examine_batch(&[
            ExamineBatchSection::Success {
                requested_path: "a.md".to_string(),
                body: "Title: A".to_string(),
            },
            ExamineBatchSection::Error {
                requested_path: "missing.md".to_string(),
                error: "Document not found".to_string(),
            },
        ]);

        assert!(text.contains("Examined 2 documents"));
        assert!(text.contains("=== a.md ==="));
        assert!(text.contains("Title: A"));
        assert!(text.contains("=== missing.md ==="));
        assert!(text.contains("ERROR: Document not found"));
    }
}
