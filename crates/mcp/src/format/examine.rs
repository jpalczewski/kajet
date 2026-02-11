use crate::domain::documents_input::ExamineContentMode;
use kajet_core::text_utils::{slice_by_char_boundary, truncate_with_ellipsis};
use kajet_core::types::Document;

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
