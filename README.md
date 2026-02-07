# kajet 📓

Local RAG for Obsidian vaults, optimized for Apple Silicon GPU. Runs as an MCP server with a web dashboard (yes, [Serena](https://github.com/oramasearch/serena)-inspired).

## Why

I take a lot of notes in Obsidian and wanted a proper RAG pipeline that actually works for me — local, fast, and tailored to how I use my vault. [local-rag](https://github.com/jonfairbanks/local-rag) was an interesting starting point, but it runs JS-only CPU models. I wanted something optimized for macOS and Apple Silicon GPU, not a glorified `grep` burning through CPU cycles.

Also: the male urge to write a side-project in Rust was too strong. Nobody talks about the 30 GB `target/` folder, but here we are.

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

## Features

- **Semantic search** over your entire vault via MCP `search` tool
- **Local embeddings** — AllMiniLM-L6-v2 via candle, Metal GPU on Apple Silicon
- **Incremental indexing** — only re-embeds changed files
- **Live file watcher** — picks up vault changes automatically
- **Web dashboard** — search playground + live MCP event stream via WebSocket
- **Single binary** — frontend embedded at compile time, zero runtime dependencies

## Architecture

```
stdin/stdout ←→ [MCP stdio] ←→ Engine ←→ [Axum HTTP :3579] ←→ Browser
                                  ↓
                              LanceDB
                          (.kajet/ in vault)
```

- **MCP**: Official `rmcp` SDK, `#[tool]` macros
- **Embeddings**: candle (AllMiniLM-L6-v2, Metal GPU on macOS, CPU fallback on Linux)
- **Vector DB**: LanceDB (embedded, Lance columnar format)
- **Frontend**: Svelte + Vite, embedded in binary via `rust-embed`

## MCP Tools

### `search`
Semantic search over the vault.

| Param | Type | Default | Description |
|-------|------|---------|-------------|
| query | string | required | Search query |
| limit | number | 5 | Max results |
