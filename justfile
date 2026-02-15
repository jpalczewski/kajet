# Generate TypeScript types from Rust and build frontend
build-front:
    @echo "Generating TypeScript types from Rust..."
    cargo test --workspace --lib --no-fail-fast
    @echo "Building frontend..."
    cd frontend && deno task build

# Development build (frontend + debug Rust binary)
dev: build-front
    @echo "Building development binary..."
    cargo build

# Release build (frontend + optimized Rust binary)
release: build-front
    @echo "Building release binary..."
    cargo build --release

# Run all tests
test:
    cargo nextest run --workspace

# Run clippy and fmt checks
check:
    cargo clippy --workspace -- -D warnings
    cargo fmt --check
    cd frontend && deno task check

# Format all code
fmt:
    cargo fmt
    cd frontend && deno fmt

# Run debug binary with vault from .vaults file
run-debug vault_id:
    #!/usr/bin/env bash
    set -euo pipefail
    vault_path=$(grep "^{{vault_id}} " .vaults | cut -d' ' -f2- || true)
    if [ -z "$vault_path" ]; then
        echo "Error: Vault ID {{vault_id}} not found in .vaults" >&2
        exit 1
    fi
    echo "Running with vault: $vault_path"
    cargo run -- --vault "$vault_path"
