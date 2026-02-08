use crate::frontmatter::strip_frontmatter;
use crate::sections::{find_section_by_heading, parse_sections, SectionLookupError};

/// Errors from section-based transforms.
#[derive(Debug, Clone)]
pub enum TransformError {
    HeadingNotFound { available: Vec<String> },
    HeadingAmbiguous { count: usize },
}

impl std::fmt::Display for TransformError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::HeadingNotFound { available } => {
                write!(f, "Heading not found. Available: {}", available.join(", "))
            }
            Self::HeadingAmbiguous { count } => {
                write!(f, "Heading is ambiguous: {count} matches found")
            }
        }
    }
}

impl std::error::Error for TransformError {}

impl From<SectionLookupError> for TransformError {
    fn from(err: SectionLookupError) -> Self {
        match err {
            SectionLookupError::NotFound { available } => {
                TransformError::HeadingNotFound { available }
            }
            SectionLookupError::Ambiguous { count } => TransformError::HeadingAmbiguous { count },
        }
    }
}

/// Position of a match in the content.
#[derive(Debug, Clone)]
pub struct MatchPosition {
    pub line: usize,
    pub column: usize,
    pub context: String,
}

/// Errors from `replace_text`.
#[derive(Debug, Clone)]
pub enum ReplaceError {
    NotFound,
    Ambiguous {
        count: usize,
        positions: Vec<MatchPosition>,
    },
}

impl std::fmt::Display for ReplaceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound => write!(f, "Text not found"),
            Self::Ambiguous { count, positions } => {
                writeln!(f, "Ambiguous: {count} matches found:")?;
                for pos in positions {
                    writeln!(
                        f,
                        "  line {}, col {}: ...{}...",
                        pos.line, pos.column, pos.context
                    )?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for ReplaceError {}

/// Append content at the end of the file, or at the end of a specific section.
pub fn append_content(
    content: &str,
    new_text: &str,
    heading: Option<&str>,
) -> Result<String, TransformError> {
    match heading {
        None => {
            let mut result = content.to_string();
            if !result.ends_with('\n') {
                result.push('\n');
            }
            result.push_str(new_text);
            if !result.ends_with('\n') {
                result.push('\n');
            }
            Ok(result)
        }
        Some(heading) => {
            let sections = parse_sections(content);
            let section = find_section_by_heading(&sections, heading)?;
            let body = &content[section.body_range.clone()];
            let trimmed_len = body.trim_end().len();
            let content_end = section.body_range.start + trimmed_len;
            let mut result = String::with_capacity(content.len() + new_text.len() + 2);
            result.push_str(&content[..content_end]);
            if !result.ends_with('\n') {
                result.push('\n');
            }
            result.push_str(new_text);
            if !result.ends_with('\n') {
                result.push('\n');
            }
            let remainder = &content[section.body_range.end..];
            if !remainder.is_empty() {
                result.push('\n');
            }
            result.push_str(remainder);
            Ok(result)
        }
    }
}

/// Prepend content after frontmatter (or at start), or right after a specific heading.
pub fn prepend_content(
    content: &str,
    new_text: &str,
    heading: Option<&str>,
) -> Result<String, TransformError> {
    match heading {
        None => {
            let (fm, body) = strip_frontmatter(content);
            let mut result = String::new();
            if !fm.is_empty() {
                result.push_str("---\n");
                result.push_str(&fm);
                result.push_str("\n---\n");
            }
            result.push_str(new_text);
            if !result.ends_with('\n') {
                result.push('\n');
            }
            let body_trimmed = body.trim_start_matches('\n');
            if !body_trimmed.is_empty() {
                result.push_str(body_trimmed);
                if !result.ends_with('\n') {
                    result.push('\n');
                }
            }
            Ok(result)
        }
        Some(heading) => {
            let sections = parse_sections(content);
            let section = find_section_by_heading(&sections, heading)?;
            let insert_pos = section.heading_range.end;
            let mut result = String::with_capacity(content.len() + new_text.len() + 2);
            result.push_str(&content[..insert_pos]);
            if !result.ends_with('\n') {
                result.push('\n');
            }
            result.push_str(new_text);
            if !result.ends_with('\n') {
                result.push('\n');
            }
            result.push_str(&content[insert_pos..]);
            Ok(result)
        }
    }
}

/// Replace the entire body while preserving frontmatter.
pub fn overwrite_body(content: &str, new_text: &str) -> String {
    let (fm, _) = strip_frontmatter(content);
    let mut result = String::new();
    if !fm.is_empty() {
        result.push_str("---\n");
        result.push_str(&fm);
        result.push_str("\n---\n");
    }
    result.push_str(new_text);
    if !result.ends_with('\n') {
        result.push('\n');
    }
    result
}

/// Replace a section's body (heading + body) with new content.
///
/// The heading line itself is preserved; only the body is replaced.
pub fn replace_section(
    content: &str,
    heading: &str,
    new_text: &str,
) -> Result<String, TransformError> {
    let sections = parse_sections(content);
    let section = find_section_by_heading(&sections, heading)?;
    let mut result = String::with_capacity(content.len() + new_text.len());
    result.push_str(&content[..section.heading_range.end]);
    if !result.ends_with('\n') {
        result.push('\n');
    }
    result.push_str(new_text);
    if !result.ends_with('\n') {
        result.push('\n');
    }
    let remainder = &content[section.body_range.end..];
    if !remainder.is_empty() {
        result.push('\n');
    }
    result.push_str(remainder);
    Ok(result)
}

/// Replace exact text. Errors on 0 or 2+ matches.
pub fn replace_text(content: &str, old: &str, new: &str) -> Result<String, ReplaceError> {
    let matches: Vec<usize> = content.match_indices(old).map(|(i, _)| i).collect();

    match matches.len() {
        0 => Err(ReplaceError::NotFound),
        1 => Ok(content.replacen(old, new, 1)),
        n => {
            let positions = matches
                .iter()
                .map(|&byte_pos| {
                    let before = &content[..byte_pos];
                    let line = before.matches('\n').count() + 1;
                    let last_newline = before.rfind('\n').map(|p| p + 1).unwrap_or(0);
                    let column = content[last_newline..byte_pos].chars().count() + 1;
                    let ctx_start = content.floor_char_boundary(byte_pos.saturating_sub(20));
                    let ctx_end =
                        content.floor_char_boundary((byte_pos + old.len() + 20).min(content.len()));
                    let context = content[ctx_start..ctx_end].replace('\n', "\\n");
                    MatchPosition {
                        line,
                        column,
                        context,
                    }
                })
                .collect();
            Err(ReplaceError::Ambiguous {
                count: n,
                positions,
            })
        }
    }
}

/// Insert content immediately after an exact text anchor. Errors on 0 or 2+ matches.
pub fn insert_after(content: &str, anchor: &str, new_text: &str) -> Result<String, ReplaceError> {
    let matches: Vec<usize> = content.match_indices(anchor).map(|(i, _)| i).collect();

    match matches.len() {
        0 => Err(ReplaceError::NotFound),
        1 => {
            let pos = matches[0] + anchor.len();
            let mut result = String::with_capacity(content.len() + new_text.len() + 2);
            result.push_str(&content[..pos]);
            // Ensure newline between anchor and inserted content
            if !result.ends_with('\n') && !new_text.starts_with('\n') {
                result.push('\n');
            }
            result.push_str(new_text);
            // Ensure newline before remainder
            if !result.ends_with('\n') && !content[pos..].starts_with('\n') {
                result.push('\n');
            }
            result.push_str(&content[pos..]);
            Ok(result)
        }
        n => {
            let positions = matches
                .iter()
                .map(|&byte_pos| {
                    let before = &content[..byte_pos];
                    let line = before.matches('\n').count() + 1;
                    let last_newline = before.rfind('\n').map(|p| p + 1).unwrap_or(0);
                    let column = content[last_newline..byte_pos].chars().count() + 1;
                    let ctx_start = content.floor_char_boundary(byte_pos.saturating_sub(20));
                    let ctx_end = content
                        .floor_char_boundary((byte_pos + anchor.len() + 20).min(content.len()));
                    let context = content[ctx_start..ctx_end].replace('\n', "\\n");
                    MatchPosition {
                        line,
                        column,
                        context,
                    }
                })
                .collect();
            Err(ReplaceError::Ambiguous {
                count: n,
                positions,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- append_content tests --

    #[test]
    fn append_to_file() {
        let content = "# Title\n\nExisting body.\n";
        let result = append_content(content, "New line.", None).unwrap();
        assert!(result.ends_with("New line.\n"));
        assert!(result.contains("Existing body."));
    }

    #[test]
    fn append_to_section() {
        let content = "# Title\n\nBody\n\n## Tasks\n\n- Task 1\n\n## Notes\n\nNote body\n";
        let result = append_content(content, "- Task 2", Some("## Tasks")).unwrap();
        // Task 2 should be between Tasks and Notes sections
        let tasks_pos = result.find("- Task 2").unwrap();
        let notes_pos = result.find("## Notes").unwrap();
        assert!(tasks_pos < notes_pos);
        assert!(result.contains("- Task 1"));
        // No extra blank line between existing and new content
        assert!(result.contains("- Task 1\n- Task 2"));
        // Blank line before next heading is preserved
        assert!(result.contains("- Task 2\n\n## Notes"));
    }

    #[test]
    fn append_to_last_section() {
        let content = "# Title\n\n## End\n\nLast content.\n";
        let result = append_content(content, "Appended.", Some("## End")).unwrap();
        assert!(result.contains("Last content."));
        assert!(result.contains("Appended."));
    }

    // -- prepend_content tests --

    #[test]
    fn prepend_no_frontmatter() {
        let content = "# Title\n\nBody\n";
        let result = prepend_content(content, "Prepended text.", None).unwrap();
        let prepend_pos = result.find("Prepended text.").unwrap();
        let title_pos = result.find("# Title").unwrap();
        assert!(prepend_pos < title_pos);
    }

    #[test]
    fn prepend_with_frontmatter() {
        let content = "---\ntags: [test]\n---\n# Title\n\nBody\n";
        let result = prepend_content(content, "Prepended.", None).unwrap();
        // Frontmatter should be preserved
        assert!(result.contains("---\ntags: [test]\n---"));
        // Prepended text should come after frontmatter but before body
        let fm_end = result.find("---\n").unwrap();
        let second_fm = result[fm_end + 4..].find("---\n").unwrap() + fm_end + 4;
        let prepend_pos = result.find("Prepended.").unwrap();
        let title_pos = result.find("# Title").unwrap();
        assert!(prepend_pos > second_fm);
        assert!(prepend_pos < title_pos);
    }

    #[test]
    fn prepend_with_heading() {
        let content = "# Title\n\nBody\n\n## Tasks\n\nExisting tasks.\n";
        let result = prepend_content(content, "New task.", Some("## Tasks")).unwrap();
        let heading_pos = result.find("## Tasks").unwrap();
        let new_pos = result.find("New task.").unwrap();
        let existing_pos = result.find("Existing tasks.").unwrap();
        assert!(new_pos > heading_pos);
        assert!(new_pos < existing_pos);
    }

    // -- overwrite_body tests --

    #[test]
    fn overwrite_no_frontmatter() {
        let content = "# Old Title\n\nOld body.\n";
        let result = overwrite_body(content, "# New Title\n\nNew body.");
        assert_eq!(result, "# New Title\n\nNew body.\n");
        assert!(!result.contains("Old"));
    }

    #[test]
    fn overwrite_with_frontmatter() {
        let content = "---\ntags: [keep]\n---\n# Old Title\n\nOld body.\n";
        let result = overwrite_body(content, "# New Title\n\nNew body.");
        assert!(result.contains("tags: [keep]"));
        assert!(result.contains("New body."));
        assert!(!result.contains("Old body."));
    }

    // -- replace_section tests --

    #[test]
    fn replace_section_basic() {
        let content = "# Title\n\n## Tasks\n\n- Old task\n\n## Notes\n\nNote body\n";
        let result = replace_section(content, "## Tasks", "- New task 1\n- New task 2").unwrap();
        assert!(result.contains("## Tasks"));
        assert!(result.contains("- New task 1"));
        assert!(result.contains("- New task 2"));
        assert!(!result.contains("- Old task"));
        assert!(result.contains("## Notes"));
        assert!(result.contains("Note body"));
        // Blank line before next heading is preserved
        assert!(result.contains("- New task 2\n\n## Notes"));
    }

    #[test]
    fn replace_section_with_subsections() {
        let content =
            "## Parent\n\nParent body\n\n### Child\n\nChild body\n\n## Sibling\n\nSibling body\n";
        let result = replace_section(content, "## Parent", "Replaced content.").unwrap();
        assert!(result.contains("## Parent"));
        assert!(result.contains("Replaced content."));
        assert!(!result.contains("Parent body"));
        assert!(!result.contains("Child body")); // subsections are part of parent
        assert!(result.contains("## Sibling"));
        assert!(result.contains("Sibling body"));
    }

    #[test]
    fn replace_section_heading_not_found() {
        let content = "# Title\n\n## Tasks\n\nBody\n";
        let err = replace_section(content, "## Nonexistent", "New").unwrap_err();
        match err {
            TransformError::HeadingNotFound { available } => {
                assert!(available.contains(&"Tasks".to_string()));
            }
            _ => panic!("Expected HeadingNotFound"),
        }
    }

    // -- replace_text tests --

    #[test]
    fn replace_text_single() {
        let content = "Hello world, this is a test.";
        let result = replace_text(content, "world", "earth").unwrap();
        assert_eq!(result, "Hello earth, this is a test.");
    }

    #[test]
    fn replace_text_not_found() {
        let content = "Hello world.";
        let err = replace_text(content, "mars", "earth").unwrap_err();
        assert!(matches!(err, ReplaceError::NotFound));
    }

    #[test]
    fn replace_text_ambiguous() {
        let content = "foo bar foo baz foo";
        let err = replace_text(content, "foo", "qux").unwrap_err();
        match err {
            ReplaceError::Ambiguous { count, positions } => {
                assert_eq!(count, 3);
                assert_eq!(positions.len(), 3);
                assert_eq!(positions[0].line, 1);
                assert_eq!(positions[0].column, 1);
            }
            _ => panic!("Expected Ambiguous"),
        }
    }

    #[test]
    fn replace_text_multiline() {
        let content = "Line one\nLine two\nLine three\n";
        let result = replace_text(content, "Line two", "LINE TWO").unwrap();
        assert_eq!(result, "Line one\nLINE TWO\nLine three\n");
    }

    // -- insert_after tests --

    #[test]
    fn insert_after_basic() {
        let content = "# Title\n\n- Item 1\n- Item 2\n\n## Notes\n";
        // Agent provides anchor and content without worrying about newlines
        let result = insert_after(content, "- Item 1", "- Item 1.5").unwrap();
        assert!(result.contains("- Item 1\n- Item 1.5\n- Item 2"));
    }

    #[test]
    fn insert_after_anchor_with_trailing_newline() {
        let content = "# Title\n\n- Item 1\n- Item 2\n";
        let result = insert_after(content, "- Item 1\n", "- Item 1.5\n").unwrap();
        assert!(result.contains("- Item 1\n- Item 1.5\n- Item 2"));
    }

    #[test]
    fn insert_after_not_found() {
        let content = "Hello world.";
        let err = insert_after(content, "mars", "new text").unwrap_err();
        assert!(matches!(err, ReplaceError::NotFound));
    }

    #[test]
    fn insert_after_ambiguous() {
        let content = "foo bar foo baz foo";
        let err = insert_after(content, "foo", "new").unwrap_err();
        match err {
            ReplaceError::Ambiguous { count, positions } => {
                assert_eq!(count, 3);
                assert_eq!(positions.len(), 3);
            }
            _ => panic!("Expected Ambiguous"),
        }
    }

    #[test]
    fn insert_after_multiline_anchor() {
        let content = "First line\nSecond line\nThird line\n";
        let result = insert_after(content, "First line\nSecond line", "Inserted").unwrap();
        assert!(result.contains("Second line\nInserted\nThird line"));
    }

    #[test]
    fn polish_content_transforms() {
        let content = "# Główne tematy\n\nTreść po polsku z [[linkiem]].\n\n## Współpraca\n\nSzczegóły współpracy.\n";
        let result = append_content(content, "Nowa treść.", Some("## Współpraca")).unwrap();
        assert!(result.contains("Szczegóły współpracy."));
        assert!(result.contains("Nowa treść."));
        assert!(result.contains("Główne tematy"));
    }
}
