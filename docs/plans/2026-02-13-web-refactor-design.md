# Web Dashboard Refactoring — Action Bus + Schema-Driven Settings

**Date:** 2026-02-13
**Issues:** #16 (doc/chunk viewer), #29 (MCP result inspector), #66 (schema-driven settings)
**Approach:** Action Bus (event-driven with unified WebSocket)

## Context

The web dashboard is mostly read-only. Vault config override is broken (saved overrides load as global after restart). There's no standard pattern for initiating actions from the frontend. Adding new config sections requires manual UI work in multiple places.

## Goals

1. Fix vault config override bug
2. Establish a unified Action Bus pattern for frontend-initiated actions
3. Implement document/chunk viewer (#16)
4. Implement MCP result inspector (#29)
5. Replace hardcoded settings with schema-driven dynamic renderer (#66)
6. Unify dashboard search to use hybrid_search (same as MCP)
7. Generate TypeScript types from Rust via `ts-rs`
8. Well-tested, extensible architecture

## Section 1: AppState + Vault Fix + Rust→Frontend Pipeline

### 1a. AppState refactoring

```rust
/// Original CLI arguments — preserved for config reload
pub struct CliArgs {
    pub port: u16,
    pub language: Option<String>,
    pub model: Option<String>,
}

pub struct AppState {
    // Core services
    pub search_engine: SearchEngine,
    pub indexer: Arc<dyn IndexerHandle>,

    // Event channels
    pub action_bus: broadcast::Sender<ActionEvent>,  // unified event bus
    pub log_buffer: Arc<LogBuffer>,

    // Config & paths
    pub config: RwLock<KajetConfig>,
    pub cli_args: CliArgs,           // preserved for reload
    pub vault_path: String,
    pub db_path: PathBuf,

    // Runtime stats
    pub note_count: AtomicUsize,
    pub chunk_count: AtomicUsize,
    pub indexing: AtomicBool,
}
```

**Removed channels:** `events` (broadcast::Sender<QueryEvent>) and `log_events` (broadcast::Sender<LogEntry>) are replaced by `action_bus`.

### 1b. Vault config fix

**Bug location:** `kajet-web/src/lib.rs`, lines ~271-275 and ~242-244

**Root cause:** `api_config_vault` and `api_config_global` call `reload_config()` with `cfg.port` from the current in-memory config (which may include vault overrides). `load_config()` treats this as a CLI override (highest priority), masking vault overrides on subsequent reloads.

**Fix:** Store original CLI args in `AppState.cli_args`. Reload always uses:
```rust
reload_config(&state.db_path, state.cli_args.port, state.cli_args.language.clone())
```

### 1c. TypeScript type generation

Add `ts-rs` crate. All shared types get `#[derive(TS)] #[ts(export)]`:
- `ActionEvent`, `ActionRequest`, `IndexMode`, `ConfigSection`
- `DocumentSummary`, `ChunkDetail`, `SearchResultSummary`
- `ConfigSchema`, `SchemaSection`, `SchemaField`, `FieldType`, `FieldScope`
- `StatusResponse`, `SearchResponse`

Build step: `cargo test` generates `.ts` files to `frontend/src/lib/types/`.

### 1d. Embedding hot-swap

When embedding config changes via settings:
1. Config saved + reloaded
2. Action Bus emits `EmbedderConfigChanged`
3. Handler creates new Embedder (Candle or Remote)
4. Swaps into SearchEngine (Arc<RwLock<dyn Embedder>>)
5. Triggers full reindex via indexer
6. Progress streamed to WebSocket

Port changes remain restart-required (UI shows badge).

## Section 2: Action Bus — Unified Event System

### Types

```rust
pub type ActionId = String; // UUID

#[derive(Serialize, TS)]
#[serde(tag = "type", content = "data")]
pub enum ActionEvent {
    // Indexing
    IndexStarted { action_id: ActionId, mode: IndexMode },
    IndexProgress { action_id: ActionId, processed: usize, total: usize },
    IndexCompleted { action_id: ActionId, stats: IndexStats },
    IndexFailed { action_id: ActionId, error: String },

    // Config
    ConfigChanged { section: ConfigSection },
    EmbedderSwapped { new_backend: String, new_model: String },

    // Stats
    StatsUpdated { note_count: usize, chunk_count: usize },

    // Query events (replaces QueryEvent)
    QueryExecuted { query: String, results: Vec<SearchResultSummary>, duration_ms: u64 },

    // Logs (replaces LogEntry broadcast)
    LogEntry { level: String, message: String, timestamp: String },
}

#[derive(Deserialize, TS)]
#[serde(tag = "action")]
pub enum ActionRequest {
    Reindex { path: Option<String> },
    RefreshStats,
}
```

### Dispatch

```
POST /api/actions → ActionRequest → dispatch_action() → spawns async handler → returns ActionId
WebSocket ← action_bus broadcasts ActionEvent (progress, completion, error)
```

Each action handler:
1. Emits `*Started` event
2. Does work (potentially long-running in spawned task)
3. Emits `*Progress` events as needed
4. Emits `*Completed` or `*Failed`

### WebSocket unification

Single `/ws` endpoint subscribes to `action_bus` and forwards all `ActionEvent` as JSON. Replaces current dual-channel (QueryEvent + LogEntry) WebSocket.

## Section 3: Document/Chunk API (#16) + MCP Inspector (#29)

### New endpoints

```
GET /api/documents?search=<query>&tag=<tag>&limit=50&offset=0
  → DocumentListResponse { documents: Vec<DocumentSummary>, total: usize }

GET /api/documents/:path/chunks
  → DocumentDetail { document: Document, chunks: Vec<ChunkDetail> }
```

### New trait methods

```rust
// DocumentStore
async fn list_documents(&self, search: Option<&str>, tag: Option<&str>, limit: usize, offset: usize) -> Result<(Vec<DocumentSummary>, usize)>;
async fn get_document(&self, source_file: &str) -> Result<Option<Document>>;

// VectorStore
async fn get_chunks_by_path(&self, note_path: &str) -> Result<Vec<StoredChunk>>;
```

### MCP result inspector (#29)

`ActionEvent::QueryExecuted` includes `results: Vec<SearchResultSummary>` with note_path, breadcrumb, content_preview, score. Frontend expands MCP query log entries to show results, each clickable → `GET /api/documents/:path/chunks` → ChunkViewer.

### Search unification

Dashboard `/api/search` switches from `vector_search` to `hybrid_search` — same algorithm as MCP. Dashboard search also emits `QueryExecuted` on action bus.

## Section 4: Schema-Driven Settings (#66)

### Schema types

```rust
pub struct ConfigSchema {
    pub sections: Vec<SchemaSection>,
}

pub struct SchemaSection {
    pub key: String,
    pub i18n_key: String,
    pub fields: Vec<SchemaField>,
}

pub struct SchemaField {
    pub key: String,
    pub i18n_key: String,
    pub field_type: FieldType,
    pub default_value: serde_json::Value,
    pub constraints: Option<FieldConstraints>,
    pub widget: Option<String>,    // "select", "text", "number", "toggle", "textarea"
    pub scope: FieldScope,         // Global, Vault, Both
    pub restart_required: bool,
    pub hot_swap: bool,
}

pub enum FieldType {
    String, Number, Bool,
    Enum { options: Vec<String> },
    Array { item_type: Box<FieldType> },
}

pub enum FieldScope { Global, Vault, Both }
```

### Generation approach: Custom registry (Option B)

```rust
fn build_config_schema(config: &KajetConfig) -> ConfigSchema {
    ConfigSchema {
        sections: vec![
            embedding_section(config),
            tree_section(config),
            writer_section(config),
        ],
    }
}
```

Adding a new config section = add Rust struct + one `xxx_section()` function + i18n keys. Zero frontend changes.

### Endpoint

```
GET /api/config/schema → ConfigSchema
```

Frontend renders settings dynamically from schema using `DynamicSettings.svelte` → `DynamicField.svelte`.

## Section 5: Frontend Architecture

### Component hierarchy

```
App.svelte
├── Dashboard.svelte (search + results + stats)
│   ├── SearchBar.svelte
│   ├── SearchResults.svelte (clickable → ChunkViewer)
│   └── StatsBar.svelte (reactive via ActionStore)
├── Documents.svelte (NEW — #16)
│   ├── DocumentList.svelte (filterable, paginated)
│   └── DocumentDetail.svelte
│       └── ChunkViewer.svelte (reusable)
├── Logs.svelte (refactored)
│   ├── LogStream.svelte
│   └── QueryLog.svelte (#29 — expandable MCP results)
│       └── ChunkViewer.svelte (reused)
├── Settings.svelte (fully dynamic — #66)
│   ├── DynamicSettings.svelte
│   └── DynamicField.svelte
└── Status.svelte
```

### Stores (Svelte 5 runes)

- `ActionStore` — central event store fed by WebSocket. Derived state: `isIndexing`, `stats`, `queries`, `indexProgress`.
- `WebSocket store` — typed ActionEvent connection with auto-reconnect.
- `SchemaStore` — config schema cache from `GET /api/config/schema`.

### Routing

- `/` → Dashboard
- `/documents` → Document browser
- `/documents/:path` → Document detail + chunks
- `/logs` → Logs + MCP query inspector
- `/settings` → Dynamic settings
- `/status` → Status

## Section 6: Testing Strategy

### Level 1: Unit tests with mocks (fast, CI)

- Action dispatch emits correct events
- Document listing with tag/search filters
- Config schema generation has all sections
- Frontend component tests (vitest): DynamicField renders correct widgets

### Level 2: Integration tests with Axum TestClient + mocks

- Full HTTP round-trip: GET /api/documents, GET /api/config/schema, POST /api/actions
- WebSocket event flow: action request → WS receives events
- Config save + reload preserves vault overrides

### Level 3: E2E with real LanceDB (feature flag, optional)

- Full reindex via action endpoint with real vault
- Document/chunk retrieval from real LanceDB
- Embedding hot-swap with real CandleEmbedder

### Test helpers

- `test_app_state()` — builds AppState with mocks and action bus
- `doc_with_tags()` — creates test Document with tags
- `mock_config_schema()` — returns test ConfigSchema
- `MockIndexerHandle` — extended to track reindex calls

## Migration Plan

1. **Phase 1:** Fix vault config bug + add CliArgs to AppState
2. **Phase 2:** Add action_bus, unify WS, add action dispatch pattern
3. **Phase 3:** Add ts-rs, generate types, refactor frontend stores
4. **Phase 4:** Implement document/chunk API + viewer (#16)
5. **Phase 5:** Implement MCP result inspector (#29) + search unification
6. **Phase 6:** Implement schema-driven settings (#66)
7. **Phase 7:** Embedding hot-swap
8. **Phase 8:** E2E tests + cleanup
