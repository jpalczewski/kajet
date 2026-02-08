# Search Filters - Test Suite

## Prompt dla agenta testującego

```
You are testing the kajet MCP search tool with new filtering capabilities.

**Your task:**
1. Execute each test case from the list below
2. Verify the output matches expected behavior
3. Note any discrepancies, errors, or unexpected behavior
4. Test both success and error cases
5. Pay attention to edge cases and boundary conditions

**Test environment:**
- Use MCP Inspector or Claude Desktop with kajet MCP server
- Vault should contain sample journal entries with dates, tags, and folders
- Recommended test vault structure:
  ```
  journal/
    2025/
      2025-01-15.md  #daily #work
      2025-01-20.md  #daily #personal
      2025-02-01.md  #daily #work #meeting
    2024/
      2024-12-25.md  #daily #holiday
  projects/
    rust-learning.md  #programming #rust
    obsidian-setup.md  #tools #obsidian
  ```

**For each test:**
1. Note test ID and description
2. Execute the search with specified parameters
3. Record: number of results, format correctness, relevance
4. Flag any issues (errors, wrong results, poor formatting)

**Output format:**
```
TEST-XX: [PASS/FAIL/PARTIAL]
Description: <test description>
Input: <search parameters>
Expected: <expected behavior>
Actual: <what actually happened>
Notes: <any observations>
---
```
```

---

## Test Cases

### Category 1: Validation & Error Handling

#### TEST-01: Empty request (no query, no filters)
**Input:**
```json
{}
```
**Expected:** Error message in English or Polish: "At least 'query' or one filter (from/to/tags/folder) is required"
**Validates:** Request validation logic

---

#### TEST-02: Invalid date format
**Input:**
```json
{"from": "invalid date string"}
```
**Expected:** Error with message indicating unable to parse date, showing the invalid input
**Validates:** Date parser error handling

---

#### TEST-03: Date range validation (from > to)
**Input:**
```json
{"from": "2025-02-01", "to": "2025-01-01"}
```
**Expected:** Error indicating 'from' must be before or equal to 'to'
**Validates:** Date range validation logic

---

### Category 2: Date Parser - ISO 8601 Formats

#### TEST-04: ISO full date as 'from'
**Input:**
```json
{"from": "2025-01-15"}
```
**Expected:** Browse mode output with entries from 2025-01-15 00:00:00 onwards
**Validates:** ISO date parsing, default 'to' behavior (should default to today)

---

#### TEST-05: ISO full date as 'to'
**Input:**
```json
{"to": "2025-01-20"}
```
**Expected:** Browse mode output with entries up to 2025-01-20 23:59:59
**Validates:** ISO date parsing with only 'to' specified

---

#### TEST-06: ISO month format (from)
**Input:**
```json
{"from": "2025-01"}
```
**Expected:** Entries from 2025-01-01 00:00:00 onwards
**Validates:** Month-only format, From bound interpretation (first day)

---

#### TEST-07: ISO month format (to)
**Input:**
```json
{"to": "2025-01"}
```
**Expected:** Entries up to 2025-01-31 23:59:59
**Validates:** Month-only format, To bound interpretation (last day)

---

#### TEST-08: ISO date range
**Input:**
```json
{"from": "2025-01-01", "to": "2025-01-31"}
```
**Expected:** Browse mode with header showing date range, entries from January 2025 only
**Validates:** Full date range filtering

---

#### TEST-09: February leap year handling
**Input:**
```json
{"from": "2024-02", "to": "2024-02"}
```
**Expected:** Entries from 2024-02-01 to 2024-02-29 (leap year)
**Validates:** Leap year calculation in last_day_of_month

---

#### TEST-10: February non-leap year
**Input:**
```json
{"from": "2025-02", "to": "2025-02"}
```
**Expected:** Entries from 2025-02-01 to 2025-02-28 (non-leap year)
**Validates:** Non-leap year calculation

---

### Category 3: Date Parser - Polish Keywords

#### TEST-11: "dzisiaj" (today)
**Input:**
```json
{"from": "dzisiaj"}
```
**Expected:** Entries from today onwards
**Validates:** Polish keyword parsing, case-insensitive

---

#### TEST-12: "wczoraj" (yesterday)
**Input:**
```json
{"from": "wczoraj", "to": "wczoraj"}
```
**Expected:** Entries from yesterday only
**Validates:** Polish keyword for yesterday

---

#### TEST-13: "zeszły tydzień" - From bound
**Input:**
```json
{"from": "zeszły tydzień", "to": "zeszły tydzień"}
```
**Expected:** Entries from last Monday to last Sunday
**Validates:** Polish relative week, proper week boundaries

---

#### TEST-14: "zeszły miesiąc" - Full range
**Input:**
```json
{"from": "zeszły miesiąc", "to": "zeszły miesiąc"}
```
**Expected:** All entries from previous month (1st to last day)
**Validates:** Polish relative month

---

#### TEST-15: "zeszły rok" - Full range
**Input:**
```json
{"from": "zeszły rok", "to": "zeszły rok"}
```
**Expected:** All entries from previous year (Jan 1 to Dec 31)
**Validates:** Polish relative year

---

#### TEST-16: Polish month name "w styczniu"
**Input:**
```json
{"from": "w styczniu", "to": "w styczniu"}
```
**Expected:** All entries from January of current year
**Validates:** Polish month names with "w" prefix

---

#### TEST-17: Polish month name "we wrześniu" (special form)
**Input:**
```json
{"from": "we wrześniu"}
```
**Expected:** Entries from September 1st onwards (current year)
**Validates:** Special Polish form "we" instead of "w"

---

#### TEST-18: Case insensitivity - Polish
**Input:**
```json
{"from": "DZISIAJ"}
```
**Expected:** Same result as "dzisiaj"
**Validates:** Case-insensitive parsing

---

### Category 4: Date Parser - English Keywords

#### TEST-19: "today"
**Input:**
```json
{"from": "today"}
```
**Expected:** Entries from today onwards
**Validates:** English keyword parsing

---

#### TEST-20: "yesterday"
**Input:**
```json
{"from": "yesterday", "to": "yesterday"}
```
**Expected:** Entries from yesterday only
**Validates:** English yesterday keyword

---

#### TEST-21: "last week" - From bound
**Input:**
```json
{"from": "last week", "to": "last week"}
```
**Expected:** Entries from last Monday to last Sunday
**Validates:** English relative week

---

#### TEST-22: "last month" - Full range
**Input:**
```json
{"from": "last month", "to": "last month"}
```
**Expected:** All entries from previous month
**Validates:** English relative month

---

#### TEST-23: "last year" - Full range
**Input:**
```json
{"from": "last year", "to": "last year"}
```
**Expected:** All entries from previous year
**Validates:** English relative year

---

#### TEST-24: English month name "in january"
**Input:**
```json
{"from": "in january", "to": "in january"}
```
**Expected:** All entries from January of current year
**Validates:** English month names with "in" prefix

---

#### TEST-25: English month name "in december"
**Input:**
```json
{"to": "in december"}
```
**Expected:** All entries up to December 31st of current year
**Validates:** English December, last day calculation

---

### Category 5: Tag Filtering

#### TEST-26: Single tag filter (browse mode)
**Input:**
```json
{"tags": ["daily"]}
```
**Expected:** Browse mode showing only entries with #daily tag
**Validates:** Tag filtering in browse mode

---

#### TEST-27: Multiple tags - ALL required (browse mode)
**Input:**
```json
{"tags": ["daily", "work"]}
```
**Expected:** Only entries that have BOTH #daily AND #work tags
**Validates:** Multiple tag filtering (intersection, not union)

---

#### TEST-28: Tag with # prefix (should be ignored)
**Input:**
```json
{"tags": ["#daily", "work"]}
```
**Expected:** Same result as ["daily", "work"]
**Validates:** # prefix normalization

---

#### TEST-29: Tag case insensitivity
**Input:**
```json
{"tags": ["DAILY", "Work"]}
```
**Expected:** Matches tags regardless of case (daily, Daily, DAILY all match)
**Validates:** Case-insensitive tag comparison

---

#### TEST-30: Tag + date range (browse mode)
**Input:**
```json
{"from": "2025-01", "to": "2025-01", "tags": ["work"]}
```
**Expected:** Browse mode with January 2025 entries that have #work tag
**Validates:** Combined date + tag filtering

---

#### TEST-31: Tag filter with no matches
**Input:**
```json
{"tags": ["nonexistent-tag"]}
```
**Expected:** Browse mode with "No entries found matching filters" message
**Validates:** Empty result handling in browse mode

---

### Category 6: Folder Filtering

#### TEST-32: Folder prefix filter (browse mode)
**Input:**
```json
{"folder": "journal"}
```
**Expected:** Browse mode showing only entries from journal/ folder and subfolders
**Validates:** Folder prefix matching

---

#### TEST-33: Folder with trailing slash
**Input:**
```json
{"folder": "journal/"}
```
**Expected:** Same as TEST-32
**Validates:** Trailing slash normalization

---

#### TEST-34: Nested folder filter
**Input:**
```json
{"folder": "journal/2025"}
```
**Expected:** Only entries from journal/2025/ subfolder
**Validates:** Nested folder filtering

---

#### TEST-35: Folder + tag filter
**Input:**
```json
{"folder": "journal", "tags": ["work"]}
```
**Expected:** Browse mode with journal entries that have #work tag
**Validates:** Combined folder + tag filtering

---

#### TEST-36: Folder + date + tag (all filters)
**Input:**
```json
{"from": "2025-01-01", "to": "2025-01-31", "folder": "journal", "tags": ["work"]}
```
**Expected:** January 2025 journal entries with #work tag
**Validates:** All filter types combined

---

### Category 7: Search Mode (with query)

#### TEST-37: Search with date filter
**Input:**
```json
{"query": "meeting", "from": "2025-01-01"}
```
**Expected:** Search results for "meeting" filtered to entries from 2025-01-01 onwards
**Validates:** Search mode with date post-filtering

---

#### TEST-38: Search with tag filter
**Input:**
```json
{"query": "rust", "tags": ["programming"]}
```
**Expected:** Search results for "rust" filtered to entries with #programming tag
**Validates:** Search mode with tag post-filtering

---

#### TEST-39: Search with folder filter
**Input:**
```json
{"query": "obsidian", "folder": "projects"}
```
**Expected:** Search results for "obsidian" from projects/ folder only
**Validates:** Search mode with folder post-filtering

---

#### TEST-40: Search with all filters
**Input:**
```json
{"query": "meeting", "from": "2025-01-01", "tags": ["work"], "folder": "journal"}
```
**Expected:** Search results for "meeting" with all filters applied
**Validates:** Search mode with combined filtering

---

#### TEST-41: Search mode - no matches after filtering
**Input:**
```json
{"query": "rust", "tags": ["nonexistent-tag"]}
```
**Expected:** Search finds results for "rust" but all are filtered out, returns "No results found"
**Validates:** Empty results after post-filtering

---

#### TEST-42: Search mode with mode parameter
**Input:**
```json
{"query": "obsidian", "mode": "vector", "tags": ["tools"]}
```
**Expected:** Vector search results for "obsidian" filtered by #tools tag
**Validates:** Search mode parameter still works with filters

---

#### TEST-43: Search mode with limit
**Input:**
```json
{"query": "daily", "limit": 3, "tags": ["work"]}
```
**Expected:** Maximum 3 results matching "daily" with #work tag
**Validates:** Limit parameter with filtering (should over-fetch then limit)

---

### Category 8: Browse Mode Output Format

#### TEST-44: Browse mode header with dates
**Input:**
```json
{"from": "2025-01-01", "to": "2025-01-31"}
```
**Expected:** Header showing "Entries (2025-01-01 → 2025-01-31) — X results"
**Validates:** Browse mode header formatting

---

#### TEST-45: Browse mode header without dates
**Input:**
```json
{"folder": "journal"}
```
**Expected:** Header showing "Entries — X results" (no date range)
**Validates:** Browse mode header without date range

---

#### TEST-46: Browse mode entry format - with tags
**Input:**
```json
{"from": "2025-01-15", "to": "2025-01-15"}
```
**Expected:** Each entry shows: title, path, date, tags (formatted as #tag1 #tag2)
**Validates:** Entry formatting with tags

---

#### TEST-47: Browse mode entry format - no tags
**Input:**
```json
{"folder": "projects"}
```
**Expected:** Entries show "Tags: none" for notes without tags
**Validates:** Entry formatting without tags

---

#### TEST-48: Browse mode snippet truncation
**Input:**
```json
{"folder": "journal"}
```
**Expected:** Long content truncated to ~200 chars with "..." suffix
**Validates:** Content snippet length limit

---

#### TEST-49: Browse mode chronological order
**Input:**
```json
{"from": "2025-01-01", "to": "2025-01-31"}
```
**Expected:** Results sorted chronologically (oldest first)
**Validates:** Chronological sorting in browse mode

---

### Category 9: Search Mode Output Format

#### TEST-50: Search mode output unchanged
**Input:**
```json
{"query": "rust"}
```
**Expected:** Standard search results format (not browse format)
**Validates:** Search mode uses format_results(), not format_entries()

---

#### TEST-51: Search mode with filters - still uses search format
**Input:**
```json
{"query": "meeting", "from": "2025-01-01"}
```
**Expected:** Standard search results format with filtered results
**Validates:** Search mode format preserved even with filters

---

### Category 10: Edge Cases & Boundaries

#### TEST-52: Empty vault / no indexed documents
**Input:**
```json
{"from": "2025-01-01"}
```
**Expected:** "No entries found matching filters" (not an error)
**Validates:** Graceful handling of empty vault

---

#### TEST-53: Very old date
**Input:**
```json
{"from": "1990-01-01"}
```
**Expected:** All indexed entries (or browse_no_results if nothing that old)
**Validates:** Old date handling

---

#### TEST-54: Future date
**Input:**
```json
{"from": "2030-01-01"}
```
**Expected:** No results (browse_no_results message)
**Validates:** Future date handling

---

#### TEST-55: Same date for from and to
**Input:**
```json
{"from": "2025-01-15", "to": "2025-01-15"}
```
**Expected:** Entries from that specific day only (00:00:00 to 23:59:59)
**Validates:** Single-day range

---

#### TEST-56: Tag that exists but no date matches
**Input:**
```json
{"tags": ["daily"], "from": "1990-01-01", "to": "1990-12-31"}
```
**Expected:** No results (tag exists but date range has none)
**Validates:** Combined filter with no matches

---

#### TEST-57: Folder that exists but no tag matches
**Input:**
```json
{"folder": "journal", "tags": ["nonexistent-tag"]}
```
**Expected:** No results
**Validates:** Combined filter with no matches

---

#### TEST-58: Very long tag list
**Input:**
```json
{"tags": ["tag1", "tag2", "tag3", "tag4", "tag5", "tag6", "tag7", "tag8"]}
```
**Expected:** Only documents with ALL 8 tags (likely no results, but should not error)
**Validates:** Multiple tag intersection logic

---

#### TEST-59: Special characters in folder path
**Input:**
```json
{"folder": "my-notes/2025"}
```
**Expected:** Matches folder with hyphen in name
**Validates:** Special character handling in folder names

---

#### TEST-60: Whitespace in date string
**Input:**
```json
{"from": "  2025-01-15  "}
```
**Expected:** Parsed correctly (whitespace trimmed)
**Validates:** Whitespace trimming in date parser

---

### Category 11: Dashboard Integration

#### TEST-61: Browse mode sends QueryEvent
**Input:**
```json
{"from": "last week", "tags": ["work"]}
```
**Expected:** Dashboard receives synthetic QueryEvent like "[browse] from:2025-01-27 to:2025-02-03 folder:* tags:work"
**Validates:** Dashboard event emission in browse mode
**Note:** This requires checking dashboard logs or WebSocket events

---

#### TEST-62: Search mode with filters sends normal QueryEvent
**Input:**
```json
{"query": "meeting", "from": "2025-01-01"}
```
**Expected:** Dashboard receives QueryEvent with query="meeting" (not synthetic browse event)
**Validates:** Normal event emission in search mode with filters
**Note:** Check dashboard query log

---

### Category 12: Performance & Over-fetching

#### TEST-63: Search mode over-fetches with filters
**Input:**
```json
{"query": "daily", "limit": 5, "tags": ["work"]}
```
**Expected:** Should over-fetch (15 results = 3x limit), filter, then return max 5
**Validates:** Over-fetching logic when filters active
**Note:** Check logs for actual fetch limit used

---

#### TEST-64: Browse mode applies limit correctly
**Input:**
```json
{"folder": "journal", "limit": 3}
```
**Expected:** Exactly 3 results (or fewer if vault has less)
**Validates:** Limit application in browse mode

---

### Category 13: Internationalization (i18n)

#### TEST-65: Error messages in English
**Input (with locale=en):**
```json
{}
```
**Expected:** Error message in English
**Validates:** English i18n keys

---

#### TEST-66: Error messages in Polish
**Input (with locale=pl):**
```json
{}
```
**Expected:** Error message in Polish
**Validates:** Polish i18n keys

---

#### TEST-67: Browse mode output in English
**Input (with locale=en):**
```json
{"from": "2025-01-01"}
```
**Expected:** Headers and labels in English ("Entries", "Path:", "Date:", "Tags:")
**Validates:** English browse mode formatting

---

#### TEST-68: Browse mode output in Polish
**Input (with locale=pl):**
```json
{"from": "2025-01-01"}
```
**Expected:** Headers and labels in Polish ("Wpisy", "Ścieżka:", "Data:", "Tagi:")
**Validates:** Polish browse mode formatting

---

## Success Criteria

- ✅ All validation tests (TEST-01 to TEST-03) pass with correct error messages
- ✅ All date parser tests (TEST-04 to TEST-60) produce correct date ranges
- ✅ Tag filtering is case-insensitive and handles # prefix (TEST-26 to TEST-31)
- ✅ Folder filtering works with nested paths (TEST-32 to TEST-36)
- ✅ Search mode with filters produces filtered results (TEST-37 to TEST-43)
- ✅ Browse mode output format is correct (TEST-44 to TEST-49)
- ✅ Search mode output format is preserved (TEST-50 to TEST-51)
- ✅ Edge cases handled gracefully (TEST-52 to TEST-60)
- ✅ Dashboard integration works (TEST-61 to TEST-62)
- ✅ Performance optimizations apply (TEST-63 to TEST-64)
- ✅ i18n works for both languages (TEST-65 to TEST-68)

## Notes

- Tests assume a populated test vault with appropriate structure
- Some tests require checking logs or dashboard for full validation
- Date-dependent tests (today, yesterday, etc.) will have different results depending on when run
- Consider running tests in both English and Polish locales
