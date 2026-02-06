#!/bin/bash
# MCP Inspector wrapper for Kajet
#
# Usage:
#   1. Copy .vault-path.example to .vault-path
#   2. Edit .vault-path with your vault location
#   3. Run: ./run-inspector.sh
#
# Or pass vault path directly:
#   ./run-inspector.sh /path/to/vault

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONFIG_FILE="$SCRIPT_DIR/.vault-path"

# Use argument if provided, otherwise read from .vault-path
if [ -n "$1" ]; then
    VAULT_PATH="$1"
elif [ -f "$CONFIG_FILE" ]; then
    VAULT_PATH="$(head -n 1 "$CONFIG_FILE" | xargs)"
else
    echo "Error: No vault path specified"
    echo ""
    echo "Option 1: Create .vault-path file"
    echo "  cp .vault-path.example .vault-path"
    echo "  # Edit .vault-path with your vault location"
    echo ""
    echo "Option 2: Pass vault path as argument"
    echo "  $0 /path/to/your/vault"
    exit 1
fi

if [ ! -d "$VAULT_PATH" ]; then
    echo "Error: Vault directory does not exist: $VAULT_PATH"
    exit 1
fi

echo "Starting Kajet MCP Inspector with vault: $VAULT_PATH"
cargo build --quiet && \
npx @modelcontextprotocol/inspector ./target/debug/kajet --vault "$VAULT_PATH"
