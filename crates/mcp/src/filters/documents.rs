use crate::filters::tags_match;
use kajet_core::path_utils::{normalize_folder_prefix, path_matches_folder_prefix};
use kajet_core::types::Document;

pub(crate) struct DocumentFilters<'a> {
    folder_prefix: Option<String>,
    from_ts: Option<f64>,
    to_ts: Option<f64>,
    tags: Option<&'a [String]>,
}

impl<'a> DocumentFilters<'a> {
    pub(crate) fn new(
        folder: Option<&str>,
        from_ts: Option<f64>,
        to_ts: Option<f64>,
        tags: Option<&'a [String]>,
    ) -> Self {
        Self {
            folder_prefix: folder.map(normalize_folder_prefix),
            from_ts,
            to_ts,
            tags,
        }
    }

    pub(crate) fn matches(&self, doc: &Document) -> bool {
        if !path_matches_folder_prefix(&doc.source_file, self.folder_prefix.as_deref(), true) {
            return false;
        }

        if let Some(from) = self.from_ts
            && doc.last_modified < from
        {
            return false;
        }

        if let Some(to) = self.to_ts
            && doc.last_modified > to
        {
            return false;
        }

        if let Some(required_tags) = self.tags
            && !tags_match(&doc.tags, required_tags)
        {
            return false;
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(path: &str, ts: f64, tags: &[&str]) -> Document {
        Document {
            source_file: path.to_string(),
            full_text: String::new(),
            title: String::new(),
            tags: tags.iter().map(|tag| tag.to_string()).collect(),
            content_hash: String::new(),
            last_modified: ts,
            outgoing_links: vec![],
            backlinks: vec![],
        }
    }

    #[test]
    fn filters_by_folder_date_and_tags() {
        let required_tags = vec!["rust".to_string()];
        let filters = DocumentFilters::new(
            Some("journal/2026"),
            Some(200.0),
            Some(400.0),
            Some(&required_tags),
        );

        assert!(filters.matches(&doc("journal/2026/today.md", 250.0, &["rust", "mcp"])));
        assert!(!filters.matches(&doc("projects/idea.md", 250.0, &["rust"])));
        assert!(!filters.matches(&doc("journal/2026/today.md", 450.0, &["rust"])));
        assert!(!filters.matches(&doc("journal/2026/today.md", 250.0, &["notes"])));
    }
}
