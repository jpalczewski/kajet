#!/bin/bash
# kajet-dev — helper for development workflows
#
# Usage:
#   ./kajet-dev.sh inspector [NUM]   — build & launch MCP Inspector on vault NUM
#   ./kajet-dev.sh log [NUM]         — tail the kajet.log for vault NUM
#   ./kajet-dev.sh log [NUM] full    — cat the entire kajet.log
#   ./kajet-dev.sh vaults            — list configured vaults
#
# Vault numbers are defined in .vaults file (one per line: NUM /path/to/vault)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
VAULTS_FILE="$SCRIPT_DIR/.vaults"

# --------------------------------------------------------------------------
# Helpers
# --------------------------------------------------------------------------

die() { echo "Error: $*" >&2; exit 1; }

usage() {
    cat <<'EOF'
kajet-dev — development helper

Commands:
  inspector [NUM]      Build & launch MCP Inspector on vault NUM (default: 1)
  log [NUM]            Tail kajet.log for vault NUM (default: 1)
  log [NUM] full       Print entire kajet.log for vault NUM
  vaults               List configured vaults

Vault numbers are defined in .vaults (one per line: NUM /path/to/vault)
EOF
    exit 0
}

# Read vault path for given number from .vaults
resolve_vault() {
    local num="${1:-1}"
    [ -f "$VAULTS_FILE" ] || die ".vaults file not found — create it with: NUM /path/to/vault"

    local line
    line=$(grep -E "^${num}\s+" "$VAULTS_FILE" | head -1) || true
    [ -n "$line" ] || die "Vault #${num} not found in .vaults"

    # Everything after the number and whitespace
    local vault_path
    vault_path=$(echo "$line" | sed "s/^${num}[[:space:]]*//")
    [ -d "$vault_path" ] || die "Vault directory does not exist: $vault_path"

    echo "$vault_path"
}

# Resolve db_path for a vault (mirrors Rust logic in db_path.rs)
resolve_db_path() {
    local vault_path="$1"
    local canonical
    canonical=$(cd "$vault_path" && pwd -P)

    # Check if cloud-synced
    if echo "$canonical" | grep -qE "(Library/Mobile Documents|OneDrive|Dropbox|Google Drive|MEGA|pCloud)"; then
        # Hash with xxh3_64 — we use Python as a portable fallback
        local hex
        hex=$(python3 -c "
import hashlib, struct
# xxh3_64 from Rust — we approximate with a simple hash
# Actually use the same xxhash if available, else SHA256-based stable hash
try:
    import xxhash
    h = xxhash.xxh3_64(b'$canonical').intdigest()
except ImportError:
    # Fallback: match Rust's xxh3_64 output — need xxhash pip package
    # For now, print a hint
    import sys
    print('NEED_XXHASH', file=sys.stderr)
    sys.exit(1)
print(format(h, '016x'))
" 2>/dev/null) || true

        if [ -z "$hex" ]; then
            # Try the Rust binary to resolve
            hex=$("$SCRIPT_DIR/target/debug/kajet" --vault "$vault_path" --db-path-only 2>/dev/null) || true
        fi

        if [ -z "$hex" ]; then
            # Brute-force: look for the vault in known db locations
            local data_dir="${HOME}/Library/Application Support"
            local vaults_dir="$data_dir/kajet/vaults"
            if [ -d "$vaults_dir" ]; then
                # If there's only one vault dir, use it; otherwise list them
                local count
                count=$(ls -1 "$vaults_dir" 2>/dev/null | wc -l | tr -d ' ')
                if [ "$count" -eq 1 ]; then
                    echo "$vaults_dir/$(ls -1 "$vaults_dir")"
                    return
                elif [ "$count" -gt 0 ]; then
                    # Find the one that has a matching config or was most recently modified
                    # Check each dir for a kajet.log that references our vault
                    for dir in "$vaults_dir"/*/; do
                        if [ -f "${dir}kajet.log" ] && grep -q "$(basename "$vault_path")" "${dir}kajet.log" 2>/dev/null; then
                            echo "${dir%/}"
                            return
                        fi
                    done
                    # Fallback: most recently modified
                    echo "$vaults_dir/$(ls -1t "$vaults_dir" | head -1)"
                    return
                fi
            fi
            die "Cannot resolve db_path for cloud vault. Install xxhash: pip3 install xxhash"
        fi

        echo "${HOME}/Library/Application Support/kajet/vaults/$hex"
    else
        echo "$canonical/.kajet"
    fi
}

list_vaults() {
    [ -f "$VAULTS_FILE" ] || die ".vaults file not found"
    echo "Configured vaults:"
    echo ""
    while IFS= read -r line; do
        # Skip comments and empty lines
        [[ "$line" =~ ^[[:space:]]*# ]] && continue
        [[ -z "${line// /}" ]] && continue
        local num vault_path
        num=$(echo "$line" | awk '{print $1}')
        vault_path=$(echo "$line" | sed "s/^[0-9]*[[:space:]]*//")
        local db_path
        db_path=$(resolve_db_path "$vault_path" 2>/dev/null) || db_path="(cannot resolve)"
        local log_exists=""
        [ -f "$db_path/kajet.log" ] && log_exists=" [log: $(wc -l < "$db_path/kajet.log" | tr -d ' ') lines]"
        printf "  %s  %s%s\n" "$num" "$vault_path" "$log_exists"
    done < "$VAULTS_FILE"
}

# --------------------------------------------------------------------------
# Commands
# --------------------------------------------------------------------------

cmd_inspector() {
    local num="${1:-1}"
    local vault_path
    vault_path=$(resolve_vault "$num")

    echo "Building kajet..."
    cargo build --quiet --manifest-path "$SCRIPT_DIR/Cargo.toml"

    echo "Starting MCP Inspector with vault: $vault_path"
    npx @modelcontextprotocol/inspector "$SCRIPT_DIR/target/debug/kajet" --vault "$vault_path"
}

cmd_log() {
    local num="${1:-1}"
    local mode="${2:-tail}"
    local vault_path
    vault_path=$(resolve_vault "$num")
    local db_path
    db_path=$(resolve_db_path "$vault_path")
    local log_file="$db_path/kajet.log"

    [ -f "$log_file" ] || die "Log file not found: $log_file"

    echo "# Vault: $vault_path"
    echo "# Log:   $log_file"
    echo ""

    case "$mode" in
        full)
            # Pretty-print JSON log lines
            if command -v jq &>/dev/null; then
                jq -r '
                    "\(.timestamp // .["timestamp"] // "") [\(.level // "")] \(.fields.message // .message // "")" +
                    if .span.name then " (\(.span.name))" else "" end +
                    if (.fields | length) > 1 then
                        " " + (
                            [.fields | to_entries[] | select(.key != "message") | "\(.key)=\(.value)"]
                            | join(" ")
                        )
                    else "" end
                ' "$log_file" 2>/dev/null || cat "$log_file"
            else
                cat "$log_file"
            fi
            ;;
        *)
            # Tail with pretty-print
            if command -v jq &>/dev/null; then
                tail -f "$log_file" | jq -r '
                    "\(.timestamp // "") [\(.level // "")] \(.fields.message // .message // "")" +
                    if .span.name then " (\(.span.name))" else "" end
                '
            else
                tail -f "$log_file"
            fi
            ;;
    esac
}

# --------------------------------------------------------------------------
# Dispatch
# --------------------------------------------------------------------------

[ $# -eq 0 ] && usage

case "$1" in
    inspector|i)  shift; cmd_inspector "$@" ;;
    log|l)        shift; cmd_log "$@" ;;
    vaults|v)     list_vaults ;;
    help|-h|--help) usage ;;
    *) die "Unknown command: $1 — run '$0 help' for usage" ;;
esac
