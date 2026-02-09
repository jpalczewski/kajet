# Tree Tool Design

## Overview

MCP tool `tree` for exploring vault folder structure. Gives LLMs a compact overview of vault organization — folders, note counts, optionally file listings.

Based on [#51](https://github.com/jpalczewski/kajet/issues/51).

## Configuration

`TreeConfig` sub-config, configurable globally, per-vault, and via dashboard.

```toml
[tree]
depth = 3          # max traversal depth
size = 50           # max entries in output
max_chars = 5000    # safety net — refuse if output exceeds this
```

All three overridable per-request by LLM. Merge priority: request param > vault config > global config > default.

## MCP Tool Interface

**Name:** `tree`

**Parameters (all optional):**

| Param | Type | Description |
|-------|------|-------------|
| `path` | `String` | Subfolder to start from |
| `depth` | `usize` | Max depth (overrides config) |
| `size` | `usize` | Max entries (overrides config) |
| `show` | `String` | `"folders"` (default) or `"files"` |

**Output format:**

```
Vault tree (142 notes):
projects/ (8)
  kajet/ (5)
  other/ (3)
journal/ (42)
  2025/ (24)
  2024/ (18)
```

With `show: files`:

```
Vault tree (142 notes):
projects/ (8)
  kajet/ (5)
    README.md
    design.md
  other/ (3)
journal/ (42)
  ...
```

**max_chars behavior:** If formatted output exceeds `max_chars`, return refusal with suggestions instead of the tree:

```
Output too large (8234 chars, limit: 5000). Suggestions:
- Use `path` to focus on a subfolder
- Reduce `depth` (current: 5)
- Use `show: folders` instead of `files`
```

## Types

### kajet-parser

```rust
pub struct WalkEntry {
    pub rel_path: String,      // NFC-normalized, relative to vault root
    pub is_dir: bool,
}

pub struct VaultFolder {
    pub name: String,
    pub rel_path: String,
    pub note_count: usize,
    pub subfolders: Vec<VaultFolder>,
    pub files: Vec<String>,        // empty when show=folders
}

pub struct VaultTreeOptions {
    pub path: Option<String>,
    pub depth: usize,
    pub size: usize,
    pub show: ShowMode,
}

pub enum ShowMode {
    Folders,
    Files,
}
```

### kajet-core

```rust
pub struct TreeConfig {
    pub depth: usize,              // default: 3
    pub size: usize,               // default: 50
    pub max_chars: usize,          // default: 5000
}
```

### kajet-mcp

```rust
pub struct TreeRequest {
    pub path: Option<String>,
    pub depth: Option<usize>,
    pub size: Option<usize>,
    pub show: Option<String>,      // "folders" | "files"
}
```

## Architecture

### Shared walker refactor

Extract common parallel walk logic from `scan_vault()` into reusable `walk_vault()`:

```rust
pub fn walk_vault(
    vault_path: &str,
    exclude_folders: &[String],
) -> anyhow::Result<Vec<WalkEntry>>
```

- Parallel walk via `ignore::WalkBuilder`
- Exclude logic (`.obsidian`, `.git`, `.kajet`, `.trash` + user config)
- NFC path normalization
- Collects both dirs and `.md` files

`scan_vault()` refactored to delegate to `walk_vault()` — same API, zero regression.

### vault_tree() — two-phase

**Phase 1: Walk** — `walk_vault()` collects entries
**Phase 2: Build tree** — pure data, zero I/O:
- Sort entries, build `VaultFolder` tree from flat list
- Apply `path` filter (start from subfolder)
- Apply `depth` limit (prune tree)
- Apply `size` limit (cap total entries)
- `show: folders` → files not collected, only `note_count++`

### MCP handler flow

```
LLM request → merge params with TreeConfig → vault_tree() → format output
→ check max_chars → OK: return tree / OVER: return refusal + suggestions
```

## Files to modify

| File | Change |
|------|--------|
| `crates/parser/src/vault.rs` | `WalkEntry`, `walk_vault()`, refactor `scan_vault()`, new `VaultFolder`, `VaultTreeOptions`, `ShowMode`, `vault_tree()` |
| `crates/parser/src/lib.rs` | Re-exports |
| `crates/core/src/config.rs` | `TreeConfig`, field in `KajetConfig`, add `"tree"` to `GLOBAL_FIELDS` and `VAULT_FIELDS` |
| `crates/mcp/src/schema.rs` | `TreeRequest` |
| `crates/mcp/src/format.rs` | `format_vault_tree()`, `format_tree_too_large()` |
| `crates/mcp/src/tools.rs` | `tree` handler |
| `locales/en.toml`, `locales/pl.toml` | `tree_*` i18n keys |
| `frontend/src/lib/types.ts` | `TreeConfig` interface |
| `frontend/src/routes/Settings.svelte` | Tree settings section (global + vault) |

## i18n keys

```toml
tree_header = "Vault tree (%{count} notes):"
tree_too_large = "Output too large (%{chars} chars, limit: %{max_chars}). Suggestions:"
tree_suggestion_path = "- Use `path` to focus on a subfolder"
tree_suggestion_depth = "- Reduce `depth` (current: %{depth})"
tree_suggestion_show = "- Use `show: folders` instead of `files`"
tree_truncated = "... and %{count} more entries"
```

## Related

- [#51](https://github.com/jpalczewski/kajet/issues/51) — original issue
- [#66](https://github.com/jpalczewski/kajet/issues/66) — schema-driven settings UI (future)
