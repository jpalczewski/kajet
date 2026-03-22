# 📝 Changelog

All notable changes to this project will be documented in this file.

## 0.3.0 (2026-03-22)

### 💥 Breaking Changes

- Roadmap no longer in repo, tracked in GitHub Issues

### ✨ Features

- batch examine multiple paths in one request (#77)
- add explore_connections graph traversal tool (#80)
- add robust find_similar discovery with link exclusion modes (#81)
- add example vault for testing and documentation (#85)
- strip frontmatter from documents.full_text (#87)
- extend SimilarityGraph trait with chunk identity + KJSG v2 serialization
- add LinkGraph trait and InMemoryLinkGraph
- Phase 0 similarity graph infrastructure
- add discover_bridges tool (#88)

### 🐛 Fixes

- UTF-8 safe search preview + panic hook (#83)
- clean up CHANGELOG duplicates from force push
- add kajet-remote to versioned files in knope.toml
- bump kajet-remote version to 0.2.2
- address code review issues in similarity_graph
- code review fixes for link_graph and similarity_graph

## 0.2.2 (2026-02-15)

### ✨ Features

- add edit_tags tool for managing note tags (#61)
- temporal and tag/folder filters for search tool (#64)
- Add tree tool with security fixes (#67)
- add recent_context tool and analytics logs (refs #39)
- selectable remote embedder + embedding settings UI (#71)
- comprehensive web dashboard refactor (#16, #29, #66) (#72)

## 0.2.1 (2026-02-08)

### ✨ Features

- add create_note/edit_note tools with bugfixes and improvements

### 🐛 Fixes

- align changelog header with config and add PR/issue links
- prevent empty frontmatter panic and symlink directory escape
- move test-utils feature to dev-dependencies in writer and indexer
- wrap if expression in ${{ }} to fix YAML parsing
- quote if expression to prevent YAML tag interpretation
- use full semver tag for knope-dev/action
- bump knope-dev/action to v2.1.0
