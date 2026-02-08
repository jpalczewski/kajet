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

Reindex the vault. Without arguments performs a full reindex; with `path` reindexes a single file.

| Parameter | Type | Description |
|---|---|---|
| `path` | `Option<String>` | Relative path of a specific file (omit for full reindex) |

### `examine`

Inspect an indexed document: metadata (title, tags, outgoing links, backlinks) and content. Supports fuzzy path matching.

| Parameter | Type | Description |
|---|---|---|
| `path` | `String` | Full or partial file path (fuzzy matched) |
| `content` | `Option<String>` | `"summary"` (default, first 500 chars), `"full"`, or `"slice"` |
| `offset` | `Option<usize>` | Character offset for slice mode |
| `length` | `Option<usize>` | Character count for slice mode |

### `index_status`

Get current index status: document count, chunk count, last indexing time. No parameters.

## Integration

- **Transport**: stdio (stdin/stdout) — designed for MCP clients
- **Events**: broadcasts `QueryEvent` via Tokio channel on each search, enabling real-time dashboard updates over WebSocket
- **i18n**: all user-facing strings localized via `t!()` macro (English + Polish)
