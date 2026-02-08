# MCP Tool: examine

## Overview

New MCP tool that returns document metadata (title, tags, outgoing links, backlinks) and content from the LanceDB `documents` table. Assumes all documents are already indexed.

## API

### Parameters

| Parameter | Type   | Required | Default     | Description                              |
|-----------|--------|----------|-------------|------------------------------------------|
| `path`    | string | yes      | —           | Full or partial file path (fuzzy match)  |
| `content` | string | no       | `"summary"` | `"summary"` / `"full"` / `"slice"`      |
| `offset`  | number | no       | `0`         | Character offset (only for `"slice"`)    |
| `length`  | number | no       | `500`       | Character count (only for `"slice"`)     |

### Path resolution (fuzzy matching)

Priority order:
1. Exact match on `source_file`
2. Suffix match — `myfile` matches `notes/myfile.md`
3. Suffix match without extension — `myfile` matches `notes/myfile.md`
4. Multiple matches → error with candidate list for disambiguation

### Response format

Always returned (metadata):
```
📄 notes/daily/myfile.md
Title: My file
Tags: #project, #notes

Outgoing links (3):
  → other-note.md
  → references/source.md
  → ideas/concept.md

Backlinks (2):
  ← index.md
  ← projects/overview.md
```

Content (depends on mode):
- `summary` — first 500 chars of `full_text` + total length info
- `full` — entire `full_text`
- `slice` — substring from `offset` to `offset+length`, with position and total length info

Footer for `summary` and `slice`:
```
Content: 500/3847 chars (use content="full" to see everything)
```

## Implementation

### 1. New method on `DocumentStore` trait (`kajet-core/src/traits.rs`)

```rust
async fn get_document_by_path(&self, path: &str) -> Result<Option<Document>>;
```

### 2. Implementation in `LanceDocumentStore` (`kajet-backend/src/document_store.rs`)

Query `documents` table where `source_file = path`.

### 3. New method on `Engine` (`kajet-core/src/engine.rs`)

```rust
pub async fn examine(&self, path: &str) -> Result<Document>
```

Fuzzy matching logic:
1. Try exact match via `get_document_by_path(path)`
2. If no match, get all documents and filter by suffix
3. If no match, retry suffix match stripping `.md` extension
4. Single match → return Document
5. No match → error "not found"
6. Multiple matches → error with candidate list

### 4. New MCP tool (`kajet-mcp/src/lib.rs`)

- `ExamineRequest { path, content, offset, length }`
- Register in `list_tools()` with English description
- Format text response (metadata + content by mode)

### 5. i18n (`locales/en.toml`, `locales/pl.toml`)

Keys: `examine_title`, `examine_tags`, `examine_outgoing_links`, `examine_backlinks`, `examine_content_summary`, `examine_not_found`, `examine_ambiguous`

### Not changed

No changes to: `Document` type, `VectorStore`, frontend, chunks table. Uses existing `documents` table only.
