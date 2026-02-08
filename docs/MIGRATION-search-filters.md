# Migration Guide: Search Filters Feature

**Version:** 0.3.0
**Date:** 2026-02-08
**PR:** #64

## Overview

This release introduces temporal and tag/folder filtering to the `search` MCP tool. The tool now supports two distinct modes:
- **Search mode** (with `query`): Semantic/hybrid/FTS search with optional filters
- **Browse mode** (without `query`): Pure temporal/tag/folder filtering

## Breaking Changes

### 1. SearchRequest Schema Change

**Before:**
```json
{
  "query": "rust",        // Required
  "mode": "hybrid",
  "limit": 5
}
```

**After:**
```json
{
  "query": "rust",        // Now OPTIONAL
  "mode": "hybrid",
  "limit": 5,
  "from": "2025-01-01",   // NEW: Optional date filter
  "to": "2025-01-31",     // NEW: Optional date filter
  "tags": ["work"],       // NEW: Optional tag filter
  "folder": "journal"     // NEW: Optional folder filter
}
```

**Impact:** The `query` field is now `Option<String>` instead of `String`.

### 2. Validation Rule

**New requirement:** At least one of `query` or a filter (`from`/`to`/`tags`/`folder`) must be provided.

**Error if both are missing:**
```json
{}  // ❌ Error: "At least 'query' or one filter is required"
```

**Valid requests:**
```json
{"query": "rust"}                    // ✅ Search mode (existing behavior)
{"from": "last week"}                // ✅ Browse mode (new)
{"query": "rust", "tags": ["work"]}  // ✅ Search mode with filter (new)
```

## Migration Steps

### For MCP Client Users (Claude Desktop, etc.)

**No action required.** The change is backward compatible:
- Existing queries with `query` field continue to work as before
- New filter parameters are optional

### For Custom Integrations

If you're using the kajet MCP server programmatically:

1. **Update schema definitions** if you're validating requests:
   ```typescript
   // Before
   interface SearchRequest {
     query: string;
     mode?: string;
     limit?: number;
   }

   // After
   interface SearchRequest {
     query?: string;        // Now optional
     mode?: string;
     limit?: number;
     from?: string;         // New
     to?: string;           // New
     tags?: string[];       // New
     folder?: string;       // New
   }
   ```

2. **Update validation logic** to ensure at least `query` or one filter is present:
   ```typescript
   function validateSearchRequest(req: SearchRequest) {
     const hasQuery = !!req.query;
     const hasFilters = !!(req.from || req.to || req.tags?.length || req.folder);

     if (!hasQuery && !hasFilters) {
       throw new Error("At least 'query' or one filter is required");
     }
   }
   ```

### For Developers Extending kajet

If you're implementing custom `DocumentStore` or working with the trait:

1. **Implement new trait method:**
   ```rust
   #[async_trait]
   impl DocumentStore for YourStore {
       // ... existing methods ...

       async fn query_documents(
           &self,
           from: Option<f64>,
           to: Option<f64>,
           folder: Option<&str>,
           limit: usize,
       ) -> Result<Vec<Document>> {
           // Your implementation here
           // - Filter by timestamp range (from/to)
           // - Filter by folder prefix
           // - Sort chronologically (oldest first)
           // - Apply limit
       }
   }
   ```

2. **Update mock implementations** in tests if using `MockDocumentStore`:
   ```rust
   use kajet_core::traits::mocks::MockDocumentStore;

   // MockDocumentStore now includes query_documents() implementation
   // No changes needed if using the provided mock
   ```

## New Features

### 1. Date Filtering

**Supported formats:**

**ISO 8601:**
```json
{"from": "2025-01-15"}           // Specific date
{"from": "2025-01", "to": "2025-01"}  // Whole month
```

**Polish keywords:**
```json
{"from": "dzisiaj"}              // Today
{"from": "wczoraj"}              // Yesterday
{"from": "zeszły tydzień"}       // Last week
{"from": "zeszły miesiąc"}       // Last month
{"from": "zeszły rok"}           // Last year
{"from": "w styczniu"}           // In January (current year)
```

**English keywords:**
```json
{"from": "today"}
{"from": "yesterday"}
{"from": "last week"}
{"from": "last month"}
{"from": "last year"}
{"from": "in january"}
```

**Default behavior:**
- If `from` is provided without `to`, `to` defaults to end of today
- Dates are context-sensitive: `"2025-01"` with `from` = 1st, with `to` = 31st

### 2. Tag Filtering

**All specified tags are required (AND logic, not OR):**

```json
{"tags": ["work"]}               // Documents with #work
{"tags": ["work", "meeting"]}    // Documents with BOTH #work AND #meeting
```

**Features:**
- Case-insensitive: `["Work"]` matches `#work`
- `#` prefix ignored: `["#work"]` same as `["work"]`

### 3. Folder Filtering

**Path prefix matching with subfolder inclusion:**

```json
{"folder": "journal"}            // Matches journal/* and journal/2025/* etc.
{"folder": "journal/2025"}       // Matches only journal/2025/*
```

**Features:**
- Trailing slash automatically normalized
- Includes all subfolders

### 4. Browse Mode

**Without `query`, returns chronological listing:**

```json
{"from": "2025-01-01", "to": "2025-01-31", "folder": "journal"}
```

**Output format (different from search results):**
```text
Entries (2025-01-01 → 2025-01-31) — 12 results

## Daily Note - January 15
Path: journal/2025/2025-01-15.md
Date: 2025-01-15 | Tags: #daily #work
Today I worked on the search filters...

## Weekend Reflection
Path: journal/2025/2025-01-20.md
Date: 2025-01-20 | Tags: #daily #personal
Spent the weekend relaxing...
```

## Examples

### Temporal Queries (Issue #38)

```json
// What did I write last week?
{"from": "last week"}

// Journal entries from January
{"from": "2025-01", "to": "2025-01", "folder": "journal"}

// Entries with anxiety tag from this month
{"from": "2025-02-01", "tags": ["anxiety"]}
```

### Search with Filters

```json
// Search for "rust" in recent entries
{"query": "rust", "from": "2025-01-01"}

// Search for "meeting" in work-tagged journal entries
{"query": "meeting", "folder": "journal", "tags": ["work"]}

// Hybrid search with all filters
{"query": "project", "from": "last month", "tags": ["work"], "folder": "projects"}
```

## Performance Implications

### Over-Fetching Strategy

When filters are active, the tool over-fetches results by 3x to ensure sufficient matches after filtering:

```rust
// Example: limit = 5, filters active
// - Fetches 15 results from search/database
// - Applies filters
// - Returns up to 5 final results
```

**Impact:** Minimal for typical vaults (<1000 notes). May increase query time by ~10-20% when filters are active.

### Caching

Date parsing results are not cached (stateless function). If performance becomes an issue with repeated queries, consider client-side caching of parsed date ranges.

## Testing

### Recommended Test Cases

1. **Backward compatibility:**
   ```json
   {"query": "rust"}  // Should work exactly as before
   ```

2. **Browse mode:**
   ```json
   {"from": "last week"}
   {"tags": ["work"]}
   {"folder": "journal/2025"}
   ```

3. **Search + filters:**
   ```json
   {"query": "meeting", "from": "2025-01-01", "tags": ["work"]}
   ```

4. **Validation:**
   ```json
   {}  // Should error
   {"from": "2025-02-01", "to": "2025-01-01"}  // Should error (from > to)
   {"from": "invalid-date"}  // Should error
   ```

### Test Vault Setup

See [`docs/testing/search-filters-test-vault.md`](./testing/search-filters-test-vault.md) for a ready-to-use test vault setup script.

### Full Test Suite

See [`docs/testing/search-filters-test-suite.md`](./testing/search-filters-test-suite.md) for 68 comprehensive test cases.

## Rollback Plan

If issues arise, rollback is straightforward as the change is additive:

1. **Revert PR #64** to restore previous behavior
2. **Database schema unchanged** - no migrations needed
3. **No data loss** - all data remains intact

**Note:** Rolling back will remove filter functionality but existing queries will continue to work.

## Support

For issues or questions:
- **GitHub Issues:** https://github.com/jpalczewski/kajet/issues
- **Reference Issue:** #38 (feat: add get_entries MCP tool — temporal journal queries)
- **Reference PR:** #64 (feat: temporal and tag/folder filters for search tool)

## Changelog

**Added:**
- Temporal filtering with natural language dates (Polish & English)
- Tag filtering (case-insensitive, AND logic)
- Folder filtering (prefix matching with subfolders)
- Browse mode (temporal document listing without semantic search)
- `DocumentStore::query_documents()` trait method
- Over-fetching strategy for accurate filtered results
- Comprehensive test suite (68 test cases)

**Changed:**
- `SearchRequest.query` field is now optional (`Option<String>`)
- New validation: at least `query` or one filter required
- Search tool description updated to reflect dual modes

**Deprecated:**
- None

**Removed:**
- None

**Fixed:**
- None

**Security:**
- SQL injection consideration documented for LanceDB predicates
- Folder path escaping implemented (single quote handling)
