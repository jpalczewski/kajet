use std::collections::HashSet;

fn normalize_tag(tag: &str) -> String {
    tag.strip_prefix('#').unwrap_or(tag).to_lowercase()
}

/// Check if document tags contain all required tags (case-insensitive, `#` prefix ignored).
pub fn tags_match(doc_tags: &[String], required: &[String]) -> bool {
    let doc_tags_normalized: HashSet<String> = doc_tags.iter().map(|t| normalize_tag(t)).collect();

    required
        .iter()
        .all(|required_tag| doc_tags_normalized.contains(&normalize_tag(required_tag)))
}
