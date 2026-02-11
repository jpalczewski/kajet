use chrono::NaiveDate;
use kajet_core::text_utils::truncate_with_ellipsis;
use kajet_core::types::Document;

/// Format a list of documents for browse mode output.
///
/// This function produces a chronologically-ordered, human-readable list of document entries
/// with metadata (title, path, date, tags) and content snippets.
///
/// # Content Truncation
///
/// Document content is truncated to approximately 200 characters with "..." suffix if longer.
pub fn format_entries(from: Option<NaiveDate>, to: Option<NaiveDate>, docs: &[Document]) -> String {
    if docs.is_empty() {
        return t!("browse_no_results").to_string();
    }

    let header = match (from, to) {
        (Some(f), Some(t)) => t!(
            "browse_header",
            from = f.to_string(),
            to = t.to_string(),
            count = docs.len()
        )
        .to_string(),
        _ => t!("browse_header_no_dates", count = docs.len()).to_string(),
    };

    let entries: Vec<String> = docs
        .iter()
        .map(|doc| {
            let date = chrono::DateTime::from_timestamp(doc.last_modified as i64, 0)
                .map(|dt| dt.format("%Y-%m-%d").to_string())
                .unwrap_or_else(|| "unknown".to_string());

            let meta = if doc.tags.is_empty() {
                t!("browse_entry_no_tags", date = &date).to_string()
            } else {
                let tags = doc
                    .tags
                    .iter()
                    .map(|tag| format!("#{tag}"))
                    .collect::<Vec<_>>()
                    .join(" ");
                t!("browse_entry_meta", date = &date, tags = tags).to_string()
            };

            let snippet = truncate_with_ellipsis(&doc.full_text, 200);

            format!(
                "## {}\n{}\n{}\n{}",
                doc.title,
                t!("browse_entry_path", path = &doc.source_file),
                meta,
                snippet
            )
        })
        .collect();

    format!("{}\n\n{}", header, entries.join("\n\n"))
}
