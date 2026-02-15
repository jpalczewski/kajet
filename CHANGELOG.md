# 📝 Changelog

All notable changes to this project will be documented in this file.# 📝 Changelog

All notable changes to this project will be documented in this file.# 📝 Changelog

All notable changes to this project will be documented in this file.# 📝 Changelog

All notable changes to this project will be documented in this file.# 📝 Changelog

All notable changes to this project will be documented in this file.# 📝 Changelog

All notable changes to this project will be documented in this file.# 📝 Changelog

All notable changes to this project will be documented in this file.# 📝 Changelog

All notable changes to this project will be documented in this file.
## `kajet` - [0.2.0](https://github.com/jpalczewski/kajet/releases/tag/v0.2.0) - 2026-02-08

### ♻️ Changed

- start dashboard before indexing for live log monitoring

- split monolith into Cargo workspace with 4 crates


### ✨ Added

- normalize filesystem paths to NFC for consistent Unicode handling

- add MCP `examine` tool for document inspection

- filter link-only noise chunks from indexing

- extract wikilinks from chunks with link resolution and backlinks

- move LanceDB out of cloud-synced vaults for fast vector search

- add tracing instrumentation to search pipeline

- dual-sink logging system with file and dashboard output

- add config management system with dashboard settings

- add Svelte frontend source and build config

- integrate new architecture with MCP tools and file watcher

- add incremental indexer with async pipeline and file watcher

- add layered config and i18n (EN/PL)

- migrate from FastEmbed to Candle for embedding model


### 🐛 Fixed

- add pull-requests read permission to release-plz release job

- improve release-plz workflow to prevent premature releases

- remove unsupported 'branch' field from release-plz config

- update release-plz to use develop branch

- start MCP server before indexing to prevent timeout on large vaults

- show "indexing in progress" instead of "no results" during startup

- build frontend with Deno before Rust compilation in CI

- add autotools for lzma-sys static build in CI

- install system deps (liblzma-dev, protobuf) in CI

- disable crates.io publish and fix GitHub URL in release-plz

- add pull-requests permission to release job

- release-plz changelog seed and workflow permissions


### 📚 Documentation

- update CLAUDE.md with db_path architecture and LanceDB FTS gotcha

- add frontend build prerequisite to CLAUDE.md

- update README with features, motivation, and current stack

- update CLAUDE.md to reflect workspace architecture


## `kajet-web` - [0.2.0](https://github.com/jpalczewski/kajet/releases/tag/kajet-web-v0.2.0) - 2026-02-08

### ♻️ Changed

- start dashboard before indexing for live log monitoring

- split monolith into Cargo workspace with 4 crates


### ✨ Added

- move LanceDB out of cloud-synced vaults for fast vector search

- dual-sink logging system with file and dashboard output

- add config management system with dashboard settings

- integrate new architecture with MCP tools and file watcher


### 🐛 Fixed

- show "indexing in progress" instead of "no results" during startup

- return empty results when chunks table doesn't exist yet


## `kajet-mcp` - [0.2.0](https://github.com/jpalczewski/kajet/releases/tag/kajet-mcp-v0.2.0) - 2026-02-08

### ♻️ Changed

- split monolith into Cargo workspace with 4 crates


### ✨ Added

- add MCP `examine` tool for document inspection

- extract wikilinks from chunks with link resolution and backlinks

- move LanceDB out of cloud-synced vaults for fast vector search

- add config management system with dashboard settings

- integrate new architecture with MCP tools and file watcher


### 🐛 Fixed

- add missing kajet-mcp README


## `kajet-indexer` - [0.2.0](https://github.com/jpalczewski/kajet/releases/tag/kajet-indexer-v0.2.0) - 2026-02-08

### ⚡ Performance

- use mtime pre-filter in change detection to avoid slow file reads on iCloud


### ✨ Added

- normalize filesystem paths to NFC for consistent Unicode handling

- extract wikilinks from chunks with link resolution and backlinks

- dual-sink logging system with file and dashboard output

- add incremental indexer with async pipeline and file watcher


### 🐛 Fixed

- use longer test content to pass min_content_chars filter

- deadlock in indexing pipeline when file count exceeds channel buffer


## `kajet-backend` - [0.2.0](https://github.com/jpalczewski/kajet/releases/tag/kajet-backend-v0.2.0) - 2026-02-08

### ♻️ Changed

- simplify vector field definition and formatting in LanceVectorStore

- split monolith into Cargo workspace with 4 crates


### ⚡ Performance

- use mtime pre-filter in change detection to avoid slow file reads on iCloud


### ✨ Added

- normalize filesystem paths to NFC for consistent Unicode handling

- add MCP `examine` tool for document inspection

- extract wikilinks from chunks with link resolution and backlinks

- move LanceDB out of cloud-synced vaults for fast vector search

- add tracing instrumentation to search pipeline

- add config management system with dashboard settings

- add hybrid search, DocumentStore trait, and content hashing


### 🐛 Fixed

- show "indexing in progress" instead of "no results" during startup

- return empty results when chunks table doesn't exist yet

- use floor_char_boundary for FTS snippet to avoid panic on multi-byte UTF-8

- move macOS target deps to end of Cargo.toml

- auto-migrate old chunks table schema on first upsert


## `kajet-core` - [0.2.0](https://github.com/jpalczewski/kajet/releases/tag/kajet-core-v0.2.0) - 2026-02-08

### ♻️ Changed

- start dashboard before indexing for live log monitoring

- extract parser into dedicated crate with chunking improvements

- split monolith into Cargo workspace with 4 crates


### ⚡ Performance

- use mtime pre-filter in change detection to avoid slow file reads on iCloud


### ✨ Added

- add MCP `examine` tool for document inspection

- extract wikilinks from chunks with link resolution and backlinks

- move LanceDB out of cloud-synced vaults for fast vector search

- add tracing instrumentation to search pipeline

- dual-sink logging system with file and dashboard output

- add config management system with dashboard settings

- integrate new architecture with MCP tools and file watcher

- add hybrid search, DocumentStore trait, and content hashing


### 🐛 Fixed

- filter noisy third-party logs (LanceDB, Arrow, ignore) from output

- show "indexing in progress" instead of "no results" during startup


## `kajet-parser` - [0.2.0](https://github.com/jpalczewski/kajet/releases/tag/kajet-parser-v0.2.0) - 2026-02-08

### ♻️ Changed

- extract parser into dedicated crate with chunking improvements


### ✨ Added

- normalize filesystem paths to NFC for consistent Unicode handling

- filter link-only noise chunks from indexing

- extract wikilinks from chunks with link resolution and backlinks


### 📚 Documentation

- add kajet-parser README with chunking and filtering docs


## `kajet` - [0.2.0](https://github.com/jpalczewski/kajet/releases/tag/kajet-v0.2.0) - 2026-02-08

### Added

- normalize filesystem paths to NFC for consistent Unicode handling

- add MCP `examine` tool for document inspection

- filter link-only noise chunks from indexing

- extract wikilinks from chunks with link resolution and backlinks

- move LanceDB out of cloud-synced vaults for fast vector search

- add tracing instrumentation to search pipeline

- dual-sink logging system with file and dashboard output

- add config management system with dashboard settings

- add Svelte frontend source and build config

- integrate new architecture with MCP tools and file watcher

- add incremental indexer with async pipeline and file watcher

- add layered config and i18n (EN/PL)

- migrate from FastEmbed to Candle for embedding model


### Changed

- start dashboard before indexing for live log monitoring

- split monolith into Cargo workspace with 4 crates


### Documentation

- update CLAUDE.md with db_path architecture and LanceDB FTS gotcha

- add frontend build prerequisite to CLAUDE.md

- update README with features, motivation, and current stack

- update CLAUDE.md to reflect workspace architecture


### Fixed

- remove unsupported 'branch' field from release-plz config

- update release-plz to use develop branch

- start MCP server before indexing to prevent timeout on large vaults

- show "indexing in progress" instead of "no results" during startup

- build frontend with Deno before Rust compilation in CI

- add autotools for lzma-sys static build in CI

- install system deps (liblzma-dev, protobuf) in CI

- disable crates.io publish and fix GitHub URL in release-plz

- add pull-requests permission to release job

- release-plz changelog seed and workflow permissions


## `kajet-web` - [0.2.0](https://github.com/jpalczewski/kajet/releases/tag/kajet-web-v0.2.0) - 2026-02-08

### Added

- move LanceDB out of cloud-synced vaults for fast vector search

- dual-sink logging system with file and dashboard output

- add config management system with dashboard settings

- integrate new architecture with MCP tools and file watcher


### Changed

- start dashboard before indexing for live log monitoring

- split monolith into Cargo workspace with 4 crates


### Fixed

- show "indexing in progress" instead of "no results" during startup

- return empty results when chunks table doesn't exist yet


## `kajet-mcp` - [0.2.0](https://github.com/jpalczewski/kajet/releases/tag/kajet-mcp-v0.2.0) - 2026-02-08

### Added

- add MCP `examine` tool for document inspection

- extract wikilinks from chunks with link resolution and backlinks

- move LanceDB out of cloud-synced vaults for fast vector search

- add config management system with dashboard settings

- integrate new architecture with MCP tools and file watcher


### Changed

- split monolith into Cargo workspace with 4 crates


## `kajet-indexer` - [0.2.0](https://github.com/jpalczewski/kajet/releases/tag/kajet-indexer-v0.2.0) - 2026-02-08

### Added

- normalize filesystem paths to NFC for consistent Unicode handling

- extract wikilinks from chunks with link resolution and backlinks

- dual-sink logging system with file and dashboard output

- add incremental indexer with async pipeline and file watcher


### Fixed

- use longer test content to pass min_content_chars filter

- deadlock in indexing pipeline when file count exceeds channel buffer


### Performance

- use mtime pre-filter in change detection to avoid slow file reads on iCloud


## `kajet-backend` - [0.2.0](https://github.com/jpalczewski/kajet/releases/tag/kajet-backend-v0.2.0) - 2026-02-08

### Added

- normalize filesystem paths to NFC for consistent Unicode handling

- add MCP `examine` tool for document inspection

- extract wikilinks from chunks with link resolution and backlinks

- move LanceDB out of cloud-synced vaults for fast vector search

- add tracing instrumentation to search pipeline

- add config management system with dashboard settings

- add hybrid search, DocumentStore trait, and content hashing


### Changed

- simplify vector field definition and formatting in LanceVectorStore

- split monolith into Cargo workspace with 4 crates


### Fixed

- show "indexing in progress" instead of "no results" during startup

- return empty results when chunks table doesn't exist yet

- use floor_char_boundary for FTS snippet to avoid panic on multi-byte UTF-8

- move macOS target deps to end of Cargo.toml

- auto-migrate old chunks table schema on first upsert


### Performance

- use mtime pre-filter in change detection to avoid slow file reads on iCloud


## `kajet-core` - [0.2.0](https://github.com/jpalczewski/kajet/releases/tag/kajet-core-v0.2.0) - 2026-02-08

### Added

- add MCP `examine` tool for document inspection

- extract wikilinks from chunks with link resolution and backlinks

- move LanceDB out of cloud-synced vaults for fast vector search

- add tracing instrumentation to search pipeline

- dual-sink logging system with file and dashboard output

- add config management system with dashboard settings

- integrate new architecture with MCP tools and file watcher

- add hybrid search, DocumentStore trait, and content hashing


### Changed

- start dashboard before indexing for live log monitoring

- extract parser into dedicated crate with chunking improvements

- split monolith into Cargo workspace with 4 crates


### Fixed

- filter noisy third-party logs (LanceDB, Arrow, ignore) from output

- show "indexing in progress" instead of "no results" during startup


### Performance

- use mtime pre-filter in change detection to avoid slow file reads on iCloud


## `kajet-parser` - [0.2.0](https://github.com/jpalczewski/kajet/releases/tag/kajet-parser-v0.2.0) - 2026-02-08

### Added

- normalize filesystem paths to NFC for consistent Unicode handling

- filter link-only noise chunks from indexing

- extract wikilinks from chunks with link resolution and backlinks


### Changed

- extract parser into dedicated crate with chunking improvements


### Documentation

- add kajet-parser README with chunking and filtering docs


## `kajet` - [0.1.0](https://github.com/jpalczewski/kajet/releases/tag/kajet-v0.1.0) - 2026-02-07

### Added

- add Svelte frontend source and build config

- integrate new architecture with MCP tools and file watcher

- add incremental indexer with async pipeline and file watcher

- add layered config and i18n (EN/PL)

- migrate from FastEmbed to Candle for embedding model


### Changed

- split monolith into Cargo workspace with 4 crates


### Documentation

- update README with features, motivation, and current stack

- update CLAUDE.md to reflect workspace architecture


### Fixed

- build frontend with Deno before Rust compilation in CI

- add autotools for lzma-sys static build in CI

- install system deps (liblzma-dev, protobuf) in CI

- disable crates.io publish and fix GitHub URL in release-plz

- add pull-requests permission to release job

- release-plz changelog seed and workflow permissions


## `kajet-web` - [0.1.0](https://github.com/jpalczewski/kajet/releases/tag/kajet-web-v0.1.0) - 2026-02-07

### Added

- integrate new architecture with MCP tools and file watcher


### Changed

- split monolith into Cargo workspace with 4 crates


## `kajet-mcp` - [0.1.0](https://github.com/jpalczewski/kajet/releases/tag/kajet-mcp-v0.1.0) - 2026-02-07

### Added

- integrate new architecture with MCP tools and file watcher


### Changed

- split monolith into Cargo workspace with 4 crates


## `kajet-indexer` - [0.1.0](https://github.com/jpalczewski/kajet/releases/tag/kajet-indexer-v0.1.0) - 2026-02-07

### Added

- add incremental indexer with async pipeline and file watcher


## `kajet-backend` - [0.1.0](https://github.com/jpalczewski/kajet/releases/tag/kajet-backend-v0.1.0) - 2026-02-07

### Added

- add hybrid search, DocumentStore trait, and content hashing


### Changed

- simplify vector field definition and formatting in LanceVectorStore

- split monolith into Cargo workspace with 4 crates


### Fixed

- move macOS target deps to end of Cargo.toml

- auto-migrate old chunks table schema on first upsert


## `kajet-core` - [0.1.0](https://github.com/jpalczewski/kajet/releases/tag/kajet-core-v0.1.0) - 2026-02-07

### Added

- integrate new architecture with MCP tools and file watcher

- add hybrid search, DocumentStore trait, and content hashing


### Changed

- extract parser into dedicated crate with chunking improvements

- split monolith into Cargo workspace with 4 crates


## `kajet-parser` - [0.1.0](https://github.com/jpalczewski/kajet/releases/tag/kajet-parser-v0.1.0) - 2026-02-07

### Changed

- extract parser into dedicated crate with chunking improvements


## `kajet` - [0.1.0](https://github.com/jpalczewski/kajet/releases/tag/kajet-v0.1.0) - 2026-02-07

### Added

- integrate new architecture with MCP tools and file watcher

- add incremental indexer with async pipeline and file watcher

- add layered config and i18n (EN/PL)

- migrate from FastEmbed to Candle for embedding model


### Changed

- split monolith into Cargo workspace with 4 crates


### Documentation

- update CLAUDE.md to reflect workspace architecture


### Fixed

- add pull-requests permission to release job

- release-plz changelog seed and workflow permissions


## `kajet-web` - [0.1.0](https://github.com/jpalczewski/kajet/releases/tag/kajet-web-v0.1.0) - 2026-02-07

### Added

- integrate new architecture with MCP tools and file watcher


### Changed

- split monolith into Cargo workspace with 4 crates


## `kajet-mcp` - [0.1.0](https://github.com/jpalczewski/kajet/releases/tag/kajet-mcp-v0.1.0) - 2026-02-07

### Added

- integrate new architecture with MCP tools and file watcher


### Changed

- split monolith into Cargo workspace with 4 crates


## `kajet-indexer` - [0.1.0](https://github.com/jpalczewski/kajet/releases/tag/kajet-indexer-v0.1.0) - 2026-02-07

### Added

- add incremental indexer with async pipeline and file watcher


## `kajet-backend` - [0.1.0](https://github.com/jpalczewski/kajet/releases/tag/kajet-backend-v0.1.0) - 2026-02-07

### Added

- add hybrid search, DocumentStore trait, and content hashing


### Changed

- simplify vector field definition and formatting in LanceVectorStore

- split monolith into Cargo workspace with 4 crates


### Fixed

- auto-migrate old chunks table schema on first upsert


## `kajet-core` - [0.1.0](https://github.com/jpalczewski/kajet/releases/tag/kajet-core-v0.1.0) - 2026-02-07

### Added

- integrate new architecture with MCP tools and file watcher

- add hybrid search, DocumentStore trait, and content hashing


### Changed

- extract parser into dedicated crate with chunking improvements

- split monolith into Cargo workspace with 4 crates


## `kajet-parser` - [0.1.0](https://github.com/jpalczewski/kajet/releases/tag/kajet-parser-v0.1.0) - 2026-02-07

### Changed

- extract parser into dedicated crate with chunking improvements

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
