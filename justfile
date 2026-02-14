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
