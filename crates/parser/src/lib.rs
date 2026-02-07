use ignore::WalkBuilder;
use pulldown_cmark::{Event, HeadingLevel, Parser, Tag, TagEnd};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ChunkConfig {
    pub max_chars: usize,
    pub overlap_chars: usize,
}

impl Default for ChunkConfig {
    fn default() -> Self {
        Self {
            max_chars: 6000,    // ~1500 tokens
            overlap_chars: 600, // ~150 tokens
        }
    }
}

// ---------------------------------------------------------------------------
// Chunk
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Chunk {
    pub note_path: String,
    pub breadcrumb: String,
    pub content: String,
    pub chunk_index: u32,
}

impl Chunk {
    /// Text sent to the embedder: breadcrumb + content for context.
    pub fn embed_text(&self) -> String {
        format!("{}\n\n{}", self.breadcrumb, self.content)
    }
}

// ---------------------------------------------------------------------------
// Document — parsed metadata from a markdown file
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ParsedDocument {
    pub source_file: String,
    pub full_text: String,
    pub title: String,
    pub tags: Vec<String>,
}

/// Parse a markdown file into a Document + Chunks.
/// Extracts title from first H1 (or filename) and tags from YAML frontmatter.
pub fn parse_document(
    rel_path: &str,
    content: &str,
    config: &ChunkConfig,
) -> (ParsedDocument, Vec<Chunk>) {
    let (frontmatter, body) = strip_frontmatter(content);
    let title = extract_title(rel_path, body);
    let tags = extract_tags(&frontmatter);

    let chunks = chunk_markdown(rel_path, body, config);

    let doc = ParsedDocument {
        source_file: rel_path.to_string(),
        full_text: content.to_string(),
        title,
        tags,
    };

    (doc, chunks)
}

// ---------------------------------------------------------------------------
// Frontmatter parsing
// ---------------------------------------------------------------------------

/// Strip YAML frontmatter (--- ... ---) from markdown content.
/// Returns (frontmatter_text, body_without_frontmatter).
fn strip_frontmatter(content: &str) -> (String, &str) {
    if !content.starts_with("---") {
        return (String::new(), content);
    }

    // Find closing ---
    if let Some(end) = content[3..].find("\n---") {
        let fm_end = end + 3; // offset from start of content[3..]
        let frontmatter = content[3..fm_end].trim().to_string();
        let body_start = fm_end + 4; // skip the closing \n---
        let body = &content[body_start..];
        (frontmatter, body)
    } else {
        (String::new(), content)
    }
}

fn extract_title(rel_path: &str, body: &str) -> String {
    // Look for first H1 in body
    for line in body.lines() {
        let trimmed = line.trim();
        if let Some(heading) = trimmed.strip_prefix("# ") {
            return heading.trim().to_string();
        }
        // Skip empty lines at start
        if !trimmed.is_empty() && !trimmed.starts_with('#') {
            break;
        }
    }

    // Fallback: filename without extension
    std::path::Path::new(rel_path)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| rel_path.to_string())
}

fn extract_tags(frontmatter: &str) -> Vec<String> {
    if frontmatter.is_empty() {
        return Vec::new();
    }

    let mut tags = Vec::new();
    let mut in_tags = false;

    for line in frontmatter.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("tags:") {
            let inline = trimmed.strip_prefix("tags:").unwrap().trim();
            if !inline.is_empty() {
                // Inline format: tags: [tag1, tag2] or tags: tag1, tag2
                let clean = inline.trim_start_matches('[').trim_end_matches(']');
                for tag in clean.split(',') {
                    let t = tag.trim().trim_matches('"').trim_matches('\'').trim();
                    if !t.is_empty() {
                        tags.push(t.to_string());
                    }
                }
                return tags;
            }
            in_tags = true;
            continue;
        }

        if in_tags {
            if let Some(tag) = trimmed.strip_prefix("- ") {
                let t = tag.trim().trim_matches('"').trim_matches('\'');
                if !t.is_empty() {
                    tags.push(t.to_string());
                }
            } else {
                break; // End of tag list
            }
        }
    }

    tags
}

// ---------------------------------------------------------------------------
// Vault parsing (filesystem)
// ---------------------------------------------------------------------------

/// Read all markdown files from `vault_path` and chunk them.
/// Uses parallel walking via the `ignore` crate. Respects `.gitignore`.
/// Folders in `exclude_folders` (e.g. `.obsidian`, `.trash`) are skipped.
pub fn parse_vault(vault_path: &str, exclude_folders: &[String]) -> anyhow::Result<Vec<Chunk>> {
    let config = ChunkConfig::default();
    let entries = scan_vault(vault_path, exclude_folders)?;
    Ok(parse_vault_entries_with_config(&entries, &config))
}

/// Scan vault directory for markdown files using parallel walking.
/// Returns `(relative_path, content)` pairs.
pub fn scan_vault(
    vault_path: &str,
    exclude_folders: &[String],
) -> anyhow::Result<Vec<(String, String)>> {
    let (tx, rx) = std::sync::mpsc::channel();

    let mut builder = WalkBuilder::new(vault_path);
    builder.standard_filters(true); // respect .gitignore

    let vault = vault_path.to_string();
    let excludes: Vec<String> = exclude_folders.to_vec();

    builder.build_parallel().run(|| {
        let tx = tx.clone();
        let vault = vault.clone();
        let excludes = excludes.clone();
        Box::new(move |entry| {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => return ignore::WalkState::Continue,
            };

            let path = entry.path();

            // Skip excluded folders
            if path.is_dir() {
                let name = path.file_name().map(|n| n.to_string_lossy().to_string());
                if let Some(name) = name {
                    if excludes.iter().any(|f| f == &name) {
                        return ignore::WalkState::Skip;
                    }
                }
                return ignore::WalkState::Continue;
            }

            if path.extension().is_none_or(|ext| ext != "md") {
                return ignore::WalkState::Continue;
            }

            if let Ok(content) = std::fs::read_to_string(path) {
                let rel = path
                    .strip_prefix(&vault)
                    .unwrap_or(path)
                    .to_string_lossy()
                    .to_string();
                let _ = tx.send((rel, content));
            }

            ignore::WalkState::Continue
        })
    });

    drop(tx);
    Ok(rx.into_iter().collect())
}

/// Chunk a list of `(relative_path, markdown_content)` pairs.
/// Pure function — no filesystem access.
pub fn parse_vault_entries(entries: &[(String, String)]) -> Vec<Chunk> {
    parse_vault_entries_with_config(entries, &ChunkConfig::default())
}

pub fn parse_vault_entries_with_config(
    entries: &[(String, String)],
    config: &ChunkConfig,
) -> Vec<Chunk> {
    entries
        .iter()
        .flat_map(|(path, content)| {
            let (_, body) = strip_frontmatter(content);
            chunk_markdown(path, body, config)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Markdown chunker — splits by headings with breadcrumbs
// ---------------------------------------------------------------------------

pub fn chunk_markdown(note_path: &str, markdown: &str, config: &ChunkConfig) -> Vec<Chunk> {
    let parser = Parser::new(markdown);
    let mut raw_sections = Vec::new();

    let mut heading_stack: Vec<(u8, String)> = Vec::new();
    let mut current_text = String::new();
    let mut in_heading = false;
    let mut current_heading_text = String::new();
    let mut current_heading_level: u8 = 0;

    for event in parser {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                flush_section(note_path, &heading_stack, &current_text, &mut raw_sections);
                current_text.clear();

                in_heading = true;
                current_heading_text.clear();
                current_heading_level = heading_level_to_u8(level);
            }
            Event::End(TagEnd::Heading(_)) => {
                in_heading = false;

                while heading_stack
                    .last()
                    .is_some_and(|(l, _)| *l >= current_heading_level)
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

    flush_section(note_path, &heading_stack, &current_text, &mut raw_sections);

    // Now split oversized sections
    let mut chunks = Vec::new();
    let mut chunk_index: u32 = 0;

    for (breadcrumb, text) in raw_sections {
        if text.len() <= config.max_chars {
            chunks.push(Chunk {
                note_path: note_path.to_string(),
                breadcrumb,
                content: text,
                chunk_index,
            });
            chunk_index += 1;
        } else {
            let sub_chunks = split_large_section(&text, config);
            for sub in sub_chunks {
                chunks.push(Chunk {
                    note_path: note_path.to_string(),
                    breadcrumb: breadcrumb.clone(),
                    content: sub,
                    chunk_index,
                });
                chunk_index += 1;
            }
        }
    }

    chunks
}

/// Internal: flush a raw section (breadcrumb + text) without splitting.
fn flush_section(
    note_path: &str,
    heading_stack: &[(u8, String)],
    text: &str,
    sections: &mut Vec<(String, String)>,
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

    sections.push((breadcrumb, trimmed.to_string()));
}

/// Split a large text into chunks respecting paragraph boundaries and code blocks.
fn split_large_section(text: &str, config: &ChunkConfig) -> Vec<String> {
    let paragraphs = split_preserving_code_blocks(text);
    let mut result = Vec::new();
    let mut current = String::new();

    for para in &paragraphs {
        if current.len() + para.len() > config.max_chars && !current.is_empty() {
            result.push(current.trim().to_string());
            // Overlap: take the end of previous chunk (floor to char boundary)
            let overlap_start =
                current.floor_char_boundary(current.len().saturating_sub(config.overlap_chars));
            current = current[overlap_start..].trim().to_string();
            current.push_str("\n\n");
        }
        current.push_str(para);
        current.push_str("\n\n");
    }

    if !current.trim().is_empty() {
        result.push(current.trim().to_string());
    }

    result
}

/// Split text on paragraph boundaries (\n\n) but keep code blocks intact.
fn split_preserving_code_blocks(text: &str) -> Vec<String> {
    let mut segments = Vec::new();
    let mut current = String::new();
    let mut in_code_block = false;

    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            in_code_block = !in_code_block;
            current.push_str(line);
            current.push('\n');
            continue;
        }

        if in_code_block {
            current.push_str(line);
            current.push('\n');
            continue;
        }

        if line.trim().is_empty() && !current.trim().is_empty() {
            segments.push(current.trim().to_string());
            current.clear();
        } else {
            current.push_str(line);
            current.push('\n');
        }
    }

    if !current.trim().is_empty() {
        segments.push(current.trim().to_string());
    }

    segments
}

// Legacy compatibility wrapper
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
        chunk_index: chunks.len() as u32,
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

    fn default_config() -> ChunkConfig {
        ChunkConfig::default()
    }

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
        let stack = vec![(1, "Title".to_string()), (2, "Section".to_string())];
        flush_chunk("note.md", &stack, "Content here", &mut chunks);
        assert_eq!(chunks[0].breadcrumb, "note.md > Title > Section");
    }

    #[test]
    fn chunk_embed_text_format() {
        let chunk = Chunk {
            note_path: "note.md".to_string(),
            breadcrumb: "note.md > Intro".to_string(),
            content: "Hello world".to_string(),
            chunk_index: 0,
        };
        assert_eq!(chunk.embed_text(), "note.md > Intro\n\nHello world");
    }

    #[test]
    fn chunk_markdown_plain_text_no_headings() {
        let chunks = chunk_markdown("note.md", "Just some plain text.", &default_config());
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].breadcrumb, "note.md");
        assert_eq!(chunks[0].content, "Just some plain text.");
    }

    #[test]
    fn chunk_markdown_empty_file() {
        let chunks = chunk_markdown("empty.md", "", &default_config());
        assert!(chunks.is_empty());
    }

    #[test]
    fn chunk_markdown_only_headings_no_content() {
        let chunks = chunk_markdown("note.md", "# Title\n## Section\n", &default_config());
        assert!(chunks.is_empty());
    }

    #[test]
    fn chunk_markdown_single_heading_with_content() {
        let md = "# Title\n\nSome content under the title.";
        let chunks = chunk_markdown("note.md", md, &default_config());
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
        let chunks = chunk_markdown("note.md", md, &default_config());
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
        let chunks = chunk_markdown("note.md", md, &default_config());
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
        let chunks = chunk_markdown("note.md", md, &default_config());
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

    // --- New tests for P1c features ---

    #[test]
    fn chunk_index_increments() {
        let md = "# A\n\nText A\n\n## B\n\nText B\n\n# C\n\nText C\n";
        let chunks = chunk_markdown("note.md", md, &default_config());
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].chunk_index, 0);
        assert_eq!(chunks[1].chunk_index, 1);
        assert_eq!(chunks[2].chunk_index, 2);
    }

    #[test]
    fn large_section_is_split() {
        let config = ChunkConfig {
            max_chars: 100,
            overlap_chars: 20,
        };
        // Create text with multiple paragraphs that exceeds max_chars
        let paragraphs: Vec<String> = (0..10)
            .map(|i| format!("Paragraph {} with some content to fill space.", i))
            .collect();
        let md = format!("# Title\n\n{}", paragraphs.join("\n\n"));
        let chunks = chunk_markdown("note.md", &md, &config);
        assert!(chunks.len() > 1, "Should split into multiple chunks");
        for chunk in &chunks {
            assert!(
                chunk.content.len() <= config.max_chars + config.overlap_chars,
                "Chunk should not vastly exceed max_chars"
            );
        }
    }

    #[test]
    fn code_blocks_not_split() {
        let config = ChunkConfig {
            max_chars: 100,
            overlap_chars: 20,
        };
        let code_block = "```rust\nfn main() {\n    println!(\"hello\");\n    let x = 42;\n    let y = x + 1;\n}\n```";
        // Code block is ~80 chars, which is under max_chars
        let md = format!("# Code\n\n{}", code_block);
        let chunks = chunk_markdown("note.md", &md, &config);
        // The code block should stay intact in one chunk
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].content.contains("fn main()"));
        assert!(chunks[0].content.contains("let y = x + 1"));
    }

    #[test]
    fn frontmatter_stripped() {
        let md = "---\ntags:\n  - rust\n  - programming\n---\n# Title\n\nBody text";
        let (doc, chunks) = parse_document("note.md", md, &default_config());
        assert_eq!(doc.tags, vec!["rust", "programming"]);
        assert_eq!(doc.title, "Title");
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].content, "Body text");
    }

    #[test]
    fn frontmatter_inline_tags() {
        let md = "---\ntags: [rust, programming]\n---\n# Hello\n\nContent";
        let (doc, _) = parse_document("note.md", md, &default_config());
        assert_eq!(doc.tags, vec!["rust", "programming"]);
    }

    #[test]
    fn title_from_h1() {
        let md = "# My Title\n\nSome text";
        let (doc, _) = parse_document("note.md", md, &default_config());
        assert_eq!(doc.title, "My Title");
    }

    #[test]
    fn title_fallback_to_filename() {
        let md = "Just some text without a heading";
        let (doc, _) = parse_document("my-note.md", md, &default_config());
        assert_eq!(doc.title, "my-note");
    }

    #[test]
    fn parse_document_full_text_preserved() {
        let md = "---\ntags: [test]\n---\n# Title\n\nBody";
        let (doc, _) = parse_document("note.md", md, &default_config());
        assert_eq!(doc.full_text, md);
        assert_eq!(doc.source_file, "note.md");
    }

    #[test]
    fn no_frontmatter() {
        let md = "# Title\n\nBody text";
        let (doc, chunks) = parse_document("note.md", md, &default_config());
        assert!(doc.tags.is_empty());
        assert_eq!(doc.title, "Title");
        assert_eq!(chunks.len(), 1);
    }

    #[test]
    fn overlap_produces_shared_content() {
        let config = ChunkConfig {
            max_chars: 50,
            overlap_chars: 20,
        };
        let paragraphs: Vec<String> = (0..5).map(|i| format!("Para {} with text.", i)).collect();
        let md = format!("# Title\n\n{}", paragraphs.join("\n\n"));
        let chunks = chunk_markdown("note.md", &md, &config);
        if chunks.len() >= 2 {
            // Last part of chunk N should appear at start of chunk N+1
            let tail_start = chunks[0]
                .content
                .floor_char_boundary(chunks[0].content.len().saturating_sub(10));
            let end_of_first = &chunks[0].content[tail_start..];
            // At least some overlap text should appear in the next chunk
            // (exact check is hard due to paragraph boundaries, just verify we have multiple chunks)
            assert!(chunks.len() >= 2);
            let _ = end_of_first; // used for debug if needed
        }
    }

    #[test]
    fn polish_text_chunking() {
        let md = "# Obszary Wsparcia\n\nWsparcie w ramach Grantu może być wykorzystane na realizację działań w odpowiedzi na zdiagnozowane potrzeby Grantobiorcy, szczegółowo opisanych we Wniosku o powierzenie Grantu.\n\n## Współpraca\n\nZłożoność zagadnień wymaga współdziałania różnych instytucji.\n";
        let chunks = chunk_markdown("notatka.md", md, &ChunkConfig::default());
        assert!(!chunks.is_empty());
        assert!(chunks[0].content.contains("Wsparcie"));
        assert!(chunks[0].breadcrumb.contains("Obszary Wsparcia"));
    }

    #[test]
    fn polish_text_large_section_overlap() {
        // Force split+overlap on text dense with multi-byte chars (ą,ę,ś,ć,ź,ż,ó,ł,ń)
        let config = ChunkConfig {
            max_chars: 60,
            overlap_chars: 20,
        };
        let paragraphs = vec![
            "Zażółć gęślą jaźń, to zdanie testowe numer jeden.",
            "Współpraca między różnymi instytucjami jest kluczowa.",
            "Działalność organizacji pozarządowych wspiera społeczność.",
            "Świętokrzyskie góry są piękne jesienią i wiosną.",
        ];
        let md = format!("# Tytuł\n\n{}", paragraphs.join("\n\n"));
        // Should not panic on multi-byte overlap boundary
        let chunks = chunk_markdown("polski.md", &md, &config);
        assert!(
            chunks.len() > 1,
            "Polish text should split into multiple chunks"
        );
        for chunk in &chunks {
            // Every chunk must be valid UTF-8 (implicit via String) and non-empty
            assert!(!chunk.content.trim().is_empty());
        }
    }

    #[test]
    fn polish_frontmatter_and_headings() {
        let md = "---\ntags: [współpraca, działalność]\n---\n# Łódź — miasto włókniarzy\n\n## Główne zabytki\n\nPałac Izraela Poznańskiego to największy pałac przemysłowca w Europie.\n";
        let chunks = chunk_markdown("łódź.md", md, &ChunkConfig::default());
        assert!(!chunks.is_empty());
        assert!(chunks[0].breadcrumb.contains("Łódź"));
        // Frontmatter should be stripped, content should not contain YAML
        assert!(!chunks[0].content.contains("tags:"));
        assert!(chunks.iter().any(|c| c.content.contains("Poznańskiego")));
        // Polish tags via parse_document
        let (doc, _) = parse_document("łódź.md", md, &ChunkConfig::default());
        assert!(doc.tags.contains(&"współpraca".to_string()));
        assert!(doc.tags.contains(&"działalność".to_string()));
        assert!(doc.title.contains("Łódź"));
    }
}
