# kajet 📓

MCP server for semantic search over your Obsidian vault, with a live debug dashboard.

## Prerequisites

### macOS
```bash
brew install protobuf
```

### Ubuntu/Debian
```bash
sudo apt install protobuf-compiler libssl-dev build-essential pkg-config
```

## Build

```bash
cargo build --release
```

## Usage

### As MCP server (Claude Code / Claude Desktop)

Add to your `.claude/mcp.json` or Claude Desktop config:

```json
{
  "mcpServers": {
    "kajet": {
      "command": "/path/to/kajet",
      "args": ["--vault", "/path/to/your/obsidian/vault"]
    }
  }
}
```

Then open `http://localhost:3579` to see the live dashboard.

### Standalone (search playground only)

```bash
kajet --vault ~/Obsidian/Vault
# Dashboard at http://localhost:3579
```

## Architecture

```
stdin/stdout ←→ [MCP stdio] ←→ Engine ←→ [Axum HTTP :3579] ←→ Browser
                                  ↓
                              LanceDB
                          (.kajet/ in vault)
```

- **MCP**: Official `rmcp` SDK, `#[tool]` macros
- **Embeddings**: `fastembed` (AllMiniLM-L6-v2, local ONNX)
- **Vector DB**: LanceDB (embedded, Lance columnar format)
- **Frontend**: Single HTML file, htmx + WebSocket
- **Dashboard**: Embedded in binary via `rust-embed`

## MCP Tools

### `search`
Semantic search over the vault.

| Param | Type | Default | Description |
|-------|------|---------|-------------|
| query | string | required | Search query |
| limit | number | 5 | Max results |
