# README Refactoring Design

**Date:** 2026-02-17
**Status:** Approved

## Context

README has become outdated:
- Only 6 MCP tools documented, but codebase has 12 tools (5 undocumented)
- Roadmap in repo gets stale quickly
- Candle embedding backend is in maintenance mode (user migrating to TEI)
- Tools documentation hard to maintain inline in README

## Goals

1. Move MCP tools documentation to separate file for easier maintenance
2. Add deprecation notice for Candle backend (subtle, non-alarming)
3. Link to GitHub Issues for roadmap (dynamic, always current)
4. Remove outdated remote-embedder design docs from repo
5. Keep README focused and high-level

## Design

### File Structure Changes

**New files:**
- `docs/TOOLS.md` - Complete reference for all 12 MCP tools

**Removed files:**
- `docs/plans/2026-02-12-remote-embedder-design.md`
- `docs/plans/2026-02-12-remote-embedder-impl.md`
- `docs/plans/2026-02-12-remote-embedder-impl-v2.md`

**Modified files:**
- `README.md` - header links, TEI migration notice, simplified MCP Tools and Roadmap sections

### README.md Changes

#### 1. Header Section (after line 3)

Add navigation links:

```markdown
**[📋 Changelog](CHANGELOG.md)** | **[🛠️ Tools Reference](docs/TOOLS.md)** | **[🗺️ Roadmap](https://github.com/jpalczewski/kajet/issues)**
```

#### 2. Features Section (line ~16-26)

Add new bullet point after "Local embeddings":

```markdown
- 🧠 **Local embeddings** — AllMiniLM-L6-v2 via [candle](https://github.com/huggingface/candle), Metal GPU on Apple Silicon, with custom model support
- ⚠️ **Embeddings migration path** — The built-in Candle backend works but is limited to a handful of models. For access to modern embedding models (nomic-embed, BGE-M3, E5-mistral, multilingual models, etc.), kajet now supports [Text Embeddings Inference (TEI)](https://github.com/huggingface/text-embeddings-inference) via the `kajet-remote` crate. TEI can run locally on the same machine or point to a remote endpoint as your needs scale. Candle backend remains available but is in maintenance mode.
- 🌍 **Unicode normalization** — handles the two ways of writing `ę` in Unicode...
```

**Rationale:** Not alarming, explains benefits (more models, scalability), keeps Candle as valid option.

#### 3. MCP Tools Section (lines 145-213)

Replace detailed tool documentation with:

```markdown
## MCP Tools

kajet provides 12 MCP tools for semantic search, note management, and vault exploration.

See **[docs/TOOLS.md](docs/TOOLS.md)** for complete documentation with parameters and examples.
```

**Rationale:** README stays high-level, detailed reference in docs/ easier to maintain.

#### 4. Roadmap Section (lines 307-315)

Replace checklist with:

```markdown
## Roadmap

See [GitHub Issues](https://github.com/jpalczewski/kajet/issues) for planned features.
```

**Rationale:** Issues are dynamic, no stale checkboxes in repo.

### docs/TOOLS.md Structure

Format: H2 for categories, H3 for individual tools, parameter tables + examples (same style as current README).

**Categories:**
1. **Search & Discovery** - `search`, `find_similar`, `explore_connections`, `recent_context`
2. **Documents** - `examine`, `tree`
3. **Notes** - `create_note`, `edit_note`
4. **Tags** - `list_tags`, `edit_tags`
5. **Index Management** - `index_status`, `reindex`

**Tool documentation format (example):**

```markdown
### search

Semantic search over the vault using hybrid vector + full-text search.

| Param | Type | Default | Description |
|-------|------|---------|-------------|
| `query` | string | *(optional)* | Search query |
| `limit` | number | `5` | Max results (1-50) |
| `mode` | string | `"hybrid"` | Search mode: `"hybrid"`, `"vector"`, `"fts"` |
| `from` | string | *(optional)* | Start date filter |
| `to` | string | *(optional)* | End date filter |
| `tags` | array | *(optional)* | Filter by tags |
| `folder` | string | *(optional)* | Filter by folder prefix |

**Example:**
```json
{
  "query": "productivity and note-taking",
  "limit": 10,
  "tags": ["work"]
}
```
```

**Source for undocumented tools:** Extract from `#[tool(description = "...")]` attributes in handlers:
- `crates/mcp/src/handlers/search.rs` - `search`
- `crates/mcp/src/handlers/documents.rs` - `examine`, `explore_connections`, `find_similar`
- `crates/mcp/src/handlers/notes.rs` - `create_note`, `edit_note`
- `crates/mcp/src/handlers/tags.rs` - `list_tags`, `edit_tags`
- `crates/mcp/src/handlers/index.rs` - `index_status`, `reindex`
- `crates/mcp/src/handlers/tree.rs` - `tree`
- `crates/mcp/src/handlers/analytics.rs` - `recent_context`

## Implementation Notes

1. **Extract tool schemas from code** - Read handler files to get parameter schemas for undocumented tools
2. **Preserve existing docs** - Copy current README tool docs as-is to TOOLS.md (search, examine, create_note, edit_note, list_tags, index_status, reindex)
3. **Git operations** - Delete old remote-embedder docs, create TOOLS.md, modify README
4. **Single commit** - All changes in one commit for atomic refactor

## Trade-offs

**Pros:**
- README stays focused on getting started
- Tools docs easier to maintain (separate file, can update without touching README)
- Roadmap always current (GitHub Issues)
- Clear migration path for Candle users without panic

**Cons:**
- One extra click to see tool docs (but acceptable for reference material)
- Need to keep TOOLS.md in sync with code changes (but easier than inline README)

## Success Criteria

- [ ] README has header navigation links
- [ ] TEI migration notice in Features (non-alarming, benefit-focused)
- [ ] MCP Tools section simplified to link
- [ ] Roadmap section links to issues
- [ ] docs/TOOLS.md created with all 12 tools documented
- [ ] Old remote-embedder design docs removed
- [ ] All links functional
- [ ] Commit message follows conventional commits format
