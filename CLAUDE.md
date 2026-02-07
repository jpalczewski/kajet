# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

kajet ("notebook" in Polish) is an MCP server providing semantic search for Obsidian vaults using local embeddings. It runs as a single binary with an embedded web dashboard for debugging.

## Build & Run Commands

```bash
# Prerequisites: none (model downloaded automatically from HF Hub on first run)
cargo build --release
cargo test                    # Run all 28 tests
cargo test engine::tests      # Run tests for a specific module (engine, mcp, parser, web)
cargo test test_name          # Run a single test by name
cargo run -- --vault ~/path/to/vault   # Run with dashboard at http://localhost:3579
./run-inspector.sh /path/to/vault      # Run with MCP Inspector
```

## Architecture

```
stdin/stdout ←→ [MCP stdio] ←→ Engine ←→ [Axum HTTP :3579] ←→ Browser
                                  ↓
                              LanceDB (.kajet/ in vault)
```

**Modules:**
- `main.rs` — CLI parsing, app orchestration, spawns MCP server + web dashboard as concurrent tasks
- `engine.rs` — Core search engine: indexing (embed + store) and search (embed query + vector similarity). Contains `CandleEmbedder` (AllMiniLM-L6-v2 via candle, 384 dims) and `LanceVectorStore` implementations
- `mcp.rs` — MCP protocol handler exposing single `search` tool. Broadcasts query events via Tokio channel for dashboard
- `parser.rs` — Pure functions for markdown chunking by heading hierarchy with breadcrumb navigation
- `traits.rs` — `Embedder` and `VectorStore` trait abstractions + mock implementations for testing
- `web.rs` — Axum HTTP server with search API (`/api/search`), WebSocket (`/ws`) for live query events, and embedded static assets

**Key patterns:**
- Trait-based DI: `Engine` accepts `Box<dyn Embedder>` and `Box<dyn VectorStore>` for testability
- Shared state via `Arc` for thread safety across MCP and web tasks
- Tokio broadcast channels for event streaming (MCP queries → WebSocket → dashboard)
- All logging goes to stderr (stdout reserved for MCP stdio transport)
- Frontend is a single HTML file (`frontend/index.html`) using htmx + vanilla JS, embedded at compile time via `rust-embed`

## Code Style

- Write elegant, idiomatic Rust. Favor clarity and expressiveness — concise code over verbose, but never at the cost of readability.
- Tests should be meaningful: test actual behavior and edge cases, not just confirm that code runs. Each test should have a clear reason to exist.

## Tooling

- Proactively use MCP context7 (`resolve-library-id` → `query-docs`) to look up current docs for crates and libraries before writing code. Don't rely on stale knowledge — check the docs.
