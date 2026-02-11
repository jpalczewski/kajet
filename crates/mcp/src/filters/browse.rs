use crate::filters::tags_match;
use kajet_core::types::Document;

/// Filter browse-mode documents by tags, then truncate to limit.
pub fn filter_browse_results(docs: &mut Vec<Document>, tags: Option<&[String]>, limit: usize) {
    if let Some(required_tags) = tags {
        docs.retain(|doc| tags_match(&doc.tags, required_tags));
    }

    docs.truncate(limit);
}
