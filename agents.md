# CLAUDE.md

## Project

kajet — Rust MCP server for semantic search over Obsidian journaling vaults. Local embeddings (Candle + Metal), LanceDB storage, single binary with embedded web dashboard.

**Target: macOS Apple Silicon.** Propose Metal-optimized, ARM-native, unified-memory-aware solutions. Linux is secondary.

## Build & Run

**Frontend-specific guidance:** See `@frontend/CLAUDE.md`

```bash
# Frontend (required before cargo build — embedded via rust-embed)
cd frontend && deno install && deno task build && cd ..

cargo build --release
cargo nextest run --workspace
cargo nextest run -p kajet-parser              # single crate
cargo nextest run -E 'test(test_name)'         # single test
cargo run -- --vault ~/path/to/vault           # dashboard at :3579
./run-inspector.sh /path/to/vault              # MCP Inspector
```

**Pre-commit (lefthook):**

```bash
cargo clippy --workspace -- -D warnings
cargo fmt --check
typos
```

## Workspace

```
kajet (root)        — CLI (clap), orchestration, spawns MCP + web + watcher
kajet-core          — Traits (Embedder, VectorStore, DocumentStore), types, Engine, config
kajet-backend       — CandleEmbedder (AllMiniLM-L6-v2, 384d), LanceVectorStore, document_store
kajet-parser        — Markdown chunking (pulldown-cmark), frontmatter, wikilinks
kajet-indexer       — Async pipeline (mpsc + semaphore), file watcher (notify), change detection
kajet-mcp           — MCP stdio handler, broadcasts query events via tokio channel
kajet-web           — Axum HTTP + WebSocket, embedded Svelte frontend (rust-embed)
```

## Critical Invariants

**These rules are not obvious from reading the code. Violating them causes subtle bugs.**

- `vault_path` ≠ `db_path`. Vault = user's Obsidian files. DB = LanceDB/config/logs (may be elsewhere for cloud-synced vaults via `db_path.rs`). All storage APIs take `db_path`, never hardcode `.kajet/`.
- **stdout is MCP transport.** All logging → stderr only. Any stdout print breaks MCP stdio protocol.
- **Search paths diverge**: Dashboard uses `vector_search` (chunks table). MCP uses `hybrid_search` (vector + FTS merged). FTS pulls from `documents.full_text` — text processing (wikilink cleaning etc.) must also apply in `fts_search()` at query time, or results will contain raw `[[wikilinks]]`.
- LanceDB FTS: always include `_score` in select columns for `full_text_search()` queries — otherwise lance logs deprecation warnings that pollute stderr.
- pulldown-cmark ignores `[[wikilinks]]` (not CommonMark) — they pass as `Event::Text`. Handling is post-parse via regex in `wikilinks.rs`.
- pulldown-cmark `End(TagEnd::Item)` doesn't emit whitespace — chunker must add `\n` after list items.
- Embedder model cached in `~/.cache/huggingface/`, auto-downloaded via hf-hub on first run.
- **LanceDB type consistency:** Schema definition, write array, and read downcast must use matching types. Example: `DataType::Int64` → `Int64Array::from` → `.downcast_ref::<Int64Array>()`. Mismatches fail at runtime ("invalid type"), not compile time.
- **Path handling must be centralized:** For folder-prefix/path membership checks, use `kajet_core::path_utils` (`normalize_folder_prefix`, `path_matches_folder_prefix`) instead of ad-hoc `starts_with` logic. If a needed path helper is missing (e.g. normalization/validation), add it to `kajet-core` and reuse it.
- **File-touching modules to keep aligned with core path utils:** `crates/indexer/src/lib.rs`, `crates/indexer/src/changes.rs`, `crates/indexer/src/pipeline.rs`, `crates/parser/src/vault.rs`, `crates/writer/src/resolve.rs`, `crates/web/src/lib.rs`.

## Code Style

**Rust idioms — be specific:**

- Prefer channels over mutexes for cross-task communication. Mutex only for truly shared mutable state with short critical sections.
- Prefer `spawn_blocking` for sync CPU-heavy work (embedding, hashing) — never block tokio worker threads.
- Use type system to prevent invalid states where practical (newtypes, enums over booleans, builder pattern for complex config).
- `thiserror` for library crate errors, `anyhow` for application-level. Error context with `.context()` — never bare `.unwrap()` in non-test code.
- `#[tracing::instrument]` with `skip(self)` on public methods.
- Reuse shared helpers from `kajet-core` for cross-crate behavior (especially path filtering/normalization). Avoid copying path logic between `indexer`/`parser`/`writer`/`web`/`mcp`.

**Logging levels (strict):**

- **INFO** — Operator-visible state changes: progress ("Indexing: 50% 250/500"), summaries ("47 added, 3 modified"), completion ("finished in 12.3s"), search queries with params and result count.
- **DEBUG** — Per-file timings and aggregate stats for tuning: `file:parsed path=note.md chunks=4 duration_ms=12`, `file:embedded duration_ms=87`, `file:done total_ms=102`, `batch_upsert docs=50 duration_ms=340`. Use `RUST_LOG=kajet=debug` to see full pipeline flow.
- **TRACE** — Raw data only, for debugging embedding/search quality: embedding vectors (`first_5=[0.023, -0.11, ...]`), token IDs, raw chunk content. Never timings.
- **ERROR** — Unrecoverable file/operation failures. **WARN** — Best-effort operations that failed (backlinks, FTS index).
- Tests: test behavior and edge cases, not that code compiles. Each test has a clear reason to exist. Use mocks from `kajet-core::traits::mocks`.
- **Test data**: All test fixtures should use anonymized content loosely inspired by Disco Elysium world (Revachol, RCM, Martinaise, etc.). Never use real personal data in tests.
- Trait-based DI: `Engine` accepts `Box<dyn Embedder>` + `Box<dyn VectorStore>` — preserve this for testability.

**Performance mindset:**

- Batch over single-item operations (especially LanceDB writes — RecordBatch vs per-row add is 10-100x difference).
- Pre-allocate buffers in hot paths. Avoid `Vec` allocation inside loops when capacity is known.
- Profile before micro-optimizing, but design for throughput: think about batch sizes, pipeline stages, backpressure.

## Conventions

- **Commits**: conventional (`feat:`, `fix:`, `refactor:`, `perf:`, `docs:`, `test:`, `chore:`, `ci:`). release-plz reads these.
- **i18n**: User-facing strings via `t!()` macro. Locales: `locales/{en,pl}.toml`.
- **Conversation language**: Polish. Code/comments/names: English.

## Anti-patterns to Avoid

- Don't hold Mutex across `.await` — use channels or restructure.
- Don't call sync I/O (`std::fs::*`) in async context without `spawn_blocking` (except `metadata()` which is fast enough).
- Don't rebuild FTS index after every single-file update — batch or debounce.
- Don't allocate `Vec<&str>` when `&[&str]` suffices in trait signatures.
- Don't fire-and-forget `tokio::spawn` without storing the `JoinHandle` — orphaned tasks leak on shutdown.
- Don't use `read_dir` manually — use `ignore::WalkBuilder` (already a dependency, respects .gitignore, parallel traversal).

## Gotchas

- `cargo clean` wipes ~14GB target/ — 5+ min rebuild. **Never delete target/ subdirectories** to free space.
- lancedb pulls AWS SDK (object_store → opendal) — long initial builds, not removable.
- Metal GPU auto-enabled on macOS via target-specific deps, CPU fallback on Linux.
- Proactively use MCP context7 (`resolve-library-id` → `query-docs`) to look up current crate docs before writing code.

## Database Schema Changes

When changing LanceDB schema (field types, new columns):

1. **Bump `CURRENT_SCHEMA_VERSION`** in `crates/backend/src/metadata.rs`
2. Add comment documenting the change (e.g., "Version 3: Fixed chunk_index type Int64")
3. Application auto-detects version mismatch and triggers full reindex

**Type consistency critical:** Arrow schema (`DataType::X`), write array (`XArray::from`), and read downcast (`.downcast_ref::<XArray>()`) must all match exactly. Mismatches cause "Database corruption" errors at runtime, not compile time.
