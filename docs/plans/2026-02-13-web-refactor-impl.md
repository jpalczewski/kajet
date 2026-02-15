# Web Dashboard Refactoring — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Refactor web dashboard with Action Bus pattern, fix vault config bug, implement #16 (doc/chunk viewer), #29 (MCP result inspector), #66 (schema-driven settings), unify search, add ts-rs type generation.

**Architecture:** Event-driven Action Bus (tokio broadcast channel) as unified communication layer. Single WebSocket carries all events. Schema-driven config UI generated from Rust metadata. TypeScript types auto-generated via ts-rs.

**Tech Stack:** Rust (Axum, tokio, ts-rs, uuid), Svelte 5 (runes, svelte-spa-router), LanceDB

**Design doc:** `docs/plans/2026-02-13-web-refactor-design.md`

---

### Task 1: Fix vault config override bug + CliArgs

**Files:**
- Modify: `crates/core/src/types.rs:35-47` (AppState struct)
- Modify: `crates/web/src/lib.rs:233-282` (config handlers)
- Modify: `src/main.rs:144-156` (AppState construction)
- Test: `crates/core/src/config.rs` (add vault override reload test)

**Step 1: Write failing test for vault config reload**

Add to `crates/core/src/config.rs` in `mod tests`:

```rust
#[test]
fn vault_config_survives_reload_with_original_cli_args() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path();

    // Write vault override: embedding.backend = remote
    let mut embedding = toml::Table::new();
    embedding.insert("backend".into(), toml::Value::String("remote".into()));
    let mut updates = HashMap::new();
    updates.insert("embedding".into(), toml::Value::Table(embedding));
    write_vault_config(db_path, &updates).unwrap();

    // Load with CLI defaults (port=3579, no language override)
    let cfg = load_config(db_path, 3579, None).unwrap();
    assert_eq!(cfg.embedding.backend, EmbeddingBackend::Remote, "vault override should be active");

    // Simulate reload with ORIGINAL CLI args (not current config's port)
    let reloaded = reload_config(db_path, 3579, None).unwrap();
    assert_eq!(reloaded.embedding.backend, EmbeddingBackend::Remote, "vault override must survive reload");
}
```

**Step 2: Run test to verify it passes (this one should already pass — it validates the correct behavior)**

Run: `cargo nextest run -p kajet-core -E 'test(vault_config_survives_reload)'`

**Step 3: Write failing test demonstrating the bug**

```rust
#[test]
fn vault_config_lost_when_reloading_with_mutated_port() {
    // This test demonstrates that reloading with the *current* config's port
    // (instead of the original CLI port) causes vault overrides to be masked.
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path();

    // Global: port=3579 (default)
    // Vault override: embedding.backend = remote
    let mut embedding = toml::Table::new();
    embedding.insert("backend".into(), toml::Value::String("remote".into()));
    let mut updates = HashMap::new();
    updates.insert("embedding".into(), toml::Value::Table(embedding));
    write_vault_config(db_path, &updates).unwrap();

    let cfg = load_config(db_path, 3579, None).unwrap();
    assert_eq!(cfg.embedding.backend, EmbeddingBackend::Remote);

    // BUG SCENARIO: reload with cfg.port (same value but demonstrates the pattern)
    // The real bug is that web handlers pass cfg.port which may differ from CLI port
    let reloaded = reload_config(db_path, cfg.port, None).unwrap();
    assert_eq!(reloaded.embedding.backend, EmbeddingBackend::Remote);
}
```

Run: `cargo nextest run -p kajet-core -E 'test(vault_config_lost)'`

**Step 4: Add CliArgs struct to types.rs**

In `crates/core/src/types.rs`, add before AppState:

```rust
/// Original CLI arguments — preserved for config reload.
/// Web handlers must use these (not current config values) when reloading
/// to prevent CLI override priority from masking vault-level overrides.
#[derive(Debug, Clone)]
pub struct CliArgs {
    pub port: u16,
    pub language: Option<String>,
    pub model: Option<String>,
}
```

Add `cli_args: CliArgs` field to AppState (after `config` field).

**Step 5: Fix web config handlers**

In `crates/web/src/lib.rs`, replace lines 242-244 in `api_config_global`:

```rust
// Before (BUG):
let (port, cli_lang) = {
    let cfg = state.config.read().unwrap();
    (cfg.port, None::<String>)
};

// After (FIX):
let (port, cli_lang) = (state.cli_args.port, state.cli_args.language.clone());
```

Same fix in `api_config_vault` (lines 271-273).

**Step 6: Update main.rs AppState construction**

In `src/main.rs`, add `cli_args` to AppState init (~line 144):

```rust
let state = Arc::new(AppState {
    search_engine,
    events: tx,
    log_events: log_tx,
    log_buffer,
    cli_args: kajet_core::types::CliArgs {
        port: cli.port,
        language: cli.language.clone(),
        model: cli.model.clone(),
    },
    vault_path: cli.vault.clone(),
    db_path: db_path.clone(),
    note_count: AtomicUsize::new(0),
    chunk_count: AtomicUsize::new(0),
    indexing: AtomicBool::new(true),
    config: std::sync::RwLock::new(cfg),
    indexer: indexer.clone(),
});
```

**Step 7: Run all tests + clippy**

Run: `cargo nextest run --workspace && cargo clippy --workspace -- -D warnings`

**Step 8: Commit**

```bash
git add crates/core/src/types.rs crates/core/src/config.rs crates/web/src/lib.rs src/main.rs
git commit -m "fix: vault config override lost after reload

Store original CLI args in AppState.cli_args. Web config handlers now
use cli_args.port/language for reload instead of current config values,
preventing CLI override priority from masking vault-level overrides.

Fixes vault config being treated as global after restart."
```

---

### Task 2: Add ts-rs + define ActionEvent types

**Files:**
- Modify: `crates/core/Cargo.toml` (add ts-rs dependency)
- Create: `crates/core/src/actions.rs` (ActionEvent, ActionRequest, supporting types)
- Modify: `crates/core/src/lib.rs` (pub mod actions)
- Test: `crates/core/src/actions.rs` (serde roundtrip + TS export tests)

**Step 1: Add ts-rs dependency**

In `crates/core/Cargo.toml`, add:
```toml
ts-rs = { version = "11", features = ["serde-compat", "chrono-impl"] }
uuid = { version = "1", features = ["v4"] }
```

**Step 2: Create actions module with types**

Create `crates/core/src/actions.rs`:

```rust
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::types::IndexStats;

pub type ActionId = String;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../frontend/src/lib/types/generated/")]
pub enum IndexMode {
    Full,
    Incremental,
    SingleFile(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../frontend/src/lib/types/generated/")]
pub enum ConfigSection {
    Global,
    Vault,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../frontend/src/lib/types/generated/")]
pub struct SearchResultSummary {
    pub note_path: String,
    pub breadcrumb: String,
    pub content_preview: String,
    pub score: f64,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../frontend/src/lib/types/generated/")]
#[serde(tag = "type", content = "data")]
pub enum ActionEvent {
    IndexStarted {
        action_id: ActionId,
        mode: IndexMode,
    },
    IndexProgress {
        action_id: ActionId,
        processed: usize,
        total: usize,
    },
    IndexCompleted {
        action_id: ActionId,
        stats: IndexStats,
    },
    IndexFailed {
        action_id: ActionId,
        error: String,
    },
    ConfigChanged {
        section: ConfigSection,
    },
    EmbedderSwapped {
        new_backend: String,
        new_model: String,
    },
    StatsUpdated {
        note_count: usize,
        chunk_count: usize,
    },
    QueryExecuted {
        query: String,
        results: Vec<SearchResultSummary>,
        duration_ms: u64,
    },
    LogEntry {
        level: String,
        message: String,
        timestamp: String,
    },
}

#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export, export_to = "../../frontend/src/lib/types/generated/")]
#[serde(tag = "action")]
pub enum ActionRequest {
    Reindex { path: Option<String> },
    RefreshStats,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../frontend/src/lib/types/generated/")]
pub struct ActionResponse {
    pub action_id: ActionId,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_event_serializes_with_tag() {
        let event = ActionEvent::IndexStarted {
            action_id: "test-123".into(),
            mode: IndexMode::Full,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains(r#""type":"IndexStarted""#));
        assert!(json.contains(r#""data""#));
    }

    #[test]
    fn action_request_deserializes_reindex() {
        let json = r#"{"action":"Reindex","path":null}"#;
        let req: ActionRequest = serde_json::from_str(json).unwrap();
        assert!(matches!(req, ActionRequest::Reindex { path: None }));
    }

    #[test]
    fn action_request_deserializes_reindex_with_path() {
        let json = r#"{"action":"Reindex","path":"notes/test.md"}"#;
        let req: ActionRequest = serde_json::from_str(json).unwrap();
        assert!(matches!(req, ActionRequest::Reindex { path: Some(_) }));
    }

    #[test]
    fn query_executed_includes_results() {
        let event = ActionEvent::QueryExecuted {
            query: "test query".into(),
            results: vec![SearchResultSummary {
                note_path: "a.md".into(),
                breadcrumb: "a.md > Title".into(),
                content_preview: "Some content...".into(),
                score: 0.85,
            }],
            duration_ms: 42,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("a.md"));
        assert!(json.contains("0.85"));
    }
}
```

**Step 3: Add pub mod to lib.rs**

In `crates/core/src/lib.rs`, add: `pub mod actions;`

**Step 4: Add TS derive to IndexStats**

In `crates/core/src/types.rs`, add `ts_rs::TS` derive to `IndexStats`:

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, ts_rs::TS)]
#[ts(export, export_to = "../../frontend/src/lib/types/generated/")]
pub struct IndexStats {
    pub total_documents: usize,
    pub total_chunks: usize,
    pub last_indexed: Option<chrono::DateTime<chrono::Utc>>,
}
```

**Step 5: Run tests**

Run: `cargo nextest run -p kajet-core -E 'test(action)'`

**Step 6: Generate TS types**

Run: `cargo test -p kajet-core export_bindings 2>/dev/null; ls frontend/src/lib/types/generated/`

Verify `.ts` files are generated.

**Step 7: Commit**

```bash
git add crates/core/src/actions.rs crates/core/src/lib.rs crates/core/src/types.rs crates/core/Cargo.toml frontend/src/lib/types/generated/
git commit -m "feat: add ActionEvent types with ts-rs TypeScript generation

Define ActionEvent (unified event bus), ActionRequest (frontend actions),
SearchResultSummary, ActionResponse. All types auto-generate TypeScript
definitions via ts-rs to frontend/src/lib/types/generated/."
```

---

### Task 3: Action Bus in AppState + unified WebSocket

**Files:**
- Modify: `crates/core/src/types.rs:35-47` (replace events/log_events with action_bus)
- Modify: `crates/web/src/lib.rs` (WS handler, add POST /api/actions)
- Modify: `src/main.rs` (AppState construction, event emission)
- Modify: `crates/core/src/logging/types.rs` (WsMessage → use ActionEvent)
- Modify: `crates/mcp/src/ports.rs` (update event emission)
- Test: `crates/web/src/lib.rs` (action dispatch tests)

**Step 1: Replace event channels with action_bus in AppState**

In `crates/core/src/types.rs`, replace:
```rust
pub events: broadcast::Sender<QueryEvent>,
pub log_events: broadcast::Sender<LogEntry>,
```
with:
```rust
pub action_bus: broadcast::Sender<crate::actions::ActionEvent>,
```

Keep `log_buffer` (needed for WS connect history).

**Step 2: Update main.rs — single channel**

In `src/main.rs`, replace the two broadcast::channel calls with one:
```rust
let (action_tx, _) = broadcast::channel::<kajet_core::actions::ActionEvent>(512);
```

Update AppState construction to use `action_bus: action_tx.clone()`.

Pass `action_tx.clone()` to the logging layer so it can emit `ActionEvent::LogEntry`.

**Step 3: Update WebSocket handler to use action_bus**

In `crates/web/src/lib.rs`, replace `handle_ws`:
```rust
async fn handle_ws(mut socket: WebSocket, state: Arc<AppState>) {
    // Send ring buffer history on connect
    for entry in state.log_buffer.recent_entries() {
        let event = ActionEvent::LogEntry {
            level: entry.level.clone(),
            message: entry.message.clone(),
            timestamp: entry.timestamp.to_rfc3339(),
        };
        let json = serde_json::to_string(&event).unwrap_or_default();
        if socket.send(Message::Text(json.into())).await.is_err() {
            return;
        }
    }

    let mut rx = state.action_bus.subscribe();

    loop {
        match rx.recv().await {
            Ok(event) => {
                let json = serde_json::to_string(&event).unwrap_or_default();
                if socket.send(Message::Text(json.into())).await.is_err() {
                    break;
                }
            }
            Err(broadcast::error::RecvError::Lagged(n)) => {
                tracing::warn!(skipped = n, "WebSocket client lagged");
            }
            Err(_) => break,
        }
    }
}
```

**Step 4: Add POST /api/actions endpoint**

In `crates/web/src/lib.rs`, add route and handler:

```rust
.route("/api/actions", axum::routing::post(api_actions))

async fn api_actions(
    State(state): State<Arc<AppState>>,
    Json(request): Json<kajet_core::actions::ActionRequest>,
) -> impl IntoResponse {
    use kajet_core::actions::*;

    let action_id = uuid::Uuid::new_v4().to_string();

    match request {
        ActionRequest::Reindex { path } => {
            let id = action_id.clone();
            let s = state.clone();
            tokio::spawn(async move {
                let mode = match &path {
                    Some(p) => IndexMode::SingleFile(p.clone()),
                    None => IndexMode::Full,
                };
                let _ = s.action_bus.send(ActionEvent::IndexStarted {
                    action_id: id.clone(),
                    mode,
                });

                let vault = std::path::Path::new(&s.vault_path);
                let result = match path {
                    Some(ref p) => s.indexer.reindex_files(vault, &[p.clone()]).await
                        .map(|_| None),
                    None => {
                        let exclude = s.config.read().unwrap().exclude_folders.clone();
                        s.indexer.full_reindex(vault, &exclude).await.map(Some)
                    }
                };

                match result {
                    Ok(stats) => {
                        if let Some(stats) = stats {
                            s.note_count.store(stats.total_documents, std::sync::atomic::Ordering::Relaxed);
                            s.chunk_count.store(stats.total_chunks, std::sync::atomic::Ordering::Relaxed);
                            let _ = s.action_bus.send(ActionEvent::IndexCompleted {
                                action_id: id,
                                stats,
                            });
                        }
                        let _ = s.action_bus.send(ActionEvent::StatsUpdated {
                            note_count: s.note_count.load(std::sync::atomic::Ordering::Relaxed),
                            chunk_count: s.chunk_count.load(std::sync::atomic::Ordering::Relaxed),
                        });
                    }
                    Err(e) => {
                        let _ = s.action_bus.send(ActionEvent::IndexFailed {
                            action_id: id,
                            error: e.to_string(),
                        });
                    }
                }
            });
        }
        ActionRequest::RefreshStats => {
            if let Ok(stats) = state.indexer.get_index_stats().await {
                state.note_count.store(stats.total_documents, std::sync::atomic::Ordering::Relaxed);
                state.chunk_count.store(stats.total_chunks, std::sync::atomic::Ordering::Relaxed);
                let _ = state.action_bus.send(ActionEvent::StatsUpdated {
                    note_count: stats.total_documents,
                    chunk_count: stats.total_chunks,
                });
            }
        }
    }

    Json(ActionResponse { action_id })
}
```

**Step 5: Update MCP event emission**

In `crates/mcp/src/ports.rs`, wherever `QueryEvent` is emitted, change to emit `ActionEvent::QueryExecuted` instead. The MCP search handler needs to send results summary along with the query.

**Step 6: Update logging broadcast layer**

The logging broadcast layer currently sends `LogEntry` on `log_events` channel. It needs to wrap entries as `ActionEvent::LogEntry` and send on `action_bus` instead. Find the broadcast_layer module and update the `Layer` impl to use `ActionEvent`.

**Step 7: Run all tests + clippy**

Run: `cargo nextest run --workspace && cargo clippy --workspace -- -D warnings`

**Step 8: Commit**

```bash
git commit -m "feat: unified Action Bus replacing dual event channels

Replace separate QueryEvent + LogEntry broadcast channels with single
action_bus (broadcast::Sender<ActionEvent>). WebSocket handler forwards
all ActionEvent variants. Add POST /api/actions endpoint for frontend
to trigger reindex and refresh stats."
```

---

### Task 4: Update frontend WebSocket store + types

**Files:**
- Modify: `frontend/src/lib/stores/websocket.svelte.ts`
- Modify: `frontend/src/lib/types.ts` (import from generated)
- Modify: `frontend/src/lib/api.ts` (add actions API)
- Modify: `frontend/src/routes/Dashboard.svelte` (use new event types)
- Modify: `frontend/src/routes/Logs.svelte` (use new event types)

**Step 1: Update types.ts to re-export generated types**

In `frontend/src/lib/types.ts`, add imports from generated types:
```typescript
export type { ActionEvent } from './types/generated/ActionEvent';
export type { ActionRequest } from './types/generated/ActionRequest';
export type { ActionResponse } from './types/generated/ActionResponse';
export type { SearchResultSummary } from './types/generated/SearchResultSummary';
export type { IndexStats } from './types/generated/IndexStats';
export type { IndexMode } from './types/generated/IndexMode';
export type { ConfigSection } from './types/generated/ConfigSection';
```

Keep existing manually-defined types that aren't yet generated.

**Step 2: Update WebSocket store to handle ActionEvent**

Refactor `frontend/src/lib/stores/websocket.svelte.ts` to handle `ActionEvent` tagged union:

```typescript
import type { ActionEvent, SearchResultSummary } from '../types';

// Parse incoming WS messages as ActionEvent (tagged by "type")
// Map to appropriate stores: logs buffer, query events, index progress, stats
```

The store should maintain:
- `logs`: `ActionEvent[]` where type === 'LogEntry' (last 500)
- `queries`: `ActionEvent[]` where type === 'QueryExecuted' (last 50)
- `indexProgress`: latest IndexProgress or null
- `isIndexing`: derived from IndexStarted/IndexCompleted lifecycle
- `stats`: latest StatsUpdated `{ note_count, chunk_count }`
- `connected`: boolean

**Step 3: Add actions API to api.ts**

```typescript
export async function dispatchAction(request: ActionRequest): Promise<ActionResponse> {
    const resp = await fetch('/api/actions', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(request),
    });
    if (!resp.ok) throw new Error(`Action failed: ${resp.statusText}`);
    return resp.json();
}
```

**Step 4: Update Dashboard.svelte and Logs.svelte**

Adapt components to consume new event types from the updated store. The key change is that events are now `ActionEvent` variants instead of separate `QueryEvent`/`LogEntry` types.

**Step 5: Build frontend**

Run: `cd frontend && deno task build`

**Step 6: Commit**

```bash
git commit -m "feat(frontend): update stores and types for Action Bus

Refactor WebSocket store to handle unified ActionEvent. Add generated
TypeScript types from ts-rs. Add dispatchAction API function."
```

---

### Task 5: Add get_chunks_by_path to VectorStore trait (#16 backend)

**Files:**
- Modify: `crates/core/src/traits.rs:68-90` (VectorStore trait + mock)
- Modify: `crates/backend/src/store.rs` (LanceVectorStore impl)
- Test: `crates/core/src/traits.rs` (mock test)

**Step 1: Write failing test**

In `crates/core/src/traits.rs` mock tests section, add:
```rust
#[cfg(test)]
mod trait_tests {
    use super::mocks::*;
    use super::*;

    #[tokio::test]
    async fn mock_vector_store_get_chunks_by_path() {
        let store = MockVectorStore::new();
        store.stored.lock().unwrap().push(StoredChunk {
            note_path: "a.md".into(),
            breadcrumb: "a.md > Title".into(),
            content: "Hello".into(),
            raw_content: "Hello".into(),
            vector: vec![0.0; 4],
            chunk_index: 0,
            content_hash: "abc".into(),
            links: vec![],
        });
        store.stored.lock().unwrap().push(StoredChunk {
            note_path: "b.md".into(),
            breadcrumb: "b.md".into(),
            content: "World".into(),
            raw_content: "World".into(),
            vector: vec![0.0; 4],
            chunk_index: 0,
            content_hash: "def".into(),
            links: vec![],
        });

        let chunks = store.get_chunks_by_path("a.md").await.unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].note_path, "a.md");
    }
}
```

**Step 2: Run test — should fail (method doesn't exist)**

Run: `cargo nextest run -p kajet-core -E 'test(mock_vector_store_get_chunks_by_path)'`

**Step 3: Add method to VectorStore trait + Mock + Arc impl**

In `crates/core/src/traits.rs`:
- Add `async fn get_chunks_by_path(&self, note_path: &str) -> Result<Vec<StoredChunk>>;` to VectorStore trait
- Add impl to MockVectorStore (filter stored by note_path)
- Add forwarding impl to `impl<T: VectorStore> VectorStore for Arc<T>`

**Step 4: Implement in LanceVectorStore**

In `crates/backend/src/store.rs`, add:
```rust
async fn get_chunks_by_path(&self, note_path: &str) -> Result<Vec<StoredChunk>> {
    // Query chunks table WHERE note_path = ?
    // Return all chunks for that document, ordered by chunk_index
}
```

**Step 5: Run test**

Run: `cargo nextest run -p kajet-core -E 'test(mock_vector_store_get_chunks_by_path)'`

**Step 6: Commit**

```bash
git commit -m "feat: add get_chunks_by_path to VectorStore trait

Enables retrieving all chunks for a specific document path.
Implemented in MockVectorStore and LanceVectorStore."
```

---

### Task 6: Document/chunk API endpoints (#16)

**Files:**
- Modify: `crates/web/src/lib.rs` (add GET /api/documents, GET /api/documents/:path)
- Create: `crates/core/src/web_types.rs` (DocumentSummary, ChunkDetail, DocumentListResponse, DocumentDetail)
- Modify: `crates/core/src/lib.rs` (pub mod web_types)

**Step 1: Define response types with ts-rs**

Create `crates/core/src/web_types.rs`:

```rust
use serde::Serialize;
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../frontend/src/lib/types/generated/")]
pub struct DocumentSummary {
    pub source_file: String,
    pub title: String,
    pub tags: Vec<String>,
    pub chunk_count: usize,
    pub last_modified: f64,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../frontend/src/lib/types/generated/")]
pub struct ChunkDetail {
    pub chunk_index: u32,
    pub breadcrumb: String,
    pub content: String,
    pub raw_content: String,
    pub links: Vec<kajet_parser::Link>,
    pub char_count: usize,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../frontend/src/lib/types/generated/")]
pub struct DocumentListResponse {
    pub documents: Vec<DocumentSummary>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../frontend/src/lib/types/generated/")]
pub struct DocumentDetail {
    pub document: crate::types::Document,
    pub chunks: Vec<ChunkDetail>,
}
```

Also add `TS` derive to `kajet_parser::Link` and `crate::types::Document`.

**Step 2: Add API endpoints in kajet-web**

```rust
.route("/api/documents", get(api_documents))
.route("/api/documents/*path", get(api_document_detail))
```

`api_documents`: Query documents via `doc_store.get_all_documents()`, filter by search/tag params, paginate, map to `DocumentSummary`.

`api_document_detail`: Get document via `doc_store.get_document_by_path()`, get chunks via `store.get_chunks_by_path()`, map to `DocumentDetail` with `ChunkDetail`.

**Step 3: Access doc_store and store from AppState**

`SearchEngine` exposes `store()` and `doc_store()` — use those in the handlers.

**Step 4: Run tests + clippy**

Run: `cargo nextest run --workspace && cargo clippy --workspace -- -D warnings`

**Step 5: Commit**

```bash
git commit -m "feat(web): add document/chunk API endpoints (#16)

GET /api/documents — list documents with search/tag filtering, pagination
GET /api/documents/*path — document detail with all chunks

Types auto-generate TypeScript via ts-rs."
```

---

### Task 7: Search unification (dashboard → hybrid)

**Files:**
- Modify: `crates/web/src/lib.rs:170-185` (api_search handler)

**Step 1: Switch vector_search to hybrid_search**

In `api_search`, change:
```rust
// Before:
state.search_engine.vector_search(&params.q, params.limit).await

// After:
state.search_engine.hybrid_search(&params.q, params.limit).await
```

**Step 2: Emit QueryExecuted on action_bus**

After successful search, emit event:
```rust
let _ = state.action_bus.send(ActionEvent::QueryExecuted {
    query: params.q.clone(),
    results: results.iter().take(5).map(|r| SearchResultSummary {
        note_path: r.note_path.clone(),
        breadcrumb: r.breadcrumb.clone(),
        content_preview: r.content.chars().take(200).collect(),
        score: r.score as f64,
    }).collect(),
    duration_ms: start.elapsed().as_millis() as u64,
});
```

**Step 3: Run tests**

Run: `cargo nextest run -p kajet-web && cargo nextest run -p kajet-core -E 'test(hybrid)'`

**Step 4: Commit**

```bash
git commit -m "feat(web): unify dashboard search to hybrid_search

Dashboard now uses same hybrid_search (vector + FTS) as MCP.
Search results emit QueryExecuted event on action bus."
```

---

### Task 8: Config schema types + registry (#66 backend)

**Files:**
- Create: `crates/core/src/schema.rs` (ConfigSchema types + builder functions)
- Modify: `crates/core/src/lib.rs` (pub mod schema)
- Test: `crates/core/src/schema.rs` (schema completeness tests)

**Step 1: Write failing test**

```rust
#[test]
fn schema_has_embedding_section() {
    let config = KajetConfig::default();
    let schema = build_config_schema(&config);
    let embedding = schema.sections.iter().find(|s| s.key == "embedding");
    assert!(embedding.is_some(), "schema must have embedding section");
    let fields = &embedding.unwrap().fields;
    assert!(fields.iter().any(|f| f.key == "backend"));
    assert!(fields.iter().any(|f| f.key == "model"));
}
```

**Step 2: Define schema types**

```rust
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../frontend/src/lib/types/generated/")]
pub struct ConfigSchema { pub sections: Vec<SchemaSection> }

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../frontend/src/lib/types/generated/")]
pub struct SchemaSection {
    pub key: String,
    pub i18n_key: String,
    pub fields: Vec<SchemaField>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../frontend/src/lib/types/generated/")]
pub struct SchemaField {
    pub key: String,
    pub i18n_key: String,
    pub field_type: FieldType,
    pub default_value: serde_json::Value,
    pub constraints: Option<FieldConstraints>,
    pub widget: Option<String>,
    pub scope: FieldScope,
    pub restart_required: bool,
    pub hot_swap: bool,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../frontend/src/lib/types/generated/")]
pub enum FieldType {
    String, Number, Bool,
    Enum { options: Vec<String> },
    Array { item_type: Box<FieldType> },
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../frontend/src/lib/types/generated/")]
pub struct FieldConstraints {
    pub min: Option<f64>,
    pub max: Option<f64>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../frontend/src/lib/types/generated/")]
pub enum FieldScope { Global, Vault, Both }
```

**Step 3: Build schema registry**

Implement `build_config_schema()` with section builders for: general, embedding, logging, writer, tree, search_tuning. Each section builder creates `SchemaSection` with appropriate fields, types, defaults, widgets, scopes.

**Step 4: Add GET /api/config/schema endpoint**

In `crates/web/src/lib.rs`:
```rust
.route("/api/config/schema", get(api_config_schema))

async fn api_config_schema(State(state): State<Arc<AppState>>) -> Json<ConfigSchema> {
    let config = state.config.read().unwrap().clone();
    Json(build_config_schema(&config))
}
```

**Step 5: Run tests + generate TS**

Run: `cargo nextest run -p kajet-core -E 'test(schema)'`

**Step 6: Commit**

```bash
git commit -m "feat: config schema registry with TS type generation (#66)

Custom registry builds ConfigSchema from KajetConfig with per-field
metadata: type, widget, scope, i18n key, constraints, restart_required.
GET /api/config/schema endpoint serves schema to frontend."
```

---

### Task 9: Frontend — Document/Chunk Viewer (#16)

**Files:**
- Create: `frontend/src/routes/Documents.svelte`
- Create: `frontend/src/components/DocumentList.svelte`
- Create: `frontend/src/components/DocumentDetail.svelte`
- Create: `frontend/src/components/ChunkViewer.svelte`
- Modify: `frontend/src/lib/api.ts` (add getDocuments, getDocumentDetail)
- Modify: `frontend/src/App.svelte` (add /documents route)

**Step 1: Add API functions**

In `frontend/src/lib/api.ts`:
```typescript
export async function getDocuments(params: { search?: string; tag?: string; limit?: number; offset?: number }) {
    const qs = new URLSearchParams();
    if (params.search) qs.set('search', params.search);
    if (params.tag) qs.set('tag', params.tag);
    qs.set('limit', String(params.limit ?? 50));
    qs.set('offset', String(params.offset ?? 0));
    const resp = await fetch(`/api/documents?${qs}`);
    if (!resp.ok) throw new Error(resp.statusText);
    return resp.json();
}

export async function getDocumentDetail(path: string) {
    const resp = await fetch(`/api/documents/${encodeURIComponent(path)}`);
    if (!resp.ok) throw new Error(resp.statusText);
    return resp.json();
}
```

**Step 2: Create ChunkViewer component (reusable)**

Displays single chunk with breadcrumb, content, raw_content diff, links, char count. This component is reused in Documents page and MCP log inspector.

**Step 3: Create DocumentList component**

Filterable, paginated document list. Search input + tag filter. Each row: title, path, tags, chunk count, last modified. Click navigates to detail.

**Step 4: Create DocumentDetail component**

Document metadata + list of ChunkViewer components.

**Step 5: Create Documents route page**

Container: DocumentList by default, DocumentDetail when path param present.

**Step 6: Add route to App.svelte**

```javascript
import Documents from './routes/Documents.svelte';
const routes = {
    '/': Dashboard,
    '/documents': Documents,
    '/documents/*': Documents,
    '/status': Status,
    '/logs': Logs,
    '/settings': Settings,
};
```

Add nav link for Documents.

**Step 7: Add i18n keys**

Add document viewer keys to `locales/en.toml` and `locales/pl.toml`. Add keys to DASHBOARD_KEYS in `crates/web/src/lib.rs`.

**Step 8: Build + test**

Run: `cd frontend && deno task build && deno task check`

**Step 9: Commit**

```bash
git commit -m "feat(frontend): document and chunk viewer (#16)

New /documents route with searchable document list and detail view.
Reusable ChunkViewer component shows breadcrumb, content, links."
```

---

### Task 10: Frontend — MCP Result Inspector (#29)

**Files:**
- Create: `frontend/src/components/QueryLog.svelte`
- Modify: `frontend/src/routes/Logs.svelte` (add query log section)
- Modify: `frontend/src/routes/Dashboard.svelte` (expandable query results)

**Step 1: Create QueryLog component**

Expandable list of QueryExecuted events. Click expands to show search results list. Each result clickable → navigates to `/documents/{path}` or opens ChunkViewer inline.

**Step 2: Integrate into Logs page**

Add tab or section in Logs.svelte: "Queries" tab shows QueryLog, "Logs" tab shows log stream.

**Step 3: Integrate into Dashboard**

Dashboard's existing query event list becomes expandable — each entry shows results on click.

**Step 4: Build + test**

Run: `cd frontend && deno task build && deno task check`

**Step 5: Commit**

```bash
git commit -m "feat(frontend): MCP result inspector with expandable query log (#29)

QueryLog component shows search results inline. Integrated into
Logs page and Dashboard. Click result navigates to chunk viewer."
```

---

### Task 11: Frontend — Schema-Driven Settings (#66)

**Files:**
- Create: `frontend/src/components/DynamicSettings.svelte`
- Create: `frontend/src/components/DynamicField.svelte`
- Modify: `frontend/src/routes/Settings.svelte` (replace hardcoded with dynamic)
- Modify: `frontend/src/lib/api.ts` (add getConfigSchema)

**Step 1: Add schema API**

```typescript
export async function getConfigSchema() {
    const resp = await fetch('/api/config/schema');
    if (!resp.ok) throw new Error(resp.statusText);
    return resp.json();
}
```

**Step 2: Create DynamicField component**

Maps `SchemaField.widget` to appropriate input:
- `"select"` → `<select>` with options from FieldType::Enum
- `"toggle"` → `<input type="checkbox">`
- `"number"` → `<input type="number">` with min/max from constraints
- `"text"` → `<input type="text">`
- `"textarea"` → `<textarea>`
- `"password"` → `<input type="password">`
- `"array"` → comma-separated input (for exclude_folders etc.)

Shows i18n label, restart_required badge, hot_swap badge.

**Step 3: Create DynamicSettings component**

Renders sections from ConfigSchema. Filters fields by scope (Global vs Vault tab). Handles save for each scope.

```svelte
<script>
    let { schema, config, scope, onSave } = $props();
</script>

{#each schema.sections as section}
    <fieldset>
        <legend>{t(section.i18n_key)}</legend>
        {#each section.fields as field}
            {#if field.scope === scope || field.scope === 'Both'}
                <DynamicField {field} bind:value={config[section.key][field.key]} />
            {/if}
        {/each}
    </fieldset>
{/each}
```

**Step 4: Replace Settings.svelte**

Replace the ~590-line hardcoded Settings.svelte with:
- Load schema via `getConfigSchema()`
- Load config via `getConfig()`
- Render two tabs (Global / Vault) via `DynamicSettings`
- Save via existing `updateGlobalConfig`/`updateVaultConfig`

**Step 5: Build + verify**

Run: `cd frontend && deno task build && deno task check`

Manually verify settings page renders correctly with all sections.

**Step 6: Commit**

```bash
git commit -m "feat(frontend): schema-driven dynamic settings (#66)

Replace hardcoded Settings.svelte with DynamicSettings + DynamicField
components. Schema fetched from GET /api/config/schema. Adding new
config fields requires zero frontend changes."
```

---

### Task 12: Embedding hot-swap

**Files:**
- Modify: `crates/core/src/search.rs:24-29` (make embedder swappable)
- Modify: `crates/web/src/lib.rs` (detect embedding config change on save, trigger swap)
- Modify: `src/main.rs` (pass embedder creation logic)

**Step 1: Make SearchEngine embedder swappable**

In `crates/core/src/search.rs`, change:
```rust
embedder: Arc<dyn Embedder>,
// →
embedder: Arc<tokio::sync::RwLock<Arc<dyn Embedder>>>,
```

Add method:
```rust
pub async fn swap_embedder(&self, new_embedder: Arc<dyn Embedder>) {
    *self.embedder.write().await = new_embedder;
}
```

Update all methods that use `self.embedder` to acquire read lock first.

**Step 2: Detect embedding change in config save handlers**

After vault/global config save + reload, compare old vs new embedding config. If changed, emit `ActionEvent::EmbedderSwapped` and trigger reindex via action bus.

**Step 3: Run tests**

Run: `cargo nextest run --workspace`

**Step 4: Commit**

```bash
git commit -m "feat: embedding hot-swap on config change

SearchEngine embedder is now Arc<RwLock<Arc<dyn Embedder>>>.
Config save detects embedding changes and triggers embedder swap
+ full reindex via action bus."
```

---

### Task 13: Integration tests

**Files:**
- Create: `crates/web/tests/integration.rs` (Axum TestClient tests)

**Step 1: Create test helper for AppState with mocks**

```rust
use kajet_core::types::{AppState, CliArgs};
use kajet_core::actions::ActionEvent;

fn test_app_state() -> Arc<AppState> {
    let (action_tx, _) = broadcast::channel::<ActionEvent>(16);
    // Build with MockEmbedder, MockVectorStore, MockDocumentStore
    // ...
}
```

**Step 2: Write integration tests**

- GET /api/documents returns list
- GET /api/documents/:path returns detail with chunks
- GET /api/config/schema returns valid schema
- POST /api/actions reindex emits events
- PUT /api/config/vault preserves original CLI args
- GET /api/search returns hybrid results

**Step 3: Run tests**

Run: `cargo nextest run -p kajet-web`

**Step 4: Commit**

```bash
git commit -m "test(web): integration tests for new API endpoints

TestClient tests covering documents, chunks, config schema, actions,
and vault config reload with original CLI args."
```

---

### Task 14: i18n keys + cleanup

**Files:**
- Modify: `locales/en.toml` (add all new keys)
- Modify: `locales/pl.toml` (add all new keys)
- Modify: `crates/web/src/lib.rs` (DASHBOARD_KEYS list)

**Step 1: Add i18n keys for new UI**

Document viewer, chunk viewer, MCP inspector, schema settings sections, action progress messages, new nav items.

**Step 2: Run clippy + fmt + build**

Run: `cargo clippy --workspace -- -D warnings && cargo fmt --check && cd frontend && deno task build`

**Step 3: Commit**

```bash
git commit -m "chore: add i18n keys for new dashboard features

Keys for document viewer, chunk inspector, MCP results, schema-driven
settings, action progress. Both en.toml and pl.toml."
```

---

### Task 15: Final verification + cleanup

**Step 1: Run full test suite**

Run: `cargo nextest run --workspace`

**Step 2: Run clippy + fmt**

Run: `cargo clippy --workspace -- -D warnings && cargo fmt --check`

**Step 3: Build frontend**

Run: `cd frontend && deno task build && deno task check`

**Step 4: Build release**

Run: `cargo build --release`

**Step 5: Manual smoke test**

Run: `cargo run -- --vault ~/path/to/test/vault`
- Verify dashboard loads
- Verify /documents page shows docs
- Verify /settings renders from schema
- Verify search returns hybrid results
- Verify reindex button works
- Verify logs show ActionEvent stream

**Step 6: Commit any final fixes**

```bash
git commit -m "chore: final cleanup and verification"
```
