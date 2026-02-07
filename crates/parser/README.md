# kajet-parser

Markdown parser for Obsidian vaults. Extracts structured chunks suitable for embedding-based semantic search.

## What it does

1. **Frontmatter extraction** — strips YAML frontmatter, extracts `tags` and `title`
2. **Heading-based chunking** — splits markdown into sections by heading hierarchy, producing breadcrumb paths like `note.md > H1 > H2`
3. **Wikilink handling** — extracts `[[wikilinks]]` as structured `Link` objects, resolves `[[target|alias]]` → display text in chunk content
4. **Noise filtering** — drops chunks with insufficient prose (e.g. link-only "Related documents" sections)
5. **Vault scanning** — parallel filesystem traversal respecting `.gitignore`

## Chunking strategy

Each heading starts a new chunk. The heading hierarchy is tracked as a stack to produce breadcrumb paths:

```
# Work           → "note.md > Work"
## Projects      → "note.md > Work > Projects"
# Personal       → "note.md > Personal"  (stack resets)
```

Text before the first heading becomes a chunk with just the filename as breadcrumb.

Chunks exceeding `max_chars` are split on paragraph boundaries (preserving code blocks), with configurable overlap for context continuity.

## Noise filtering

Obsidian notes often contain sections that are pure wikilink lists ("Related documents", "See also"). These carry no semantic value after wikilink resolution — they become just lists of note names.

The chunker counts "prose characters" — non-whitespace text remaining after stripping `[[wikilinks]]` and list markers (`- `, `* `, `1. `). Chunks below `min_content_chars` (default: 50) are dropped.

See [issue #15](https://github.com/jpalczewski/kajet/issues/15) for planned improvements (heading blacklists, link-to-prose ratio, quality scoring).

## Configuration

```rust
ChunkConfig {
    max_chars: 6000,        // ~1500 tokens — max chunk size before splitting
    overlap_chars: 600,     // ~150 tokens — overlap between split chunks
    resolve_wikilinks: true, // [[Target|alias]] → "alias" in content
    min_content_chars: 50,  // minimum prose chars to keep a chunk
}
```

## Public API

| Function | Description |
|---|---|
| `parse_document(path, content, config)` | Parse markdown into `ParsedDocument` + `Vec<Chunk>` |
| `chunk_markdown(path, markdown, config)` | Chunk markdown (no frontmatter handling) |
| `strip_frontmatter(content)` | Split `---` frontmatter from body |
| `extract_title(path, body)` | Title from first H1, falling back to filename |
| `extract_tags(frontmatter)` | Tags from YAML `tags:` field |
| `extract_wikilinks(text)` | Extract `Link` structs from raw text |
| `resolve_wikilinks_in_text(text)` | Replace `[[target\|alias]]` → display text |
| `scan_vault(path)` | Parallel vault scan returning `(path, content)` pairs |
| `parse_vault_entries(entries)` | Chunk pre-loaded entries with default config |

## Key types

- **`Chunk`** — `note_path`, `breadcrumb`, `content` (resolved), `raw_content` (with wikilinks), `links`, `chunk_index`
- **`Link`** — `target`, `alias`, `resolved_path`
- **`ParsedDocument`** — `source_file`, `full_text`, `title`, `tags`
