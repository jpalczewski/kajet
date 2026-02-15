# kajet-mcp

MCP (Model Context Protocol) server for kajet. Exposes semantic search over Obsidian vaults to AI assistants like Claude via stdio transport.

## Tools

### `search`

Search the vault using hybrid search (vector + full-text).

| Parameter | Type | Description |
|---|---|---|
| `query` | `String` | The search query |
| `limit` | `Option<usize>` | Max results (default from config) |
| `mode` | `Option<String>` | `"hybrid"` (default), `"vector"`, or `"fts"` |

### `reindex`

Reindex the vault. Without arguments performs a full reindex; with `path` reindexes a single file or all files under a folder path.

| Parameter | Type | Description |
|---|---|---|
| `path` | `Option<String>` | Relative path of a specific file or folder (omit for full reindex) |

### `examine`

Inspect one or more indexed documents: metadata (title, tags, outgoing links, backlinks) and content. Supports fuzzy path matching.

| Parameter | Type | Description |
|---|---|---|
| `paths` | `Vec<String>` | One or more full/partial file paths (fuzzy matched) |
| `content` | `Option<String>` | `"summary"` (default, first 500 chars), `"full"`, or `"slice"` |
| `offset` | `Option<usize>` | Character offset for slice mode |
| `length` | `Option<usize>` | Character count for slice mode |

### `index_status`

Get current index status: document count, chunk count, last indexing time. No parameters.

### `recent_context`

Get a compact overview of recent journaling activity: recent entries, top tags, writing frequency, gaps, and newly appearing tags.

| Parameter | Type | Description |
|---|---|---|
| `days` | `Option<u32>` | Analysis window in days (default: `7`, valid range: `1..=90`) |
| `limit` | `Option<usize>` | Maximum number of recent entries shown in the output (default: `10`) |

## Integration

- **Transport**: stdio (stdin/stdout) — designed for MCP clients
- **Events**: broadcasts `ActionEvent::QueryExecuted` via Tokio channel on each search, enabling real-time dashboard updates over WebSocket (includes query, results summary, and timing)
- **i18n**: all user-facing strings localized via `t!()` macro (English + Polish)
