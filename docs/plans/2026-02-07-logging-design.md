# Logging System Design

## Overview

Replace the current single stderr tracing setup with a dual-sink logging system: file and web dashboard. No console output at all — stdout reserved for MCP, stderr no longer used for logs.

## Architecture

```
tracing events
    ├── FileLayer        → .kajet/kajet.log (JSON lines, overwritten on start)
    └── BroadcastLayer   → tokio::broadcast → WebSocket → dashboard tab
```

Both sinks are `tracing_subscriber::Layer` implementations on a shared `Registry`.

### Level hierarchy

```
global_level (config)          # floor — nothing below passes
├── file_level (config / UI)   # default: trace
└── dashboard_level (UI)       # default: info
```

Effective sink level = `max(global_level, sink_level)`.

All three levels persist in `kajet.toml` and are editable from the dashboard UI.

### Configuration

```toml
[logging]
level = "debug"              # global floor
file_level = "trace"         # file sink level
dashboard_level = "info"     # dashboard sink level
progress_percent_step = 5    # report indexing progress every N%
```

## Trace logging for indexing pipeline

Two levels of progress reporting:

**INFO** — percentage progress:
```
Indexing: 25% (150/600 files) — elapsed 12.3s
Indexing: 50% (300/600 files) — elapsed 23.1s
Indexing complete: 600 files, 2847 chunks — total 45.2s
```
Threshold controlled by `progress_percent_step`.

**TRACE** — full file lifecycle:
```
file:start path="notes/rust.md"
file:parsed path="notes/rust.md" chunks=5 duration_ms=2
file:embedded path="notes/rust.md" chunks=5 duration_ms=45
file:stored path="notes/rust.md" chunks=5 duration_ms=8
file:done path="notes/rust.md" total_ms=55
```

**DEBUG** — pipeline decisions:
```
file:skip path="notes/old.md" reason="unchanged hash"
file:delete path="notes/removed.md" chunks_removed=3
```

All events use structured fields (JSON fields in file, key=value in dashboard).

## BroadcastLayer and dashboard

### Backend

`BroadcastLayer` serializes events to `LogEntry`:
```rust
struct LogEntry {
    timestamp: DateTime<Utc>,
    level: Level,
    target: String,
    message: String,
    fields: HashMap<String, Value>,
}
```

Delivered via existing WebSocket (`/ws`) as a new message type:
```json
{"type": "log", "data": {"level": "INFO", "target": "...", "message": "...", ...}}
```

Ring buffer of ~500 recent entries so the dashboard has history on connect.

### Frontend

New "Logs" tab on the dashboard:
- Dropdown to select visible level (filters client-side)
- Log list with human-readable format, color-coded by level
- Auto-scroll to bottom, pauses when user scrolls up
- Controls to change `file_level` and `dashboard_level` (persisted via config API)

## File sink

- Path: `.kajet/kajet.log`
- Format: JSON lines (one JSON object per line)
- Overwritten on each application start
- No rotation

## Scope of changes

### New/modified files

1. `crates/core/src/config.rs` — add `[logging]` section to config struct
2. `crates/core/src/logging/` — new module:
   - `file_layer.rs` — Layer writing JSON lines to `.kajet/kajet.log`
   - `broadcast_layer.rs` — Layer sending LogEntry via broadcast channel
   - `types.rs` — LogEntry struct
   - `mod.rs` — `init_logging(config, broadcast_tx)` replaces current tracing setup
3. `src/main.rs` — replace stderr tracing init with `init_logging()`, remove `eprintln!`
4. `crates/indexer/src/pipeline.rs` + `crates/core/src/engine.rs` — add trace!/debug!/info! with structured fields for file lifecycle and percentage progress
5. `crates/web/src/lib.rs` — extend WS protocol with `log` message type, ring buffer
6. Frontend — new Logs tab with dropdown and log list

### Unchanged

- Core traits (`Embedder`, `VectorStore`) — logging doesn't affect them
- MCP — zero changes, stdout untouched
- Parser — no changes, trace logs live at pipeline level
