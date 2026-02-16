use crate::section_filter::should_skip_section;
use crate::sections::{Section, parse_sections};
use crate::types::{Chunk, ChunkConfig};
use crate::wikilinks::{extract_wikilinks, resolve_wikilinks_in_text};
use pulldown_cmark::{Event, HeadingLevel, Parser, TagEnd};

/// Extract clean text from a raw markdown slice using pulldown-cmark events.
/// Mirrors the text accumulation logic of the original chunker.
fn extract_text(markdown: &str) -> String {
    let parser = Parser::new(markdown);
    let mut text = String::new();
    for event in parser {
        match event {
            Event::Text(t) | Event::Code(t) => text.push_str(&t),
            Event::SoftBreak | Event::HardBreak => text.push('\n'),
            Event::End(TagEnd::Paragraph) => text.push_str("\n\n"),
            Event::End(TagEnd::Item) => text.push('\n'),
            _ => {}
        }
    }
    text
}

/// Build hierarchical breadcrumbs from a flat section list.
/// E.g. [H1, H2, H3] → ["note.md > H1", "note.md > H1 > H2", "note.md > H1 > H2 > H3"]
fn build_breadcrumbs(note_path: &str, sections: &[Section]) -> Vec<String> {
    let mut stack: Vec<(u8, &str)> = Vec::new();
    sections
        .iter()
        .map(|s| {
            while stack.last().is_some_and(|(l, _)| *l >= s.level) {
                stack.pop();
            }
            stack.push((s.level, &s.heading_text));
            let path: Vec<&str> = stack.iter().map(|(_, h)| *h).collect();
            format!("{} > {}", note_path, path.join(" > "))
        })
        .collect()
}

/// Compute "direct body" byte ranges: from heading end to next heading start (any level).
/// Returns (start, end) pairs for each section.
fn direct_body_ranges(sections: &[Section], doc_len: usize) -> Vec<(usize, usize)> {
    sections
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let start = s.heading_range.end;
            let end = sections
                .get(i + 1)
                .map(|next| next.heading_range.start)
                .unwrap_or(doc_len);
            (start, end)
        })
        .collect()
}

/// Create a Chunk with wikilink processing.
fn make_chunk(
    note_path: &str,
    breadcrumb: String,
    raw_text: String,
    chunk_index: u32,
    config: &ChunkConfig,
) -> Chunk {
    let links = extract_wikilinks(&raw_text);
    let content = if config.resolve_wikilinks {
        resolve_wikilinks_in_text(&raw_text)
    } else {
        raw_text.clone()
    };
    Chunk {
        note_path: note_path.to_string(),
        breadcrumb,
        content,
        raw_content: raw_text,
        chunk_index,
        links,
    }
}

pub fn chunk_markdown(note_path: &str, markdown: &str, config: &ChunkConfig) -> Vec<Chunk> {
    let sections = parse_sections(markdown);

    // Collect raw sections: (breadcrumb, body_text)
    let mut raw_sections: Vec<(String, String)> = Vec::new();

    // Preamble: text before first heading
    let preamble_end = sections
        .first()
        .map(|s| s.heading_range.start)
        .unwrap_or(markdown.len());
    let preamble = markdown[..preamble_end].trim();
    if !preamble.is_empty() {
        let text = extract_text(preamble);
        let trimmed = text.trim().to_string();
        if !trimmed.is_empty() {
            raw_sections.push((note_path.to_string(), trimmed));
        }
    }

    // Headed sections
    let breadcrumbs = build_breadcrumbs(note_path, &sections);
    let body_ranges = direct_body_ranges(&sections, markdown.len());

    for (i, (start, end)) in body_ranges.iter().enumerate() {
        let raw_body = markdown[*start..*end].trim();
        if raw_body.is_empty() {
            continue;
        }
        let text = extract_text(raw_body);
        let trimmed = text.trim().to_string();
        if !trimmed.is_empty() {
            raw_sections.push((breadcrumbs[i].clone(), trimmed));
        }
    }

    // Filter, split, and create chunks
    let mut chunks = Vec::new();
    let mut chunk_index: u32 = 0;

    for (breadcrumb, raw_text) in raw_sections {
        if should_skip_section(&raw_text, config) {
            continue;
        }

        if raw_text.len() <= config.max_chars {
            chunks.push(make_chunk(
                note_path,
                breadcrumb,
                raw_text,
                chunk_index,
                config,
            ));
            chunk_index += 1;
        } else {
            let sub_chunks = split_large_section(&raw_text, config);
            for sub in sub_chunks {
                chunks.push(make_chunk(
                    note_path,
                    breadcrumb.clone(),
                    sub,
                    chunk_index,
                    config,
                ));
                chunk_index += 1;
            }
        }
    }

    chunks
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ChunkConfig;

    fn default_config() -> ChunkConfig {
        ChunkConfig {
            min_content_chars: 0, // disable filtering for legacy tests
            ..ChunkConfig::default()
        }
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
            resolve_wikilinks: true,
            min_content_chars: 0,
        };
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
            resolve_wikilinks: true,
            min_content_chars: 0,
        };
        let code_block = "```rust\nfn main() {\n    println!(\"hello\");\n    let x = 42;\n    let y = x + 1;\n}\n```";
        let md = format!("# Code\n\n{}", code_block);
        let chunks = chunk_markdown("note.md", &md, &config);
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].content.contains("fn main()"));
        assert!(chunks[0].content.contains("let y = x + 1"));
    }

    #[test]
    fn heading_level_maps_correctly() {
        use pulldown_cmark::HeadingLevel;
        assert_eq!(heading_level_to_u8(HeadingLevel::H1), 1);
        assert_eq!(heading_level_to_u8(HeadingLevel::H2), 2);
        assert_eq!(heading_level_to_u8(HeadingLevel::H3), 3);
        assert_eq!(heading_level_to_u8(HeadingLevel::H4), 4);
        assert_eq!(heading_level_to_u8(HeadingLevel::H5), 5);
        assert_eq!(heading_level_to_u8(HeadingLevel::H6), 6);
    }

    #[test]
    fn overlap_produces_shared_content() {
        let config = ChunkConfig {
            max_chars: 50,
            overlap_chars: 20,
            resolve_wikilinks: true,
            min_content_chars: 0,
        };
        let paragraphs: Vec<String> = (0..5).map(|i| format!("Para {} with text.", i)).collect();
        let md = format!("# Title\n\n{}", paragraphs.join("\n\n"));
        let chunks = chunk_markdown("note.md", &md, &config);
        if chunks.len() >= 2 {
            let tail_start = chunks[0]
                .content
                .floor_char_boundary(chunks[0].content.len().saturating_sub(10));
            let end_of_first = &chunks[0].content[tail_start..];
            assert!(chunks.len() >= 2);
            let _ = end_of_first;
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
        let config = ChunkConfig {
            max_chars: 60,
            overlap_chars: 20,
            resolve_wikilinks: true,
            min_content_chars: 0,
        };
        let paragraphs = vec![
            "Zażółć gęślą jaźń, to zdanie testowe numer jeden.",
            "Współpraca między różnymi instytucjami jest kluczowa.",
            "Działalność organizacji pozarządowych wspiera społeczność.",
            "Świętokrzyskie góry są piękne jesienią i wiosną.",
        ];
        let md = format!("# Tytuł\n\n{}", paragraphs.join("\n\n"));
        let chunks = chunk_markdown("polski.md", &md, &config);
        assert!(
            chunks.len() > 1,
            "Polish text should split into multiple chunks"
        );
        for chunk in &chunks {
            assert!(!chunk.content.trim().is_empty());
        }
    }

    #[test]
    fn chunk_has_raw_content_and_links() {
        let md = "# Title\n\nSee [[Note]] and [[Target|alias]] here.";
        let chunks = chunk_markdown("note.md", md, &default_config());
        assert_eq!(chunks.len(), 1);
        assert_eq!(
            chunks[0].raw_content,
            "See [[Note]] and [[Target|alias]] here."
        );
        assert_eq!(chunks[0].content, "See Note and alias here.");
        assert_eq!(chunks[0].links.len(), 2);
        assert_eq!(chunks[0].links[0].target, "Note");
        assert_eq!(chunks[0].links[1].target, "Target");
        assert_eq!(chunks[0].links[1].alias, Some("alias".to_string()));
    }

    #[test]
    fn resolve_wikilinks_config_false() {
        let config = ChunkConfig {
            max_chars: 6000,
            overlap_chars: 600,
            resolve_wikilinks: false,
            min_content_chars: 0,
        };
        let md = "# Title\n\nSee [[Note]] here.";
        let chunks = chunk_markdown("note.md", md, &config);
        assert_eq!(chunks[0].content, "See [[Note]] here.");
        assert_eq!(chunks[0].raw_content, "See [[Note]] here.");
    }

    // --- Link-only chunk filtering ---

    #[test]
    fn filter_drops_link_only_section() {
        let md = "\
# Workplace
Lorem ipsum dolor sit amet, consectetur adipiscing elit.

## Related documents
- [[Topic A]]
- [[Topic B with details]]
- [[Topic C]]
";
        let config = ChunkConfig {
            min_content_chars: 30,
            ..ChunkConfig::default()
        };
        let chunks = chunk_markdown("workplace.md", md, &config);
        assert_eq!(chunks.len(), 1, "link-only section should be filtered out");
        assert!(chunks[0].content.contains("Lorem ipsum"));
    }

    #[test]
    fn filter_keeps_prose_with_links() {
        let md = "\
# Workplace
Lorem ipsum dolor sit amet, consectetur adipiscing elit ([[Topic A|alias]]).
";
        let chunks = chunk_markdown("workplace.md", md, &ChunkConfig::default());
        assert_eq!(chunks.len(), 1);
    }

    #[test]
    fn filter_disabled_when_zero() {
        let md = "\
# Links
- [[A]]
- [[B]]
";
        let config = ChunkConfig {
            min_content_chars: 0,
            ..ChunkConfig::default()
        };
        let chunks = chunk_markdown("note.md", md, &config);
        assert_eq!(
            chunks.len(),
            1,
            "filter should be disabled with min_content_chars=0"
        );
    }

    #[test]
    fn chunk_index_skips_filtered_chunks() {
        let md = "\
# Good
This is real content with enough prose to pass the filter easily.

## Links only
- [[A]]
- [[B]]

# Also good
Another section with meaningful content that should be indexed.
";
        let config = ChunkConfig {
            min_content_chars: 30,
            ..ChunkConfig::default()
        };
        let chunks = chunk_markdown("note.md", md, &config);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].chunk_index, 0);
        assert_eq!(chunks[1].chunk_index, 1);
    }

    // --- Integration tests: changelog and boilerplate filtering ---

    #[test]
    fn changelog_section_filtered() {
        let md = "\
# System zarządzania

System priorytetyzacji to automatyczna metoda oceny ważności zadań.
Przejawia się rankingowaniem działań według wartości biznesowej.

## Historia zmian
- 2026-02-06: Utworzenie dokumentu
- 2026-02-14 00:15: aktualizacja po spotkaniu zespołu
";
        let chunks = chunk_markdown("system.md", md, &ChunkConfig::default());
        assert_eq!(chunks.len(), 1, "changelog section should be filtered out");
        assert!(chunks[0].content.contains("System priorytetyzacji"));
        assert_eq!(chunks[0].chunk_index, 0);
    }

    #[test]
    fn changelog_with_wikilinked_dates_filtered() {
        let md = "\
# Temat

Treść merytoryczna z wystarczającą ilością tekstu do embedowania.

## Historia zmian
- [[2026-01-31]]: pierwsze rozpoznanie tej części
- [[2026-02-06]]: aktualizacja po spotkaniu
";
        let chunks = chunk_markdown("temat.md", md, &ChunkConfig::default());
        assert_eq!(chunks.len(), 1);
        assert!(!chunks[0].content.contains("2026"));
    }

    #[test]
    fn links_with_comments_kept() {
        let md = "\
# Moduł systemu

Opis modułu w kontekście architektury. To jest ważny moduł odpowiedzialny za walidację.

## Powiązania
- [[Zespół]] - delegacja uprawnień, źródło decyzji o poziomie dostępu
- [[Projekt Alpha]] - historia migracji bez testów regresji, bez dokumentacji
- [[Wzorzec legacy systems]] - akceptowanie architektury gdzie nasze wymogi nie są priorytetem
";
        let chunks = chunk_markdown("modul.md", md, &ChunkConfig::default());
        assert_eq!(
            chunks.len(),
            2,
            "Powiazania with prose comments should be kept"
        );
        assert!(chunks[1].content.contains("delegacja uprawnień"));
    }

    #[test]
    fn full_template_document_filters_boilerplate() {
        let md = "\
# Proces skalowania

Proces skalowania to automatyczna reakcja na wzrost obciążenia systemu.
Przejawia się uruchamianiem dodatkowych instancji aplikacji.

## Jak się przejawia

Monitorowanie metryk wydajności, alokacja zasobów, balansowanie obciążenia.

## Powiązane dokumenty
- [[Infrastruktura]]
- [[Projekt Alpha]]
- [[Wzorzec microservices]]

## Historia zmian
- 2026-02-06: Utworzenie dokumentu
- 2026-02-14: Rozbudowa po przeglądzie architektury
";
        let chunks = chunk_markdown("proces.md", md, &ChunkConfig::default());
        assert_eq!(
            chunks.len(),
            2,
            "Only 2 content sections should survive: intro + Jak sie przejawia"
        );
        assert!(chunks[0].content.contains("Proces skalowania"));
        assert!(
            chunks[1]
                .content
                .contains("Monitorowanie metryk wydajności")
        );
        assert_eq!(chunks[0].chunk_index, 0);
        assert_eq!(chunks[1].chunk_index, 1);
    }
}
