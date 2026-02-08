# Test Vault Setup for Search Filters

## Quick Setup Script

```bash
# Create test vault structure
mkdir -p ~/test-vault-kajet/{journal/{2024,2025},projects,notes}
cd ~/test-vault-kajet

# Journal entries
cat > journal/2025/2025-01-15.md << 'EOF'
---
title: Daily Note - January 15
tags:
  - daily
  - work
created: 2025-01-15T08:00:00
---

# Daily Note - January 15

Started working on the new Rust project today. Had a productive morning reviewing the codebase and planning the architecture.

## Tasks
- [ ] Review Rust documentation
- [x] Set up project structure
- [ ] Write initial tests

Very excited about this project!
EOF

cat > journal/2025/2025-01-20.md << 'EOF'
---
title: Weekend Reflection
tags:
  - daily
  - personal
  - reflection
created: 2025-01-20T10:00:00
---

# Weekend Reflection

Spent the weekend relaxing and reading. Had some time to think about my goals for the year.

Need to focus more on:
- Personal projects
- Learning new technologies
- Work-life balance

Feeling refreshed and ready for the week ahead.
EOF

cat > journal/2025/2025-02-01.md << 'EOF'
---
title: February Kickoff
tags:
  - daily
  - work
  - meeting
created: 2025-02-01T09:00:00
---

# February Kickoff

New month, new goals! Had our monthly team meeting this morning to discuss Q1 priorities.

## Meeting Notes
- Project deadline moved to March 15
- New team member joining next week
- Focus on code quality and documentation

Action items: Update project roadmap, schedule 1-on-1s with team.

Lots of Rust work ahead - need to finish the authentication module this week.
EOF

cat > journal/2024/2024-12-25.md << 'EOF'
---
title: Holiday Reflections
tags:
  - daily
  - holiday
  - personal
created: 2024-12-25T12:00:00
---

# Holiday Reflections

Merry Christmas! Spending time with family today. Looking back at 2024 - it's been quite a year.

Grateful for:
- New opportunities at work
- Personal growth
- Amazing friends and family

Looking forward to 2025!
EOF

# Projects folder
cat > projects/rust-learning.md << 'EOF'
---
title: Rust Learning Path
tags:
  - programming
  - rust
  - learning
created: 2025-01-10T14:00:00
---

# Rust Learning Path

My journey learning Rust programming language.

## Resources
- The Rust Book (official)
- Rust by Example
- Exercism Rust track
- rustlings exercises

## Topics to Master
1. Ownership and borrowing
2. Lifetimes
3. Traits and generics
4. Async/await
5. Macros

Making good progress! The ownership model is starting to click. Working on a small CLI tool to practice.
EOF

cat > projects/obsidian-setup.md << 'EOF'
---
title: Obsidian Setup and Configuration
tags:
  - tools
  - obsidian
  - productivity
created: 2025-01-05T16:00:00
---

# Obsidian Setup and Configuration

My Obsidian vault setup and preferred plugins.

## Core Plugins
- Daily notes
- Templates
- Backlinks
- Graph view

## Community Plugins
- Dataview
- Calendar
- Advanced Tables
- Kanban

## Workflow
Using Obsidian for personal knowledge management and journaling. Really enjoying the wikilink feature and bidirectional linking.

Setting up kajet MCP server for semantic search - this is going to be a game changer!
EOF

cat > projects/kajet-testing.md << 'EOF'
---
title: Kajet Testing Notes
tags:
  - testing
  - kajet
  - mcp
  - work
created: 2025-02-08T11:00:00
---

# Kajet Testing Notes

Testing the new search filters feature for kajet MCP server.

## Features to Test
- Date filtering (from/to)
- Tag filtering (multiple tags, case insensitive)
- Folder filtering
- Browse mode vs search mode
- Combined filters

## Test Cases
Need to verify:
1. ISO date formats (2025-01-15, 2025-01)
2. Polish keywords (dzisiaj, zeszły tydzień, w styczniu)
3. English keywords (today, last week, in january)
4. Tag combinations
5. Empty results handling

This is an exciting new feature that will make temporal queries much easier!
EOF

# Notes folder
cat > notes/anxiety-management.md << 'EOF'
---
title: Anxiety Management Techniques
tags:
  - mental-health
  - anxiety
  - reflection
  - personal
created: 2025-01-18T20:00:00
---

# Anxiety Management Techniques

Collection of techniques that help me manage anxiety.

## Breathing Exercises
- 4-7-8 breathing
- Box breathing
- Deep belly breathing

## Physical Activities
- Walking in nature
- Yoga
- Light exercise

## Mental Techniques
- Mindfulness meditation
- Journaling
- Talking to friends

## Notes
Remember: It's okay to not be okay. Progress over perfection. Small steps count.

Regular reflection helps track what works best for me.
EOF

cat > notes/meeting-notes-template.md << 'EOF'
---
title: Meeting Notes Template
tags:
  - template
  - work
  - meeting
created: 2025-01-12T09:30:00
---

# Meeting Notes Template

Standard template for meeting notes.

## Meeting Info
- Date:
- Attendees:
- Duration:

## Agenda
1. Topic 1
2. Topic 2
3. Topic 3

## Discussion Points
- Point 1
- Point 2

## Action Items
- [ ] Action 1 (Owner: X, Due: Y)
- [ ] Action 2 (Owner: X, Due: Y)

## Follow-up
Next meeting scheduled for: [date]
EOF

echo "Test vault created at ~/test-vault-kajet"
echo ""
echo "To use with kajet:"
echo "  kajet --vault ~/test-vault-kajet"
echo ""
echo "Or configure in Claude Desktop config.json:"
echo '  "args": ["--vault", "~/test-vault-kajet"]'
```

## Vault Statistics

After setup, the vault will contain:

- **Total notes:** 9
- **Date range:** 2024-12-25 to 2025-02-08
- **Folders:**
  - `journal/2024/` (1 note)
  - `journal/2025/` (3 notes)
  - `projects/` (3 notes)
  - `notes/` (2 notes)
- **Tags:**
  - `daily` (4 notes)
  - `work` (5 notes)
  - `personal` (3 notes)
  - `reflection` (2 notes)
  - `meeting` (2 notes)
  - `programming` (1 note)
  - `rust` (1 note)
  - `tools` (1 note)
  - `obsidian` (1 note)
  - `testing` (1 note)
  - `kajet` (1 note)
  - `mcp` (1 note)
  - `holiday` (1 note)
  - `learning` (1 note)
  - `template` (1 note)
  - `mental-health` (1 note)
  - `anxiety` (1 note)

## Coverage for Test Cases

This vault structure enables testing:

✅ **Date ranges:**
- Single day (2025-01-15)
- Month range (2025-01)
- Year transitions (2024-12 to 2025-01)
- Multiple months (2025-01 to 2025-02)

✅ **Tag combinations:**
- Single tags (`daily`, `work`, `personal`)
- Multiple tags (`daily` + `work`, `work` + `meeting`)
- Rare tags for testing no-match scenarios

✅ **Folder structure:**
- Top-level folders (`journal`, `projects`, `notes`)
- Nested folders (`journal/2024`, `journal/2025`)
- Mixed content types

✅ **Content variety:**
- Short notes
- Long notes with multiple sections
- Notes with tasks
- Template notes
- Notes with various formatting

## Quick Test Queries

After indexing, try these quick tests:

```json
// Browse last week
{"from": "last week"}

// Browse January 2025
{"from": "2025-01", "to": "2025-01"}

// All work-tagged entries
{"tags": ["work"]}

// Work entries from January
{"from": "2025-01-01", "to": "2025-01-31", "tags": ["work"]}

// Journal folder only
{"folder": "journal"}

// Journal 2025 subfolder
{"folder": "journal/2025"}

// Multiple tags
{"tags": ["daily", "work"]}

// Search with filter
{"query": "rust", "tags": ["programming"]}

// Search in folder
{"query": "meeting", "folder": "journal"}
```

## Cleanup

```bash
# Remove test vault
rm -rf ~/test-vault-kajet
```
