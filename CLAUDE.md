# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

kajet ("notebook" in Polish) is an MCP server providing semantic search for Obsidian vaults using local embeddings. It runs as a single binary with an embedded web dashboard for debugging.

**Target platform: macOS (Apple Silicon).** Optimize and propose solutions accordingly — Metal GPU, unified memory, ARM-native crates. Linux is a secondary target.

## Build & Run Commands

```bash
# Prerequisites: deno (https://deno.land)
cd frontend && deno install && deno task build && cd ..  # Build frontend (required before cargo build)
cargo build --release
cargo nextest run --workspace         # Run all tests (preferred runner)
cargo nextest run -p kajet-parser     # Run tests for a specific crate
cargo nextest run -E 'test(test_name)'  # Run a single test by name
cargo run -- --vault ~/path/to/vault  # Run with dashboard at http://localhost:3579
./run-inspector.sh /path/to/vault     # Run with MCP Inspector
```

## Architecture

```
stdin/stdout ←→ [MCP stdio] ←→ Engine ←→ [Axum HTTP :3579] ←→ Browser
                                  ↓
                              LanceDB (.kajet/ in vault)
```

**Root binary** (`src/main.rs`): CLI parsing (clap), app orchestration — spawns MCP server + web dashboard + file watcher as concurrent Tokio tasks.

**Workspace crates** (`crates/`):
- `kajet-core` — Domain model and abstractions
  - `traits.rs` — `Embedder` and `VectorStore` trait definitions + mock impls for testing
  - `types.rs` — shared types (`Chunk`, `SearchResult`, etc.)
  - `engine.rs` — core engine: indexing (embed + store) and search orchestration
  - `search.rs` — search query logic and result ranking
  - `config.rs` — configuration types
- `kajet-backend` — Concrete implementations of core traits
  - `embedder.rs` — `CandleEmbedder` (AllMiniLM-L6-v2 via candle, 384 dims)
  - `store.rs` — `LanceVectorStore` (LanceDB)
  - `document_store.rs` — document metadata persistence
  - `hasher.rs` — content hashing for incremental indexing
- `kajet-parser` — Markdown chunking by heading hierarchy with breadcrumb navigation, frontmatter extraction
- `kajet-indexer` — Incremental indexing pipeline
  - `pipeline.rs` — async embed-and-store pipeline
  - `changes.rs` — change detection (new/modified/deleted files)
  - `watcher.rs` — filesystem watcher for live re-indexing
- `kajet-mcp` — MCP protocol handler exposing `search` tool. Broadcasts query events via Tokio channel
- `kajet-web` — Axum HTTP server: search API (`/api/search`), WebSocket (`/ws`) for live events, embedded static assets

**Key patterns:**
- Trait-based DI: `Engine` accepts `Box<dyn Embedder>` and `Box<dyn VectorStore>` for testability
- Shared state via `Arc` for thread safety across MCP and web tasks
- Tokio broadcast channels for event streaming (MCP queries → WebSocket → dashboard)
- All logging goes to stderr (stdout reserved for MCP stdio transport)
- Frontend: Svelte + Vite (`frontend/`), built to `frontend/dist/` and embedded at compile time via `rust-embed`

## Code Style

- Write elegant, idiomatic Rust. Favor clarity and expressiveness — concise code over verbose, but never at the cost of readability.
- Tests should be meaningful: test actual behavior and edge cases, not just confirm that code runs. Each test should have a clear reason to exist.

## Linting & Pre-commit

Lefthook runs on every commit: `cargo fmt --check`, `cargo clippy --workspace -- -D warnings`, `typos`.
Run manually before committing:

```bash
cargo clippy --workspace -- -D warnings
cargo fmt --check
```

## Conventions

- **Commits**: conventional commits (`feat:`, `fix:`, `refactor:`, `perf:`, `docs:`, `test:`, `chore:`, `ci:`). release-plz generates changelogs from these.
- **i18n**: User-facing strings go through `t!()` macro (rust-i18n). Locale files: `locales/{en,pl}.toml`.

## Tooling

- Proactively use MCP context7 (`resolve-library-id` → `query-docs`) to look up current docs for crates and libraries before writing code. Don't rely on stale knowledge — check the docs.

## Gotchas

- `cargo clean` wipes ~14GB target/ — recompile takes 5+ min. Avoid unless necessary
- **Never delete target/ subdirectories** (debug/, release/, deps/) to free disk space — you'll just have to rebuild them immediately, wasting time. If disk is low, free space elsewhere or ask the user
- lancedb pulls in AWS SDK (object_store → opendal) — long initial builds, not removable via features
- Embeddings: candle with Metal GPU on macOS (auto-enabled via target-specific deps), CPU fallback on Linux. Model from HF Hub cached in `~/.cache/huggingface/`
- User prefers Polish for conversation
