# MCP Filters Extraction Design

## Problem

Business logic in `crates/mcp/src/tools.rs` is interleaved with MCP layer concerns, making it untestable without full MCP runtime. Key logic with 0 test coverage:
- Search result post-filtering (folder, date range, tags)
- Browse mode tag filtering
- Tag aggregation for `list_tags`
- `tags_match()` helper

## Solution

Extract pure functions to `crates/mcp/src/filters.rs`. No new crates, no signature changes to tool methods.

## New file: `crates/mcp/src/filters.rs`

### Functions

```rust
use kajet_core::search::SearchResult;
use kajet_core::types::Document;
use std::collections::HashMap;

pub struct TagStats {
    pub tag: String,
    pub files: Vec<String>,
}

/// Filter search results by folder, date range, and tags.
/// Mutates `results` in place, truncating to `limit`.
pub fn filter_search_results(
    results: &mut Vec<SearchResult>,
    docs: &[Document],
    folder: Option<&str>,
    from_ts: Option<f64>,
    to_ts: Option<f64>,
    tags: Option<&[String]>,
    limit: usize,
)

/// Filter browse-mode documents by tags, truncate to limit.
pub fn filter_browse_results(
    docs: &mut Vec<Document>,
    tags: Option<&[String]>,
    limit: usize,
)

/// Check if document tags contain all required tags (case-insensitive, # prefix ignored).
/// Moved from tools.rs.
pub fn tags_match(doc_tags: &[String], required: &[String]) -> bool

/// Aggregate tags from documents, optionally filtering by folder.
/// Returns sorted: count desc, then name asc.
pub fn aggregate_tags(
    docs: &[Document],
    folder: Option<&str>,
    recursive: bool,
) -> Vec<TagStats>
```

### Tests (~8-10)

- `tags_match`: case insensitivity, `#` prefix normalization, empty required, partial match fails
- `filter_search_results`: folder prefix (with/without trailing `/`), date range (from only, to only, both), tags, doc not in map → filtered out, limit applied after filtering
- `filter_browse_results`: tags + limit interaction
- `aggregate_tags`: folder filtering recursive vs non-recursive, sort order, docs without tags, same tag in multiple docs, empty folder = all docs

## Changes to existing files

### `crates/mcp/src/lib.rs`
- Add `mod filters;`

### `crates/mcp/src/tools.rs`
- `search()` lines 179-237: replace inline filtering with `filters::filter_search_results()`
- `search()` browse branch lines 287-292: replace with `filters::filter_browse_results()`
- `list_tags()` lines 566-608: replace with `filters::aggregate_tags()`
- Move `tags_match()` to `filters.rs`, remove from `tools.rs`

## Not in scope

- No changes to tool method signatures
- No changes to format.rs, date_parser.rs, schema.rs
- No changes to any other crate (core, backend, parser, indexer, writer, web)
- No new crate
