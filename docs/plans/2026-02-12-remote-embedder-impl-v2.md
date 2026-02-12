# Remote Embedder — Implementation Plan (v2)

**Date:** 2026-02-12  
**Supersedes:** `docs/plans/2026-02-12-remote-embedder-impl.md`  
**Goal:** Deliver selectable embedding backend (Candle + OpenAI-compatible remote) with strong testability, reusable abstractions, and safe migration.

---

## 1. What Changes in v2

This plan keeps the functional scope from v1, but fixes implementation risks:

1. Retry logic must be actually testable and tested.
2. Remote embedder constructor must be reusable (no forced network I/O on `new`).
3. API payload validation must be strict (count, index continuity, dimensions).
4. Prefix behavior must have both positive and negative tests.
5. Config migration must cover legacy `embedding_model` compatibility.
6. Runtime knobs for remote backend (timeouts/retries/batch size) must be configurable and test-friendly.
7. Library-level errors should be typed (`thiserror`) for precise assertions.

---

## 2. Target Architecture

### 2.1 Core contracts

- `Embedder` remains async.
- Prefix injection remains outside `Embedder` (caller-level concern).
- `SearchEngine` handles query prefix.
- `IndexPipeline` handles document prefix.

### 2.2 New reusable remote abstractions

- `RemoteEmbedderConfig` (timeouts, retry policy, batch size, probe behavior).
- `RemoteEmbedderBuilder` (construct test/prod variants without branching in callers).
- `RemoteTransport` trait for HTTP boundary abstraction.
- Pure response validator (`validate_and_reorder`) independent from network stack.

### 2.3 Construction split

- `RemoteEmbedder::new(...)` is pure construction (no network).
- `RemoteEmbedder::probe().await` performs connectivity + dimension discovery.
- `RemoteEmbedder::connect(...).await` convenience API (`new` + `probe`) for production startup.

---

## 3. Implementation Tasks

## Task 0: Baseline and safety net

**Purpose:** Start with explicit behavior contracts before refactor.

**Files:**
- Add: `crates/core/tests/embedder_contracts.rs`
- Add: `crates/remote/tests/contracts.rs`

**Tests to add first:**
1. `query_prefix_applied_when_configured`
2. `query_prefix_not_applied_by_default`
3. `document_prefix_applied_when_configured`
4. `document_prefix_not_applied_by_default`
5. `embed_empty_input_returns_empty`

**Exit criteria:**
- Tests compile (some fail before implementation updates).

---

## Task 1: Config model + backward compatibility

**Files:**
- Modify: `crates/core/src/config.rs`
- Modify tests in: `crates/core/src/config.rs`

**Changes:**
1. Keep `EmbeddingConfig` + `EmbeddingBackend`.
2. Replace `embedding_model` with `embedding`.
3. Add migration compatibility path for legacy config:
   - Legacy key `embedding_model` still accepted.
   - If both new and legacy exist, new `embedding.model` wins.
4. Keep defaults for all `embedding.*` values.

**Tests (must exist):**
1. `embedding_config_defaults`
2. `embedding_backend_serde_roundtrip`
3. `config_accepts_legacy_embedding_model_key`
4. `config_prefers_new_embedding_model_over_legacy`
5. `default_config_uses_candle_backend_without_embedding_section`

**Exit criteria:**
- `cargo nextest run -p kajet-core -E 'test(config) | test(embedding_)'`

---

## Task 2: Async Embedder migration (core + backend)

**Files:**
- Modify: `crates/core/src/traits.rs`
- Modify: `crates/core/src/engine.rs`
- Modify: `crates/core/src/search.rs`
- Modify: `crates/backend/src/embedder.rs`
- Modify: `crates/indexer/src/embedding_worker.rs`

**Changes:**
1. Keep `Embedder` async.
2. Move blocking Candle work behind `spawn_blocking` inside Candle embedder.
3. Remove redundant `spawn_blocking` wrapper from embedding worker.
4. Guard trace logging for empty output (no indexing into `embeddings[0]` without checks).

**Tests (must exist):**
1. `mock_embedder_records_calls_async`
2. `candle_embedder_empty_input_returns_empty`
3. existing engine/search tests adapted to `.await`.

**Exit criteria:**
- `cargo nextest run -p kajet-core`
- `cargo nextest run -p kajet-backend`
- `cargo nextest run -p kajet-indexer`

---

## Task 3: Reusable prefix formatting

**Files:**
- Add: `crates/core/src/embedding_input.rs`
- Modify: `crates/core/src/lib.rs` (exports if needed)
- Modify: `crates/core/src/search.rs`
- Modify: `crates/indexer/src/pipeline.rs`
- Modify: `crates/indexer/src/lib.rs`

**Changes:**
1. Introduce helper:
   - `apply_prefix(prefix: &str, text: &str) -> String`
   - `apply_prefix_batch(prefix: &str, texts: impl IntoIterator<Item=&str>) -> Vec<String>`
2. Use helper in:
   - query embedding path
   - document chunk embedding path
3. Avoid duplicated `format!("{}{}", ...)` across crates.

**Tests (must exist):**
1. `apply_prefix_no_prefix_returns_original`
2. `apply_prefix_handles_empty_text`
3. `vector_search_prepends_query_prefix`
4. `vector_search_no_prefix_by_default`
5. `pipeline_prepends_document_prefix`
6. `pipeline_no_document_prefix_by_default`

**Exit criteria:**
- `cargo nextest run -p kajet-core -E 'test(prefix) | test(search)'`
- `cargo nextest run -p kajet-indexer -E 'test(prefix) | test(pipeline)'`

---

## Task 4: Metadata for reindex detection

**Files:**
- Modify: `crates/backend/src/metadata.rs`

**Changes:**
1. Track:
   - `embedding_backend`
   - `embedding_model`
   - `embedding_base_url`
2. Compare normalized values in `needs_reindex`:
   - trim trailing slash from URL before compare.
3. Keep serde defaults for backward compatibility with old metadata.

**Tests (must exist):**
1. `save_and_load_with_embedding_config`
2. `reindex_on_backend_change`
3. `reindex_on_model_change`
4. `reindex_on_base_url_change`
5. `base_url_normalization_avoids_false_reindex` (`http://x` vs `http://x/`)
6. `old_metadata_without_new_fields_still_deserializes`

**Exit criteria:**
- `cargo nextest run -p kajet-backend -E 'test(metadata) | test(reindex)'`

---

## Task 5: Create `kajet-remote` with testable boundaries

**Files:**
- Create: `crates/remote/Cargo.toml`
- Create: `crates/remote/src/lib.rs`
- Create: `crates/remote/src/config.rs`
- Create: `crates/remote/src/error.rs`
- Create: `crates/remote/src/transport.rs`
- Create: `crates/remote/src/validator.rs`
- Create: `crates/remote/src/embedder.rs`
- Modify: root `Cargo.toml` workspace members

**Design details:**

### 5.1 Config object

`RemoteEmbedderConfig` fields:
- `base_url: String`
- `model: String`
- `api_key: Option<String>`
- `connect_timeout: Duration`
- `request_timeout: Duration`
- `max_retries: u32`
- `initial_backoff: Duration`
- `max_batch_size: usize`
- `probe_on_connect: bool`

### 5.2 Typed errors

Use `thiserror` in `crates/remote`:
- `Transport`
- `HttpStatus { status, body }`
- `InvalidResponse { reason }`
- `DimensionMismatch { expected, got }`
- `ProbeFailed`

Map to `anyhow` only at app boundary (`main.rs`).

### 5.3 Transport abstraction

Create trait:
- `RemoteTransport::embed(request) -> Result<EmbedResponse, RemoteEmbedderError>`

Implementations:
- `ReqwestTransport` (production)
- `FakeTransport` in tests (deterministic retry/validation tests)

### 5.4 Constructor split

APIs:
- `RemoteEmbedder::new(config, transport)` (no network)
- `RemoteEmbedder::probe(&mut self).await`
- `RemoteEmbedder::connect(config).await` (convenience for main)

### 5.5 Response validator

Pure function validates:
1. `data.len() == input_count`
2. Indexes are unique and cover `0..input_count`
3. Embedding vectors have consistent dimension
4. Optional `expected_dim` match when already known

Returns ordered `Vec<Vec<f32>>`.

**Tests (must exist):**
1. `probe_discovers_dimension`
2. `probe_fails_on_empty_response`
3. `embed_returns_ordered_by_index`
4. `embed_fails_on_missing_index`
5. `embed_fails_on_duplicate_index`
6. `embed_fails_on_dimension_mismatch`
7. `embed_retries_on_429_then_succeeds` (assert request count)
8. `embed_retries_on_5xx_then_succeeds` (assert request count)
9. `embed_does_not_retry_on_4xx`
10. `embed_splits_batches_using_max_batch_size`
11. `authorization_header_set_when_api_key_present` (integration/wiremock)
12. `no_authorization_header_when_api_key_absent` (integration/wiremock)

**Exit criteria:**
- `cargo nextest run -p kajet-remote`

---

## Task 6: Factory and wiring

**Files:**
- Modify: `crates/backend/src/lib.rs`
- Modify: root `Cargo.toml` (dependency on `kajet-remote`)
- Modify: `src/main.rs`

**Changes:**
1. Factory accepts `Arc<dyn Embedder>`.
2. Add dedicated embedder builder function in app layer:
   - `async fn build_embedder(cfg: &EmbeddingConfig) -> anyhow::Result<Arc<dyn Embedder>>`
3. In remote path, use `RemoteEmbedder::connect`.
4. Wire prefixes:
   - query prefix -> `SearchEngine`
   - document prefix -> `Indexer`
5. Avoid repeated `RwLock` reads when saving metadata; read config once per operation.

**Tests:**
1. config `backend = candle` -> Candle embedder path
2. config `backend = remote` -> Remote embedder path
3. metadata save uses backend/model/base_url tuple

**Exit criteria:**
- `cargo build`
- `cargo nextest run --workspace`

---

## Task 7: i18n and observability

**Files:**
- Modify: `locales/en.toml`
- Modify: `locales/pl.toml`
- Modify: remote logging call sites

**Changes:**
1. Add localized strings for connect/probe/failure.
2. Keep logs at correct levels:
   - `INFO`: backend selection, probe success, result counts
   - `DEBUG`: retry attempt with status + backoff
   - `ERROR`: probe/embedding final failure

**Tests/Checks:**
- `cargo clippy --workspace -- -D warnings`
- `cargo fmt --check`
- `typos`

---

## 4. Suggested Commit Plan

1. `feat(core): add embedding config with legacy migration compatibility`
2. `refactor(core): make Embedder async across callers and implementors`
3. `refactor(core,indexer): centralize embedding prefix formatting`
4. `feat(backend): extend metadata reindex detection for backend/model/base_url`
5. `feat(remote): add testable remote embedder with typed errors and validator`
6. `feat(app): wire backend selection and embedder construction in main`
7. `feat(i18n): add remote embedder localization strings`

---

## 5. Verification Matrix

- [ ] `cargo build --release`
- [ ] `cargo nextest run --workspace`
- [ ] `cargo clippy --workspace -- -D warnings`
- [ ] `cargo fmt --check`
- [ ] `typos`
- [ ] Legacy config with `embedding_model` still works.
- [ ] Default config with no `[embedding]` still resolves to Candle.
- [ ] Remote probe failure returns typed error and clear top-level message.
- [ ] Retry behavior verified by tests with counted requests.
- [ ] Invalid remote payloads are rejected (no silent reorder/accept).
- [ ] Query/document prefix behavior tested for both configured and default paths.
- [ ] Backend/model/base_url changes trigger reindex; URL normalization prevents false positives.

---

## 6. Non-Goals

- No change to MCP transport semantics (`stdout` remains reserved for protocol).
- No addition of chat-completions yet (remote crate prepares extension point only).
- No change in vector store schema beyond metadata fields already required for reindex detection.
