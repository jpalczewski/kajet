# Search Filters - Quick Reference Card

## Priority Test Cases (Top 20)

Quick checklist for essential functionality validation.

### 🔴 Critical (Must Pass)

#### 1. Empty Request Validation
```json
{}
```
**Expected:** Error - "At least query or one filter required"

---

#### 2. Browse Mode - Date Range (ISO)
```json
{"from": "2025-01-01", "to": "2025-01-31"}
```
**Expected:** January 2025 entries, browse format, chronological order

---

#### 3. Browse Mode - Single Tag
```json
{"tags": ["work"]}
```
**Expected:** All entries with #work tag

---

#### 4. Browse Mode - Multiple Tags (AND logic)
```json
{"tags": ["daily", "work"]}
```
**Expected:** Only entries with BOTH tags (not either/or)

---

#### 5. Browse Mode - Folder Filter
```json
{"folder": "journal/2025"}
```
**Expected:** Only entries from journal/2025/ subfolder

---

#### 6. Search Mode with Date Filter
```json
{"query": "rust", "from": "2025-01-01"}
```
**Expected:** Search results for "rust" from 2025-01-01 onwards, search format (not browse)

---

#### 7. Search Mode with Tag Filter
```json
{"query": "meeting", "tags": ["work"]}
```
**Expected:** Search results filtered to #work tagged entries

---

#### 8. Date Range Validation
```json
{"from": "2025-02-01", "to": "2025-01-01"}
```
**Expected:** Error - "from must be before or equal to to"

---

#### 9. Invalid Date Format
```json
{"from": "invalid-date"}
```
**Expected:** Error - "Unable to parse date"

---

#### 10. Polish Keyword - "zeszły tydzień"
```json
{"from": "zeszły tydzień", "to": "zeszły tydzień"}
```
**Expected:** Last Monday to Sunday entries

---

### 🟡 High Priority

#### 11. English Keyword - "last month"
```json
{"from": "last month", "to": "last month"}
```
**Expected:** Previous month entries (1st to last day)

---

#### 12. ISO Month Format
```json
{"from": "2025-01", "to": "2025-01"}
```
**Expected:** All January 2025 entries (1st to 31st)

---

#### 13. Combined Filters (All Types)
```json
{"from": "2025-01-01", "to": "2025-01-31", "folder": "journal", "tags": ["work"]}
```
**Expected:** January journal entries with #work tag

---

#### 14. Tag Case Insensitivity
```json
{"tags": ["WORK", "Daily"]}
```
**Expected:** Matches tags regardless of case

---

#### 15. Tag # Prefix Ignored
```json
{"tags": ["#work", "daily"]}
```
**Expected:** Same as ["work", "daily"]

---

### 🟢 Medium Priority

#### 16. Empty Results (Browse)
```json
{"tags": ["nonexistent-tag"]}
```
**Expected:** "No entries found" message (not error)

---

#### 17. Empty Results (Search with Filters)
```json
{"query": "test", "tags": ["nonexistent-tag"]}
```
**Expected:** "No results found" message

---

#### 18. Default 'to' When Only 'from' Provided
```json
{"from": "2025-01-15"}
```
**Expected:** From 2025-01-15 to today (inclusive)

---

#### 19. Browse Output Format
```json
{"from": "2025-01-15", "to": "2025-01-15"}
```
**Expected:**
- Header: "Entries (2025-01-15 → 2025-01-15) — X results"
- Each entry: Title, Path, Date, Tags, Snippet (~200 chars)

---

#### 20. Search Output Format Preserved
```json
{"query": "rust", "tags": ["programming"]}
```
**Expected:** Standard search format (score, path, section, content), not browse format

---

## Quick Command Reference

### Run Tests in MCP Inspector
```bash
# Start kajet with test vault
kajet --vault ~/test-vault-kajet

# In another terminal, start MCP Inspector
npx @modelcontextprotocol/inspector
```

### Run Specific Test Package
```bash
# Date parser tests only
cargo nextest run -p kajet-mcp -E 'test(date_parser)'

# Format tests only
cargo nextest run -p kajet-mcp -E 'test(format_entries)'

# All MCP tests
cargo nextest run -p kajet-mcp
```

---

## Expected Behaviors Summary

### Browse Mode (no query)
- Uses `format_entries()` output
- Header with date range and count
- Chronological order (oldest first)
- Entries show: title, path, date, tags, snippet
- Synthetic dashboard event: `[browse] from:X to:Y folder:Z tags:W`

### Search Mode (with query)
- Uses `format_results()` output
- Standard search format with scores
- Respects mode parameter (hybrid/vector/fts)
- Post-filters by date/tags/folder
- Over-fetches 3x when filters active
- Normal dashboard event with query text

### Tag Filtering
- Case-insensitive comparison
- # prefix stripped before comparison
- ALL tags required (AND logic, not OR)
- Stored tags don't have # prefix in database

### Date Filtering
- from: 00:00:00 of specified day
- to: 23:59:59 of specified day
- Month-only: From=1st day, To=last day
- Missing to: defaults to end of today when from is set

### Folder Filtering
- Prefix match with trailing /
- Includes all subfolders
- "journal" matches "journal/", "journal/2025/", etc.

---

## Success Checklist

Use this for quick validation:

- [ ] Validation errors work (TEST-1, 8, 9)
- [ ] Browse mode outputs correct format (TEST-2, 19)
- [ ] Search mode preserves format (TEST-6, 20)
- [ ] Tags filter correctly (TEST-3, 4, 14, 15)
- [ ] Folders filter correctly (TEST-5, 13)
- [ ] Date parsing works (ISO + keywords) (TEST-10, 11, 12)
- [ ] Combined filters work (TEST-13)
- [ ] Empty results handled gracefully (TEST-16, 17)
- [ ] Default behaviors correct (TEST-18)

---

## Common Issues to Watch For

⚠️ **Date Parsing:**
- Watch for timezone issues (should use UTC)
- Leap year handling (Feb 29)
- Month boundary (Jan 31 → Feb 1)

⚠️ **Tag Filtering:**
- Case sensitivity issues
- # prefix not normalized
- OR logic instead of AND logic

⚠️ **Folder Filtering:**
- Missing trailing / causing wrong matches
- Not matching subfolders

⚠️ **Output Format:**
- Search mode using browse format by mistake
- Browse mode using search format by mistake
- Truncation not working (too long snippets)

⚠️ **Error Messages:**
- Not internationalized
- Wrong locale used
- Missing error messages

---

## Report Template

```
## Test Run Report

**Date:** YYYY-MM-DD
**Tester:** [Name]
**Environment:** MCP Inspector / Claude Desktop
**Vault:** Test vault / Production vault

### Results Summary
- Total tests: X
- Passed: X
- Failed: X
- Partial: X

### Critical Issues
1. [Issue description]
   - Test case: TEST-XX
   - Expected: [...]
   - Actual: [...]
   - Severity: Critical/High/Medium/Low

### Notes
[Any additional observations]

### Recommendation
[ ] Ready to merge
[ ] Needs fixes before merge
[ ] Needs discussion
```
