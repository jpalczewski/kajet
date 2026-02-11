/// Tag with the list of files that contain it.
pub struct TagStats {
    pub tag: String,
    pub files: Vec<String>,
}

/// Result of applying tag edits to a note.
pub struct TagEditResult {
    pub content: String,
    pub tags_added: usize,
    pub tags_removed: usize,
    pub timestamp_updated: bool,
}
