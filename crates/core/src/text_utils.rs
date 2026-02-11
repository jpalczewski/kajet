/// Safely slice a string using byte offsets, clamped to UTF-8 character boundaries.
pub fn slice_by_char_boundary(text: &str, start: usize, end: usize) -> &str {
    let start = text.floor_char_boundary(start.min(text.len()));
    let mut end = text.floor_char_boundary(end.min(text.len()));

    if end < start {
        end = start;
    }

    &text[start..end]
}

/// Truncate a string by byte length on a UTF-8 boundary and append `...`.
pub fn truncate_with_ellipsis(text: &str, max_bytes: usize) -> String {
    if text.len() <= max_bytes {
        return text.to_string();
    }

    let end = text.floor_char_boundary(max_bytes);
    format!("{}...", &text[..end])
}
