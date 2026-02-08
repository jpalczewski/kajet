use crate::chunker::heading_level_to_u8;
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use std::ops::Range;

/// A section of a markdown document, defined by its heading.
#[derive(Debug, Clone)]
pub struct Section {
    /// Heading level (1–6).
    pub level: u8,
    /// The text content of the heading (without `#` markers).
    pub heading_text: String,
    /// Byte range of the full heading line(s) in the source markdown.
    pub heading_range: Range<usize>,
    /// Byte range of the body (content between this heading and the next same-or-higher-level heading, or EOF).
    pub body_range: Range<usize>,
}

/// Errors from `find_section_by_heading`.
#[derive(Debug, Clone)]
pub enum SectionLookupError {
    NotFound { available: Vec<String> },
    Ambiguous { count: usize },
}

impl std::fmt::Display for SectionLookupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound { available } => {
                write!(f, "Heading not found. Available: {}", available.join(", "))
            }
            Self::Ambiguous { count } => {
                write!(f, "Heading is ambiguous: {count} matches found")
            }
        }
    }
}

impl std::error::Error for SectionLookupError {}

/// Parse all sections from a markdown document.
///
/// Returns sections with byte-accurate ranges for both heading and body.
/// Headings inside fenced code blocks are ignored.
pub fn parse_sections(markdown: &str) -> Vec<Section> {
    let parser = Parser::new_ext(markdown, Options::empty()).into_offset_iter();

    let mut sections = Vec::new();
    let mut in_heading = false;
    let mut heading_text = String::new();
    let mut heading_start: usize = 0;
    let mut heading_level: u8 = 0;
    let mut in_code_block = false;

    for (event, range) in parser {
        match event {
            Event::Start(Tag::CodeBlock(_)) => {
                in_code_block = true;
            }
            Event::End(TagEnd::CodeBlock) => {
                in_code_block = false;
            }
            Event::Start(Tag::Heading { level, .. }) if !in_code_block => {
                in_heading = true;
                heading_text.clear();
                heading_start = range.start;
                heading_level = heading_level_to_u8(level);
            }
            Event::End(TagEnd::Heading(_)) if !in_code_block => {
                in_heading = false;
                sections.push(Section {
                    level: heading_level,
                    heading_text: heading_text.clone(),
                    heading_range: heading_start..range.end,
                    body_range: range.end..range.end, // placeholder, computed below
                });
            }
            Event::Text(text) | Event::Code(text) if in_heading => {
                heading_text.push_str(&text);
            }
            _ => {}
        }
    }

    compute_body_ranges(&mut sections, markdown.len());
    sections
}

/// Compute body ranges: each section's body runs from after its heading
/// to the start of the next same-or-higher-level heading (or EOF).
fn compute_body_ranges(sections: &mut [Section], doc_len: usize) {
    for i in 0..sections.len() {
        let body_start = sections[i].heading_range.end;
        let body_end = sections[i + 1..]
            .iter()
            .find(|s| s.level <= sections[i].level)
            .map(|s| s.heading_range.start)
            .unwrap_or(doc_len);
        sections[i].body_range = body_start..body_end;
    }
}

/// Find a section by its heading text.
///
/// Accepts headings with or without `#` prefix (e.g. both `"Tasks"` and `"## Tasks"` match).
/// Returns an error if no match or multiple matches are found.
pub fn find_section_by_heading<'a>(
    sections: &'a [Section],
    heading: &str,
) -> Result<&'a Section, SectionLookupError> {
    // Strip leading `#` markers and whitespace
    let needle = heading.trim_start_matches('#').trim();

    let matches: Vec<&Section> = sections
        .iter()
        .filter(|s| s.heading_text.trim() == needle)
        .collect();

    match matches.len() {
        0 => Err(SectionLookupError::NotFound {
            available: sections.iter().map(|s| s.heading_text.clone()).collect(),
        }),
        1 => Ok(matches[0]),
        n => Err(SectionLookupError::Ambiguous { count: n }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_document() {
        let sections = parse_sections("");
        assert!(sections.is_empty());
    }

    #[test]
    fn no_headings() {
        let sections = parse_sections("Just some plain text without any headings.");
        assert!(sections.is_empty());
    }

    #[test]
    fn single_heading() {
        let md = "# Title\n\nSome body text.\n";
        let sections = parse_sections(md);
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].level, 1);
        assert_eq!(sections[0].heading_text, "Title");
        assert_eq!(&md[sections[0].heading_range.clone()], "# Title\n");
        assert_eq!(md[sections[0].body_range.clone()].trim(), "Some body text.");
    }

    #[test]
    fn sibling_headings() {
        let md = "# First\n\nBody 1\n\n# Second\n\nBody 2\n";
        let sections = parse_sections(md);
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].heading_text, "First");
        assert_eq!(sections[1].heading_text, "Second");
        // First section body ends at second heading
        assert!(md[sections[0].body_range.clone()].contains("Body 1"));
        assert!(!md[sections[0].body_range.clone()].contains("Body 2"));
        // Second section body to EOF
        assert!(md[sections[1].body_range.clone()].contains("Body 2"));
    }

    #[test]
    fn nested_headings() {
        let md = "# H1\n\nH1 body\n\n## H2\n\nH2 body\n\n### H3\n\nH3 body\n";
        let sections = parse_sections(md);
        assert_eq!(sections.len(), 3);
        assert_eq!(sections[0].level, 1);
        assert_eq!(sections[1].level, 2);
        assert_eq!(sections[2].level, 3);
        // H1 body includes everything until EOF (no same-or-higher level heading)
        let h1_body = &md[sections[0].body_range.clone()];
        assert!(h1_body.contains("H1 body"));
        assert!(h1_body.contains("H2 body"));
        assert!(h1_body.contains("H3 body"));
        // H2 body includes H3 (lower level)
        let h2_body = &md[sections[1].body_range.clone()];
        assert!(h2_body.contains("H2 body"));
        assert!(h2_body.contains("H3 body"));
        // H3 body is just its own content
        let h3_body = &md[sections[2].body_range.clone()];
        assert!(h3_body.contains("H3 body"));
        assert!(!h3_body.contains("H2 body"));
    }

    #[test]
    fn code_block_headings_ignored() {
        let md = "# Real heading\n\nBody\n\n```markdown\n# Fake heading\n```\n\nMore body\n";
        let sections = parse_sections(md);
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].heading_text, "Real heading");
    }

    #[test]
    fn heading_range_includes_markers() {
        let md = "## My Section\n\nContent here.\n";
        let sections = parse_sections(md);
        assert_eq!(sections.len(), 1);
        let heading_text = &md[sections[0].heading_range.clone()];
        assert!(heading_text.starts_with("## My Section"));
    }

    #[test]
    fn body_extends_to_eof() {
        let md = "# Only heading\n\nContent goes to the end.";
        let sections = parse_sections(md);
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].body_range.end, md.len());
        assert!(md[sections[0].body_range.clone()].contains("Content goes to the end."));
    }

    #[test]
    fn find_exact_heading() {
        let md = "# Title\n\nBody\n\n## Tasks\n\nTask list\n";
        let sections = parse_sections(md);
        let found = find_section_by_heading(&sections, "Tasks").unwrap();
        assert_eq!(found.heading_text, "Tasks");
        assert_eq!(found.level, 2);
    }

    #[test]
    fn find_with_hash_prefix() {
        let md = "# Title\n\nBody\n\n## Tasks\n\nTask list\n";
        let sections = parse_sections(md);
        let found = find_section_by_heading(&sections, "## Tasks").unwrap();
        assert_eq!(found.heading_text, "Tasks");
    }

    #[test]
    fn find_not_found() {
        let md = "# Title\n\nBody\n\n## Tasks\n\nTask list\n";
        let sections = parse_sections(md);
        let err = find_section_by_heading(&sections, "Nonexistent").unwrap_err();
        match err {
            SectionLookupError::NotFound { available } => {
                assert!(available.contains(&"Title".to_string()));
                assert!(available.contains(&"Tasks".to_string()));
            }
            _ => panic!("Expected NotFound"),
        }
    }

    #[test]
    fn find_ambiguous() {
        let md = "# Title\n\n## Notes\n\nFirst\n\n## Notes\n\nSecond\n";
        let sections = parse_sections(md);
        let err = find_section_by_heading(&sections, "Notes").unwrap_err();
        match err {
            SectionLookupError::Ambiguous { count } => assert_eq!(count, 2),
            _ => panic!("Expected Ambiguous"),
        }
    }

    #[test]
    fn polish_headings() {
        let md = "# Główne tematy\n\nTreść po polsku.\n\n## Współpraca\n\nDalsze informacje.\n";
        let sections = parse_sections(md);
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].heading_text, "Główne tematy");
        assert_eq!(sections[1].heading_text, "Współpraca");
        let found = find_section_by_heading(&sections, "Współpraca").unwrap();
        assert_eq!(found.level, 2);
    }

    #[test]
    fn h2_siblings_isolate_body() {
        let md = "## A\n\nContent A\n\n## B\n\nContent B\n\n## C\n\nContent C\n";
        let sections = parse_sections(md);
        assert_eq!(sections.len(), 3);
        let a_body = &md[sections[0].body_range.clone()];
        assert!(a_body.contains("Content A"));
        assert!(!a_body.contains("Content B"));
        let b_body = &md[sections[1].body_range.clone()];
        assert!(b_body.contains("Content B"));
        assert!(!b_body.contains("Content C"));
    }

    #[test]
    fn heading_with_subsections_body_includes_them() {
        let md =
            "## Parent\n\nParent body\n\n### Child\n\nChild body\n\n## Sibling\n\nSibling body\n";
        let sections = parse_sections(md);
        assert_eq!(sections.len(), 3);
        let parent_body = &md[sections[0].body_range.clone()];
        assert!(parent_body.contains("Parent body"));
        assert!(parent_body.contains("Child body"));
        assert!(!parent_body.contains("Sibling body"));
    }
}
