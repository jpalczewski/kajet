use pulldown_cmark::{Event, HeadingLevel, Parser, Tag, TagEnd};
use walkdir::WalkDir;

// ---------------------------------------------------------------------------
// Chunk
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Chunk {
    pub note_path: String,
    pub breadcrumb: String,
    pub content: String,
}

impl Chunk {
    /// Text sent to the embedder: breadcrumb + content for context.
    pub fn embed_text(&self) -> String {
        format!("{}\n\n{}", self.breadcrumb, self.content)
    }
}

// ---------------------------------------------------------------------------
// Vault parsing (filesystem)
// ---------------------------------------------------------------------------

/// Read all markdown files from `vault_path` and chunk them.
/// Folders in `exclude_folders` (e.g. `.obsidian`, `.trash`) are skipped.
pub fn parse_vault(vault_path: &str, exclude_folders: &[String]) -> anyhow::Result<Vec<Chunk>> {
    let mut entries = Vec::new();

    for entry in WalkDir::new(vault_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            let path = e.path().to_string_lossy();
            e.path().extension().map_or(false, |ext| ext == "md")
                && !exclude_folders.iter().any(|folder| path.contains(folder))
        })
    {
        let content = std::fs::read_to_string(entry.path())?;
        let rel_path = entry
            .path()
            .strip_prefix(vault_path)
            .unwrap_or(entry.path())
            .to_string_lossy()
            .to_string();

        entries.push((rel_path, content));
    }

    Ok(parse_vault_entries(&entries))
}

/// Chunk a list of `(relative_path, markdown_content)` pairs.
/// Pure function — no filesystem access.
pub fn parse_vault_entries(entries: &[(String, String)]) -> Vec<Chunk> {
    entries
        .iter()
        .flat_map(|(path, content)| chunk_markdown(path, content))
        .collect()
}

// ---------------------------------------------------------------------------
// Markdown chunker — splits by headings with breadcrumbs
// ---------------------------------------------------------------------------

pub fn chunk_markdown(note_path: &str, markdown: &str) -> Vec<Chunk> {
    let parser = Parser::new(markdown);
    let mut chunks = Vec::new();

    let mut heading_stack: Vec<(u8, String)> = Vec::new();
    let mut current_text = String::new();
    let mut in_heading = false;
    let mut current_heading_text = String::new();
    let mut current_heading_level: u8 = 0;

    for event in parser {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                flush_chunk(note_path, &heading_stack, &current_text, &mut chunks);
                current_text.clear();

                in_heading = true;
                current_heading_text.clear();
                current_heading_level = heading_level_to_u8(level);
            }
            Event::End(TagEnd::Heading(_)) => {
                in_heading = false;

                while heading_stack
                    .last()
                    .map_or(false, |(l, _)| *l >= current_heading_level)
                {
                    heading_stack.pop();
                }
                heading_stack.push((current_heading_level, current_heading_text.clone()));
            }
            Event::Text(text) | Event::Code(text) => {
                if in_heading {
                    current_heading_text.push_str(&text);
                } else {
                    current_text.push_str(&text);
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                if !in_heading {
                    current_text.push('\n');
                }
            }
            Event::End(TagEnd::Paragraph) => {
                current_text.push_str("\n\n");
            }
            _ => {}
        }
    }

    flush_chunk(note_path, &heading_stack, &current_text, &mut chunks);

    chunks
}

pub fn flush_chunk(
    note_path: &str,
    heading_stack: &[(u8, String)],
    text: &str,
    chunks: &mut Vec<Chunk>,
) {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return;
    }

    let breadcrumb = if heading_stack.is_empty() {
        note_path.to_string()
    } else {
        let path: Vec<&str> = heading_stack.iter().map(|(_, h)| h.as_str()).collect();
        format!("{} > {}", note_path, path.join(" > "))
    };

    chunks.push(Chunk {
        note_path: note_path.to_string(),
        breadcrumb,
        content: trimmed.to_string(),
    });
}

pub fn heading_level_to_u8(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use pulldown_cmark::HeadingLevel;

    #[test]
    fn heading_level_maps_correctly() {
        assert_eq!(heading_level_to_u8(HeadingLevel::H1), 1);
        assert_eq!(heading_level_to_u8(HeadingLevel::H2), 2);
        assert_eq!(heading_level_to_u8(HeadingLevel::H3), 3);
        assert_eq!(heading_level_to_u8(HeadingLevel::H4), 4);
        assert_eq!(heading_level_to_u8(HeadingLevel::H5), 5);
        assert_eq!(heading_level_to_u8(HeadingLevel::H6), 6);
    }

    #[test]
    fn flush_chunk_skips_empty_text() {
        let mut chunks = Vec::new();
        flush_chunk("note.md", &[], "", &mut chunks);
        flush_chunk("note.md", &[], "   \n  ", &mut chunks);
        assert!(chunks.is_empty());
    }

    #[test]
    fn flush_chunk_uses_note_path_as_breadcrumb_when_no_headings() {
        let mut chunks = Vec::new();
        flush_chunk("folder/note.md", &[], "Some text", &mut chunks);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].breadcrumb, "folder/note.md");
        assert_eq!(chunks[0].content, "Some text");
    }

    #[test]
    fn flush_chunk_builds_breadcrumb_from_heading_stack() {
        let mut chunks = Vec::new();
        let stack = vec![
            (1, "Title".to_string()),
            (2, "Section".to_string()),
        ];
        flush_chunk("note.md", &stack, "Content here", &mut chunks);
        assert_eq!(chunks[0].breadcrumb, "note.md > Title > Section");
    }

    #[test]
    fn chunk_embed_text_format() {
        let chunk = Chunk {
            note_path: "note.md".to_string(),
            breadcrumb: "note.md > Intro".to_string(),
            content: "Hello world".to_string(),
        };
        assert_eq!(chunk.embed_text(), "note.md > Intro\n\nHello world");
    }

    #[test]
    fn chunk_markdown_plain_text_no_headings() {
        let chunks = chunk_markdown("note.md", "Just some plain text.");
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].breadcrumb, "note.md");
        assert_eq!(chunks[0].content, "Just some plain text.");
    }

    #[test]
    fn chunk_markdown_empty_file() {
        let chunks = chunk_markdown("empty.md", "");
        assert!(chunks.is_empty());
    }

    #[test]
    fn chunk_markdown_only_headings_no_content() {
        let chunks = chunk_markdown("note.md", "# Title\n## Section\n");
        assert!(chunks.is_empty());
    }

    #[test]
    fn chunk_markdown_single_heading_with_content() {
        let md = "# Title\n\nSome content under the title.";
        let chunks = chunk_markdown("note.md", md);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].breadcrumb, "note.md > Title");
        assert_eq!(chunks[0].content, "Some content under the title.");
    }

    #[test]
    fn chunk_markdown_nested_headings() {
        let md = "\
# H1

Text under H1

## H2

Text under H2

### H3

Text under H3
";
        let chunks = chunk_markdown("note.md", md);
        assert_eq!(chunks.len(), 3);

        assert_eq!(chunks[0].breadcrumb, "note.md > H1");
        assert_eq!(chunks[0].content, "Text under H1");

        assert_eq!(chunks[1].breadcrumb, "note.md > H1 > H2");
        assert_eq!(chunks[1].content, "Text under H2");

        assert_eq!(chunks[2].breadcrumb, "note.md > H1 > H2 > H3");
        assert_eq!(chunks[2].content, "Text under H3");
    }

    #[test]
    fn chunk_markdown_heading_level_reset() {
        let md = "\
# H1

Text A

## H2

Text B

# Another H1

Text C
";
        let chunks = chunk_markdown("note.md", md);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[2].breadcrumb, "note.md > Another H1");
    }

    #[test]
    fn chunk_markdown_text_before_first_heading() {
        let md = "\
Preamble text

# Title

Body text
";
        let chunks = chunk_markdown("note.md", md);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].breadcrumb, "note.md");
        assert_eq!(chunks[0].content, "Preamble text");
        assert_eq!(chunks[1].breadcrumb, "note.md > Title");
    }

    #[test]
    fn parse_vault_entries_processes_multiple_files() {
        let entries = vec![
            ("a.md".to_string(), "# A\n\nContent A".to_string()),
            ("b.md".to_string(), "# B\n\nContent B".to_string()),
        ];
        let chunks = parse_vault_entries(&entries);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].note_path, "a.md");
        assert_eq!(chunks[1].note_path, "b.md");
    }

    #[test]
    fn parse_vault_entries_empty_input() {
        let chunks = parse_vault_entries(&[]);
        assert!(chunks.is_empty());
    }

    #[test]
    fn parse_vault_entries_filters_empty_files() {
        let entries = vec![
            ("empty.md".to_string(), "".to_string()),
            ("has_content.md".to_string(), "Hello".to_string()),
        ];
        let chunks = parse_vault_entries(&entries);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].note_path, "has_content.md");
    }
}
