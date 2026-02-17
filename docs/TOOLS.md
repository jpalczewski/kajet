# MCP Tools Reference

Complete documentation for all kajet MCP tools.

## Search & Discovery

### search

Semantic search over the vault using hybrid vector + full-text search.

| Param | Type | Default | Description |
|-------|------|---------|-------------|
| `query` | string | **required** | Search query |
| `limit` | number | `5` | Max results (1-50) |
| `mode` | string | `"hybrid"` | Search mode: `"hybrid"` (vector + FTS), `"vector"` (semantic only), `"fts"` (full-text only) |

**Example:**
```json
{
  "query": "productivity and note-taking",
  "limit": 10,
  "mode": "hybrid"
}
```

### find_similar

Find semantically similar notes to a source note using multi-vector chunk matching. Supports threshold, aggregation mode (max/avg), sort mode, optional exclude_linked mode (none/outgoing/both), and filters (from/to/tags/folder).

| Param | Type | Default | Description |
|-------|------|---------|-------------|
| `path` | string | **required** | Source note path — full relative path or partial (filename). Fuzzy suffix matching is used if exact match fails. |
| `limit` | number | *(config)* | Maximum number of similar notes to return |
| `threshold` | number | *(config)* | Minimum similarity threshold in 0..1 range |
| `aggregation` | string | `"max"` | Aggregation mode: `"max"` (best chunk match) or `"avg"` (average best match per source chunk) |
| `sort` | string | `"similarity"` | Sort mode: `"similarity"`, `"recent"` (newest notes first), or `"path"` |
| `exclude_linked` | string | `"outgoing"` | Exclude linked notes: `"outgoing"` (hides notes linked from source), `"both"` (also hides backlinks), or `"none"` |
| `tags` | array | *(optional)* | Filter by tags. Documents must have ALL specified tags. |
| `from` | string | *(optional)* | Date filter start. Formats: ISO (`2025-01-15`, `2025-01`), Polish/English keywords like `dzisiaj`, `wczoraj`, `today`, `last week` |
| `to` | string | *(optional)* | Date filter end. Same formats as `from` |
| `folder` | string | *(optional)* | Filter by folder path prefix, e.g., `journal/2025`. Matches documents in this folder and subfolders. |

**Example:**
```json
{
  "path": "journal/2025-02-15.md",
  "limit": 10,
  "threshold": 0.7,
  "aggregation": "max",
  "sort": "similarity",
  "exclude_linked": "outgoing",
  "folder": "journal/2025"
}
```

### explore_connections

Explore note connections as a traversal tree. Supports depth/limit, deduplication, optional section context, and filter modes ('display' or 'traverse').

| Param | Type | Default | Description |
|-------|------|---------|-------------|
| `path` | string | **required** | Starting note path — full relative path or partial (filename). Fuzzy suffix matching is used if exact match fails. |
| `depth` | number | *(config)* | Maximum traversal depth |
| `limit` | number | *(config)* | Maximum total nodes in output |
| `dedup` | boolean | `true` | Deduplicate nodes. `false` shows repeated nodes. |
| `include_context` | boolean | `false` | Include section context where the link appears |
| `filter_mode` | string | `"display"` | Filter mode: `"display"` (traverse full graph, filter shown nodes) or `"traverse"` (only traverse matching nodes) |
| `tags` | array | *(optional)* | Filter by tags. Documents must have ALL specified tags. |
| `from` | string | *(optional)* | Date filter start. Formats: ISO (`2025-01-15`, `2025-01`), Polish/English keywords like `dzisiaj`, `wczoraj`, `today`, `last week` |
| `to` | string | *(optional)* | Date filter end. Same formats as `from` |
| `folder` | string | *(optional)* | Filter by folder path prefix, e.g., `journal/2025`. Matches documents in this folder and subfolders. |

**Example:**
```json
{
  "path": "projects/kajet.md",
  "depth": 3,
  "limit": 50,
  "dedup": true,
  "include_context": true,
  "filter_mode": "display",
  "folder": "projects"
}
```

### recent_context

Get a compact overview of recent journaling activity: recent entries, top tags, writing frequency, gaps, and newly appearing tags.

| Param | Type | Default | Description |
|-------|------|---------|-------------|
| `days` | number | `7` | Number of days to include in the analysis window |
| `limit` | number | `10` | Maximum number of recent entries to display |

**Example:**
```json
{
  "days": 14,
  "limit": 20
}
```

## Documents

### examine

View a document's metadata and content. Accepts full or partial file paths with fuzzy matching.

| Param | Type | Default | Description |
|-------|------|---------|-------------|
| `path` | string | **required** | Full or partial file path |
| `content` | string | `"summary"` | Content mode: `"summary"` (first 500 chars), `"full"`, or `"slice"` |
| `offset` | number | `0` | Character offset for `"slice"` mode |
| `length` | number | `500` | Number of characters for `"slice"` mode |

**Example:**
```json
{
  "path": "journal/2025-02-15.md",
  "content": "full"
}
```

### tree

Show vault folder structure as a tree. Returns folder names with note counts. Use `path` to focus on a subfolder, `show: files` to include filenames.

| Param | Type | Default | Description |
|-------|------|---------|-------------|
| `path` | string | *(optional)* | Starting folder path relative to vault root. Omit for full vault. |
| `depth` | number | *(config)* | Maximum folder depth to show (typically 3) |
| `size` | number | *(config)* | Maximum total entries (folders + files) in output (typically 50) |
| `show` | string | `"folders"` | Display mode: `"folders"` (folder names + note counts) or `"files"` (include .md filenames) |

**Example:**
```json
{
  "path": "journal",
  "depth": 2,
  "size": 100,
  "show": "files"
}
```

## Notes

### create_note

Create a new note with automatic frontmatter generation.

| Param | Type | Description |
|-------|------|-------------|
| `target` | string | Relative path for the new note (e.g., `"Projects/ideas.md"`) |
| `content` | string | Markdown body content |
| `tags` | array | *(optional)* Tags for frontmatter |
| `aliases` | array | *(optional)* Aliases for Obsidian linking |

**Example:**
```json
{
  "target": "projects/new-idea.md",
  "content": "# New Idea\n\nThis is a promising concept...",
  "tags": ["projects", "ideas"],
  "aliases": ["The Big Idea"]
}
```

### edit_note

Edit an existing note with multiple modes (append, prepend, replace, etc.).

| Param | Type | Description |
|-------|------|-------------|
| `path` | string | Full or partial file path |
| `content` | string | New content to insert or replace with |
| `mode` | string | Edit mode: `"append"`, `"prepend"`, `"overwrite"`, `"replace_section"`, `"replace_text"`, `"insert_after"` |
| `target_heading` | string | *(optional)* Target heading for section operations |
| `old_text` | string | *(optional)* Exact text to replace (required for `replace_text`) or anchor for `insert_after` |

**Example:**
```json
{
  "path": "journal/2025-02-15.md",
  "content": "\n## Evening reflection\n\nToday was productive...",
  "mode": "append"
}
```

## Tags

### list_tags

List all unique tags from the vault with optional filtering.

| Param | Type | Default | Description |
|-------|------|---------|-------------|
| `detail` | string | `"counts"` | Detail level: `"names"`, `"counts"` (tag + doc count), `"full"` (tag + count + paths) |
| `folder` | string | *(optional)* | Filter by folder path prefix |
| `recursive` | boolean | `true` | Include subfolders |

**Example:**
```json
{
  "detail": "full",
  "folder": "journal/2025",
  "recursive": true
}
```

### edit_tags

Edit tags in a note's frontmatter. Adds/removes tags and optionally updates timestamp. Supports fuzzy path matching. At least one of `add` or `remove` must be provided.

| Param | Type | Description |
|-------|------|-------------|
| `path` | string | Path to the note — full relative path or partial (filename). Fuzzy suffix matching resolves partial paths. |
| `add` | array | *(optional)* Tags to add to the note's frontmatter |
| `remove` | array | *(optional)* Tags to remove from the note's frontmatter |

**Note:** At least one of `add` or `remove` must be provided.

**Example:**
```json
{
  "path": "journal/2025-02-15.md",
  "add": ["review", "important"],
  "remove": ["draft"]
}
```

## Index Management

### index_status

Get current index status: number of documents, chunks, and last indexing time.

**Example:**
```json
{}
```

### reindex

Reindex the vault. Without arguments, performs a full vault reindex. With a `path` argument, reindexes only that specific file.

| Param | Type | Description |
|-------|------|-------------|
| `path` | string | *(optional)* Relative path of a specific file to reindex |

**Example:**
```json
{
  "path": "journal/2025-02-15.md"
}
```
