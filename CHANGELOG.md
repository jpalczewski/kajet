# Changelog

All notable changes to this project will be documented in this file.
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

