# edit_tags MCP Tool - Design Document

**Date:** 2026-02-08
**Issue:** #35
**Status:** Approved

## Overview

Add a new MCP tool `edit_tags` that adds/removes tags from a note's frontmatter, updating both the .md file and LanceDB immediately through reindexing.

## Design Decisions

### 1. Timestamp Update Behavior
**Decision:** Always update `modified` field if `config.writer.timestamps.enabled == true`

- Tag changes are metadata edits that deserve timestamp updates
- Consistent with `edit_note` behavior
- Can be disabled globally via config
- Uses `update_existing_frontmatter_field()` - only updates if field already exists (respects notes created by Obsidian with different field names)

### 2. Database Update Strategy
**Decision:** Only reindex (like `edit_note`), no direct DocumentStore update

- Simple - single code path, delegates to existing indexer
- Guarantees consistency - everything updates together
- Fast enough - reindex of single file takes 1-2 seconds
- Avoids temporary inconsistency between `documents` and `chunks` tables

### 3. Timestamp Format
**Decision:** Use `kajet_writer::timestamp::format_now(&config.writer.timestamps)`

- Consistent with `create_note`
- Respects user preferences (iso8601, date_only, obsidian, custom strftime)
- Respects timezone config (local, UTC, IANA timezones)

### 4. Missing Frontmatter Handling
**Decision:** Create new frontmatter block with tags

- Best UX - operation succeeds regardless of file state
- Uses simple format: `---\ntags: [tag1, tag2]\n---\n`

### 5. Parameter Validation
**Decision:** Validation error if both `add` and `remove` are empty/None

- Explicit communication - prevents accidental no-op calls
- As specified in issue #35 edge cases

### 6. Implementation Approach
**Decision:** YAML parsing with serde_yaml (deserialize → modify → serialize)

- More robust - handles edge cases better than string manipulation
- Trade-off: may change field order, formatting, remove comments
- Acceptable since frontmatter is primarily machine-generated

### 7. Operation Order
**Decision:** Remove tags first, then add tags

- Enables "rename tag" in single operation: `remove: [old], add: [new]`
- Intuitive: "remove old, add new"
- Edge case: `remove: [foo], add: [foo]` → result: tag exists (add wins)

## Architecture

### Components

**kajet-parser (`frontmatter.rs`)** - Pure functions:
- `add_tags(content: &str, tags: &[String]) -> Result<String>`
- `remove_tags(content: &str, tags: &[String]) -> Result<String>`

**kajet-mcp (`schema.rs`)** - Request type:
```rust
pub struct EditTagsRequest {
    pub path: String,
    pub add: Option<Vec<String>>,
    pub remove: Option<Vec<String>>,
}
```

**kajet-mcp (`tools.rs`)** - MCP tool handler:
```rust
#[tool(description = "Edit tags in a note's frontmatter...")]
#[instrument(level = "debug", skip(self, params),
    fields(path, add_count, remove_count, timestamp_updated))]
async fn edit_tags(&self, params: Parameters<EditTagsRequest>)
    -> Result<CallToolResult, ErrorData>
```

## Flow

1. **Validation**: If `add` and `remove` both empty/None → validation error
2. **Resolve path**: Fuzzy suffix matching (like `examine` tool)
3. **Read file** from `vault_path`
4. **Remove tags** (if provided) → `remove_tags()`
5. **Add tags** (if provided) → `add_tags()`
6. **Update timestamp** (if `config.writer.timestamps.enabled`):
   - Format: `kajet_writer::timestamp::format_now(&config.writer.timestamps)`
   - Update: `update_existing_frontmatter_field()` - only if field exists
7. **Write file** back to disk
8. **Reindex** via `indexer.reindex_files()` (like `edit_note`)
9. **Return result** with formatted success message

## Implementation Details

### add_tags() Function

```rust
pub fn add_tags(content: &str, tags: &[String]) -> Result<String> {
    let (fm_text, body) = strip_frontmatter(content);

    if fm_text.is_empty() {
        // No frontmatter → create new with tags
        let new_fm = format!("---\ntags: [{}]\n---\n", tags.join(", "));
        return Ok(format!("{new_fm}{body}"));
    }

    // Deserialize YAML
    let mut fm: serde_yaml::Value = serde_yaml::from_str(&fm_text)?;

    // Get existing tags (or empty list)
    let existing = fm.get("tags")
        .and_then(|v| v.as_sequence())
        .map(|seq| seq.iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect::<Vec<_>>())
        .unwrap_or_default();

    // Merge + deduplicate
    let mut merged = existing;
    for tag in tags {
        if !merged.contains(tag) {
            merged.push(tag.clone());
        }
    }

    // Update YAML structure
    fm["tags"] = serde_yaml::Value::Sequence(
        merged.into_iter().map(serde_yaml::Value::String).collect()
    );

    // Serialize back
    let new_fm = serde_yaml::to_string(&fm)?;
    Ok(format!("---\n{new_fm}---\n{body}"))
}
```

### remove_tags() Function

Similar to `add_tags()` but filters out specified tags instead of adding them.

### MCP Handler Pseudo-code

```rust
async fn edit_tags(params) -> Result<...> {
    // Validation
    if add.is_empty() && remove.is_empty() {
        return Err(validation_error);
    }

    // Resolve path (fuzzy)
    let resolved = resolve_note_path(&vault_path, &req.path)?;

    // Read
    let content = read_to_string(&resolved).await?;

    // Apply operations (remove first, then add)
    let mut modified = content;
    if let Some(remove) = req.remove {
        modified = remove_tags(&modified, &remove)?;
    }
    if let Some(add) = req.add {
        modified = add_tags(&modified, &add)?;
    }

    // Update timestamp if enabled
    let config = state.config.read().unwrap();
    if config.writer.timestamps.enabled {
        let ts = format_now(&config.writer.timestamps)?;
        modified = update_existing_frontmatter_field(
            &modified,
            &config.writer.timestamps.modified_field,
            &ts
        );
    }

    // Write
    write(&resolved, &modified).await?;

    // Reindex
    indexer.reindex_files(&vault_path, &[rel_path]).await?;

    // Log & return
    tracing::info!("Tags edited successfully");
    Ok(success_result)
}
```

## Logging

**Instrumentation:**
```rust
#[instrument(level = "debug", skip(self, params),
    fields(path, add_count, remove_count, timestamp_updated))]
```

**Log points:**
- **DEBUG** (instrument): Entry point with parameters
- **INFO**: Success with summary of changes
- **WARN**: Reindex failure (non-fatal, like `edit_note`)

**Example logs:**
```
DEBUG edit_tags: path="daily/2026-02-08.md" add_count=2 remove_count=1
INFO  Tags edited successfully path="daily/2026-02-08.md" added=2 removed=1
```

## i18n

**Keys in `locales/en.toml`:**
```toml
edit_tags_empty_params = "At least one of 'add' or 'remove' must be provided"
edit_tags_path_not_found = "Note not found: {path}"
edit_tags_success = "Tags updated in {path}"
edit_tags_added = "Added: {tags}"
edit_tags_removed = "Removed: {tags}"
```

**Keys in `locales/pl.toml`:**
```toml
edit_tags_empty_params = "Musisz podać 'add' lub 'remove'"
edit_tags_path_not_found = "Nie znaleziono notatki: {path}"
edit_tags_success = "Zaktualizowano tagi w {path}"
edit_tags_added = "Dodano: {tags}"
edit_tags_removed = "Usunięto: {tags}"
```

## Testing

**Tests in `kajet-parser/frontmatter.rs`:**

- `test_add_tags_to_empty_frontmatter()` - Creates new frontmatter block
- `test_add_tags_deduplicate()` - Adding existing tag → skip (no duplicate)
- `test_remove_tags_nonexistent()` - Removing non-existent tag → skip (no error)
- `test_add_and_remove_same_tag()` - remove then add → tag exists (add wins)
- `test_remove_all_tags()` - `tags` field remains as empty list `[]`
- `test_add_tags_yaml_error()` - Invalid YAML → propagate error
- `test_remove_tags_preserves_other_fields()` - Other frontmatter fields unchanged

**Integration test in `kajet-mcp`:**

- Round-trip: create note → edit_tags → verify file content and DB state

## Edge Cases

| Case | Behavior |
|------|----------|
| Adding existing tag | Skip (deduplicate) - no duplicate tags |
| Removing non-existent tag | Skip (no error) |
| Empty `add` and `remove` | Validation error |
| No frontmatter | Create new `---\ntags: [...]\n---` block |
| YAML parsing error | Propagate error with helpful message |
| Concurrent file edits | File write may fail → return error |
| Fuzzy match returns multiple files | `resolve_note_path` returns error (already handled) |
| Timestamp format error | Propagate error from `format_now()` |
| Reindex failure | Log warning (non-fatal, user sees success for file edit) |

## Output Format

```
✓ Tags updated in path/to/note.md
  Added: tag1, tag2
  Removed: old-tag
```

## Dependencies

- **serde_yaml** (already in workspace) - for frontmatter parsing
- **kajet_writer::timestamp::format_now** - for timestamp formatting
- **kajet_writer::resolve::resolve_note_path** - for fuzzy path matching
- No new external dependencies required

## Migration Notes

- No database schema changes required - `tags` column already exists as JSON array
- No config changes required - uses existing `writer.timestamps` config
- Fully backward compatible

## Success Criteria

1. ✅ Can add tags to note with existing frontmatter
2. ✅ Can add tags to note without frontmatter (creates new block)
3. ✅ Can remove tags from note
4. ✅ Can add and remove in single operation (rename use case)
5. ✅ Duplicates are prevented
6. ✅ Timestamp updated if config enabled and field exists
7. ✅ Database reflects changes after reindex
8. ✅ Fuzzy path matching works
9. ✅ Validation error on empty parameters
10. ✅ i18n support for all messages
