# 📓 kajet 

Journaling-focused RAG for Obsidian vaults, optimized for Apple Silicon GPU. Runs as an MCP server with a web dashboard ([Serena](https://github.com/oramasearch/serena)-inspired). Think [Rosebud AI](https://rosebud.app/) but for your local markdown notes.

## Why "kajet"?

*Kajet* is an old/regional Polish word for a notebook (from French *cahier*). Once common, now mostly found in dialects or among older generations. The name came from a walk in the snow with the dog (the `/touch-grass` endpoint was temporarily unavailable due to weather conditions) — the phrase *"sprawdzić w kajecie"* ("check it in the notebook") struck me as an absurdly fitting thing to say to an LLM.

## Why this exists

I take a lot of notes in Obsidian and wanted a proper RAG pipeline that actually works for me — local, fast, and tailored to how I use my vault. [local-rag](https://github.com/jonfairbanks/local-rag) was an interesting starting point, but it runs JS-only CPU models. I wanted something optimized for macOS and Apple Silicon GPU, not a glorified `grep` burning through CPU cycles.

Also: the male urge to write a side-project in Rust was too strong. Nobody talks about the 30 GB `target/` folder, but here we are.

## Features

- 🔍 **Semantic search** over your entire vault via MCP `search` tool (hybrid vector + full-text)
- 🧠 **Local embeddings** — AllMiniLM-L6-v2 via [candle](https://github.com/huggingface/candle), Metal GPU on Apple Silicon, with custom model support
- 🌍 **Unicode normalization** — handles the two ways of writing `ę` in Unicode: NFC (`ę` as one character) vs NFD (`e` + combining ogonek). Searching for "Gdańsk" finds "Gdańsk" even when your filesystem and editor disagree on encoding
- ⚡ **Incremental indexing** — only re-embeds changed files (content hashing)
- 👀 **Live file watcher** — picks up vault changes automatically
- 📝 **Note editing** — create, edit, append, and modify notes directly via MCP tools (`create_note`, `edit_note`)
- 🌐 **Web dashboard** — search playground + live MCP event stream via WebSocket
- ☁️ **Cloud storage support** — auto-detects cloud-synced vaults (iCloud, OneDrive, Dropbox, Google Drive) and stores LanceDB outside the sync folder. This is critical for performance: indexing 600 files on iCloud takes ~1 file/sec vs ~70s total (~8.5 files/sec) when stored locally on M4 Mac
- 📦 **Single binary** — frontend embedded at compile time, zero runtime dependencies

## System Requirements

**Tested platforms:**
- macOS (Apple Silicon) — primary target with Metal GPU acceleration
- Linux (x86_64) — experimental CPU fallback

**Runtime requirements (rough estimates):**
- 16GB RAM (depends on vault size)
- ~500MB disk space for embedding model
- Additional space for LanceDB (varies by vault size)

**Building from source:**
- 15-40GB free disk space for Rust `target/` directory and dependencies
- Expect 5-10 minute initial build (LanceDB pulls in large dependency trees)

**Recommended:**
- macOS 12+ (Monterey) or later for Metal GPU support
- 8GB+ RAM for vaults with 1000+ files

## Prerequisites

### macOS
```bash
brew install protobuf
```

### Ubuntu/Debian
```bash
sudo apt install protobuf-compiler libssl-dev build-essential pkg-config
```

> ⚠️ **Note:** The above is AI hallucination. For a working Linux build, see the [CI workflow](https://github.com/jpalczewski/kajet/blob/develop/.github/workflows/ci.yml) — you'll need to translate dependencies to your favorite distro.

## Installation

### From source

```bash
# Clone and build (requires Deno for frontend build)
git clone https://github.com/yourusername/kajet.git
cd kajet
cd frontend && deno install && deno task build && cd ..
cargo build --release

# Binary will be at target/release/kajet
```

## ⚠️ Important Warning

**kajet includes tools that can modify and delete your notes.** Destructive edits are backed up automatically, but **this is experimental software**. LLM agents can be unpredictable — data loss is a real risk if your agent decides to overwrite files because you didn't say "good morning" or "thank you" nicely enough.

**This MCP is for playing around with data you have backed up.** Use git, Time Machine, or whatever backup solution you trust. Don't point it at your only copy of anything important.

You've been warned. 🙃

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

The dashboard will be available at `http://localhost:3579` while the MCP server is running.

**Example queries to try in Claude:**
- "What notes do I have about machine learning?"
- "Find my thoughts on productivity systems"
- "Show me notes mentioning both Rust and performance"

### With Goose

[Goose](https://block.github.io/goose/) has a GUI where you can add MCP servers by clicking through the interface.

**Protip:** Use the path to your built binary from `target/release/kajet` and pass `--vault /path/to/your/markdown/repo` as arguments.

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
                          (.kajet/ in vault or ~/Library/Application Support/kajet/)
```

**Stack:**
- **MCP**: Official `rmcp` SDK with `#[tool]` macros
- **Embeddings**: [candle](https://github.com/huggingface/candle) (AllMiniLM-L6-v2, Metal GPU on macOS, CPU fallback on Linux)
- **Vector DB**: [LanceDB](https://lancedb.com/) (embedded, Lance columnar format)
- **Frontend**: Svelte + Vite, embedded in binary via `rust-embed`
- **File watching**: [notify](https://github.com/notify-rs/notify) for live re-indexing
- **HTTP**: [Axum](https://github.com/tokio-rs/axum) with WebSocket support

**Workspace structure:**
- `crates/core` — Domain model, `Engine`, trait definitions
- `crates/parser` — Markdown parsing, chunking, wikilink extraction
- `crates/backend` — Concrete implementations (embedder, vector store)
- `crates/indexer` — Incremental indexing pipeline and file watcher
- `crates/mcp` — MCP protocol handler
- `crates/web` — Axum HTTP server and WebSocket broadcaster
- `crates/writer` — Note creation and editing 

## MCP Tools

### `search`
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

### `examine`
View a document's metadata and content. Accepts full or partial file paths with fuzzy matching.

| Param | Type | Default | Description |
|-------|------|---------|-------------|
| `path` | string | **required** | Full or partial file path |
| `content` | string | `"summary"` | Content mode: `"summary"` (first 500 chars), `"full"`, or `"slice"` |
| `offset` | number | `0` | Character offset for `"slice"` mode |
| `length` | number | `500` | Number of characters for `"slice"` mode |

### `edit_note`
Edit an existing note with multiple modes (append, prepend, replace, etc.).

| Param | Type | Description |
|-------|------|-------------|
| `path` | string | Full or partial file path |
| `content` | string | New content to insert or replace with |
| `mode` | string | Edit mode: `"append"`, `"prepend"`, `"overwrite"`, `"replace_section"`, `"replace_text"`, `"insert_after"` |
| `target_heading` | string | *(optional)* Target heading for section operations |
| `old_text` | string | *(optional)* Exact text to replace (required for `replace_text`) or anchor for `insert_after` |

### `create_note`
Create a new note with automatic frontmatter generation.

| Param | Type | Description |
|-------|------|-------------|
| `target` | string | Relative path for the new note (e.g., `"Projects/ideas.md"`) |
| `content` | string | Markdown body content |
| `tags` | array | *(optional)* Tags for frontmatter |
| `aliases` | array | *(optional)* Aliases for Obsidian linking |

### `list_tags`
List all unique tags from the vault with optional filtering.

| Param | Type | Default | Description |
|-------|------|---------|-------------|
| `detail` | string | `"counts"` | Detail level: `"names"`, `"counts"` (tag + doc count), `"full"` (tag + count + paths) |
| `folder` | string | *(optional)* | Filter by folder path prefix |
| `recursive` | boolean | `true` | Include subfolders |

### `index_status`
Get current index status: number of documents, chunks, and last indexing time.

### `reindex`
Reindex the vault. Without arguments, performs a full vault reindex. With a `path` argument, reindexes only that specific file.

| Param | Type | Description |
|-------|------|-------------|
| `path` | string | *(optional)* Relative path of a specific file to reindex |

## Development

### Prerequisites for building

- **Rust** (latest stable, edition 2024)
- **Deno** (for frontend build) — [deno.land](https://deno.land)
- **protobuf** (see [Prerequisites](#prerequisites))

### Building from source

```bash
# 1. Build frontend first
cd frontend
deno install
deno task build
cd ..

# 2. Build Rust workspace
cargo build --release

# Binary at target/release/kajet
```

### Running tests

```bash
# Run all tests with cargo-nextest (recommended)
cargo nextest run --workspace

# Run tests for a specific crate
cargo nextest run -p kajet-parser

# Run a single test by name
cargo nextest run -E 'test(test_name)'

# Fallback to standard cargo test if nextest not installed
cargo test --workspace
```

### Code style

```bash
# Format check (runs on pre-commit hook)
cargo fmt --check

# Lint (runs on pre-commit hook)
cargo clippy --workspace -- -D warnings

# Typo check (runs on pre-commit hook)
typos
```

Pre-commit hooks are managed via [Lefthook](https://github.com/evilmartians/lefthook). Install with:
```bash
lefthook install
```

### Running with MCP Inspector

```bash
# Use the included script
./run-inspector.sh /path/to/vault

# Or manually
npx @modelcontextprotocol/inspector cargo run -- --vault /path/to/vault
```

### Project conventions

- **Commits**: Use [Conventional Commits](https://www.conventionalcommits.org/) (`feat:`, `fix:`, `refactor:`, `perf:`, `docs:`, `test:`, `chore:`, `ci:`)
- **i18n**: User-facing strings go through `t!()` macro (rust-i18n). Locale files: `locales/{en,pl}.toml`
- **Logging levels**:
  - `INFO` = entry point (query, params, result count)
  - `DEBUG` = timings and score stats
  - `TRACE` = raw data (embeddings, scores)
  - Use `#[tracing::instrument]` with `skip(self)` on search methods

### Workspace architecture

The project uses a Cargo workspace with trait-based dependency injection:

```
kajet (root binary)
├── kajet-core        # Domain model, Engine, trait definitions
├── kajet-parser      # Markdown parsing, chunking, wikilinks
├── kajet-backend     # Concrete implementations (embedder, vector store)
├── kajet-indexer     # Incremental indexing pipeline + file watcher
├── kajet-mcp         # MCP protocol handler
├── kajet-web         # Axum HTTP server + WebSocket
└── kajet-writer      # Note creation and editing (WIP)
```

## Roadmap

- [ ] Advanced filtering (by date, folders, tag combinations)
- [ ] Daily notes and templates support
- [ ] Tag analysis tools (co-occurrence, edit tags)
- [ ] Temporal journal queries (recent context, on this day, get entries)
- [ ] Vault structure exploration tool
- [ ] Chunk quality scoring and viewer in dashboard

## Acknowledgments

- Inspired by [Serena](https://github.com/oramasearch/serena) for the MCP + dashboard approach
- Built on [LanceDB](https://lancedb.com/), [candle](https://github.com/huggingface/candle), and the [MCP Rust SDK](https://github.com/modelcontextprotocol/rust-sdk)
