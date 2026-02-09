mod chunker;
mod frontmatter;
pub mod path_validation;
pub mod sections;
pub mod transforms;
pub mod types;
pub mod vault;
pub mod wikilinks;

// Re-export public API
pub use chunker::{chunk_markdown, heading_level_to_u8};
pub use frontmatter::{
    add_tags, extract_date, extract_tags, extract_title, generate_frontmatter, remove_tags,
    strip_frontmatter, update_existing_frontmatter_field, update_frontmatter_field,
};
pub use sections::{Section, SectionLookupError, find_section_by_heading, parse_sections};
pub use transforms::{
    MatchPosition, ReplaceError, TransformError, append_content, insert_after, overwrite_body,
    prepend_content, replace_section, replace_text,
};
pub use types::{Chunk, ChunkConfig, Link, ParsedDocument};
pub use vault::{
    ShowMode, VaultFolder, VaultTreeOptions, WalkEntry, parse_vault, parse_vault_entries,
    parse_vault_entries_with_config, scan_vault, vault_tree, walk_vault,
};
pub use wikilinks::{extract_wikilinks, resolve_wikilinks_in_text};

/// Parse a markdown file into a Document + Chunks.
/// Extracts title from first H1 (or filename) and tags from YAML frontmatter.
pub fn parse_document(
    rel_path: &str,
    content: &str,
    config: &ChunkConfig,
) -> (ParsedDocument, Vec<Chunk>) {
    parse_document_with_date_fields(rel_path, content, config, None, None)
}

/// Parse a markdown file into a Document + Chunks with optional date field extraction.
pub fn parse_document_with_date_fields(
    rel_path: &str,
    content: &str,
    config: &ChunkConfig,
    created_field: Option<&str>,
    modified_field: Option<&str>,
) -> (ParsedDocument, Vec<Chunk>) {
    let (frontmatter, body) = strip_frontmatter(content);
    let title = extract_title(rel_path, body);
    let tags = extract_tags(&frontmatter);
    let frontmatter_created = created_field.and_then(|field| extract_date(&frontmatter, field));
    let frontmatter_modified = modified_field.and_then(|field| extract_date(&frontmatter, field));

    let chunks = chunk_markdown(rel_path, body, config);

    let doc = ParsedDocument {
        source_file: rel_path.to_string(),
        full_text: content.to_string(),
        title,
        tags,
        frontmatter_created,
        frontmatter_modified,
    };

    (doc, chunks)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_config() -> ChunkConfig {
        ChunkConfig {
            min_content_chars: 0,
            ..ChunkConfig::default()
        }
    }

    #[test]
    fn chunk_embed_text_format() {
        let chunk = Chunk {
            note_path: "note.md".to_string(),
            breadcrumb: "note.md > Intro".to_string(),
            content: "Hello world".to_string(),
            raw_content: "Hello world".to_string(),
            chunk_index: 0,
            links: vec![],
        };
        assert_eq!(chunk.embed_text(), "note.md > Intro\n\nHello world");
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
    fn polish_frontmatter_and_headings() {
        let md = "---\ntags: [współpraca, działalność]\n---\n# Łódź — miasto włókniarzy\n\n## Główne zabytki\n\nPałac Izraela Poznańskiego to największy pałac przemysłowca w Europie.\n";
        let chunks = chunk_markdown("łódź.md", md, &ChunkConfig::default());
        assert!(!chunks.is_empty());
        assert!(chunks[0].breadcrumb.contains("Łódź"));
        assert!(!chunks[0].content.contains("tags:"));
        assert!(chunks.iter().any(|c| c.content.contains("Poznańskiego")));
        let (doc, _) = parse_document("łódź.md", md, &ChunkConfig::default());
        assert!(doc.tags.contains(&"współpraca".to_string()));
        assert!(doc.tags.contains(&"działalność".to_string()));
        assert!(doc.title.contains("Łódź"));
    }

    #[test]
    fn parse_document_with_date_fields_extracts_created() {
        let md = "---\ncreated: 2026-02-08T12:00:00+01:00\ntags: [test]\n---\n# Note\n\nBody";
        let (doc, _) = parse_document_with_date_fields(
            "note.md",
            md,
            &default_config(),
            Some("created"),
            None,
        );
        assert!(doc.frontmatter_created.is_some());
        let ts = doc.frontmatter_created.unwrap();
        // Verify it's a reasonable timestamp
        assert!(ts > 1700000000.0 && ts < 2000000000.0);
    }

    #[test]
    fn parse_document_with_date_fields_extracts_both() {
        let md = "---\ncreated: 2026-02-08T12:00:00+01:00\nmodified: 2026-02-09T15:30:00+01:00\n---\n# Note\n\nBody";
        let (doc, _) = parse_document_with_date_fields(
            "note.md",
            md,
            &default_config(),
            Some("created"),
            Some("modified"),
        );
        assert!(doc.frontmatter_created.is_some());
        assert!(doc.frontmatter_modified.is_some());
        assert!(doc.frontmatter_modified.unwrap() > doc.frontmatter_created.unwrap());
    }

    #[test]
    fn parse_document_with_date_fields_none_when_missing() {
        let md = "---\ntags: [test]\n---\n# Note\n\nBody";
        let (doc, _) = parse_document_with_date_fields(
            "note.md",
            md,
            &default_config(),
            Some("created"),
            Some("modified"),
        );
        assert!(doc.frontmatter_created.is_none());
        assert!(doc.frontmatter_modified.is_none());
    }

    #[test]
    fn parse_document_with_date_fields_none_when_no_field_specified() {
        let md = "---\ncreated: 2026-02-08\nmodified: 2026-02-09\n---\n# Note\n\nBody";
        let (doc, _) =
            parse_document_with_date_fields("note.md", md, &default_config(), None, None);
        assert!(doc.frontmatter_created.is_none());
        assert!(doc.frontmatter_modified.is_none());
    }
}
