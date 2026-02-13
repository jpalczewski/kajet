#!/usr/bin/env bash
# kajet-dev — helper for local development workflows
#
# Vaults are defined in .vaults (default):
#   1 /absolute/path/to/vault
#   2 /absolute/path with spaces

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
VAULTS_FILE="${KAJET_VAULTS_FILE:-$SCRIPT_DIR/.vaults}"

CLOUD_MARKERS=(
  "Library/Mobile Documents"
  "OneDrive"
  "Dropbox"
  "Google Drive"
  "MEGA"
  "pCloud"
)

die() {
  echo "Error: $*" >&2
  exit 1
}

usage() {
  cat <<EOF_USAGE
kajet-dev — development helper

Usage:
  ./kajet-dev.sh <command> [args]

Commands:
  run [TARGET] [-- kajet_args...]      Run kajet for TARGET (default: 1)
  inspector [TARGET] [-- kajet_args...] Run MCP Inspector with kajet for TARGET
  log [TARGET] [full]                  Tail kajet.log, or print full file
  status [TARGET]                      Show vault/db/log/config status
  db [TARGET]                          Print resolved db path for TARGET
  open [PORT]                          Open dashboard URL (default: 3579)
  vaults                               List configured vaults and status
  doctor                               Check toolchain/dependency health
  help                                 Show this help

TARGET:
  - Vault number from .vaults (e.g. 1)
  - Absolute/relative vault path

Environment overrides:
  KAJET_BIN         Explicit path to kajet binary
  KAJET_VAULTS_FILE Alternate vault mapping file
  KAJET_DB_PATH     Force db path (useful for cloud vault edge cases)
  KAJET_TAIL_LINES  Number of lines for log tail (default: 120)
EOF_USAGE
}

is_number() {
  [[ "$1" =~ ^[0-9]+$ ]]
}

canonicalize_path() {
  local path="$1"
  (cd "$path" && pwd -P)
}

resolve_data_dir() {
  if [[ -n "${XDG_DATA_HOME:-}" ]]; then
    printf '%s\n' "$XDG_DATA_HOME"
    return
  fi

  case "$(uname -s)" in
    Darwin) printf '%s\n' "$HOME/Library/Application Support" ;;
    Linux) printf '%s\n' "$HOME/.local/share" ;;
    *) printf '%s\n' "$HOME/.local/share" ;;
  esac
}

vault_path_by_number() {
  local wanted="$1"
  [[ -f "$VAULTS_FILE" ]] || die "Vault file not found: $VAULTS_FILE"

  while IFS= read -r line; do
    [[ "$line" =~ ^[[:space:]]*# ]] && continue
    [[ -z "${line//[[:space:]]/}" ]] && continue

    if [[ "$line" =~ ^[[:space:]]*([0-9]+)[[:space:]]+(.+)$ ]]; then
      local num="${BASH_REMATCH[1]}"
      local path="${BASH_REMATCH[2]}"
      if [[ "$num" == "$wanted" ]]; then
        printf '%s\n' "$path"
        return 0
      fi
    fi
  done < "$VAULTS_FILE"

  return 1
}

resolve_vault() {
  local target="${1:-1}"
  local raw_path

  if is_number "$target"; then
    raw_path="$(vault_path_by_number "$target")" || die "Vault #$target not found in $VAULTS_FILE"
  else
    raw_path="$target"
  fi

  [[ -d "$raw_path" ]] || die "Vault directory does not exist: $raw_path"
  canonicalize_path "$raw_path"
}

is_cloud_synced() {
  local path="$1"
  local marker
  for marker in "${CLOUD_MARKERS[@]}"; do
    if [[ "$path" == *"$marker"* ]]; then
      return 0
    fi
  done
  return 1
}

try_find_cloud_db_by_logs() {
  local vault_canonical="$1"
  local vaults_root="$2"
  local dir

  [[ -d "$vaults_root" ]] || return 1

  while IFS= read -r -d '' dir; do
    if [[ -f "$dir/kajet.log" ]] && rg -F -m 1 --quiet -- "$vault_canonical" "$dir/kajet.log"; then
      printf '%s\n' "$dir"
      return 0
    fi
  done < <(find "$vaults_root" -mindepth 1 -maxdepth 1 -type d -print0 | sort -z)

  return 1
}

try_compute_xxh3_hex_python() {
  local input="$1"

  command -v python3 >/dev/null 2>&1 || return 1

  python3 - "$input" <<'PY'
import sys

path = sys.argv[1]
try:
    import xxhash
except Exception:
    raise SystemExit(1)

print(f"{xxhash.xxh3_64(path.encode('utf-8')).intdigest():016x}")
PY
}

resolve_db_path() {
  local vault="$1"

  if [[ -n "${KAJET_DB_PATH:-}" ]]; then
    printf '%s\n' "$KAJET_DB_PATH"
    return
  fi

  local canonical
  canonical="$(canonicalize_path "$vault")"

  if ! is_cloud_synced "$canonical"; then
    printf '%s\n' "$canonical/.kajet"
    return
  fi

  local data_dir
  data_dir="$(resolve_data_dir)"
  local vaults_root="$data_dir/kajet/vaults"

  local from_logs
  from_logs="$(try_find_cloud_db_by_logs "$canonical" "$vaults_root" || true)"
  if [[ -n "$from_logs" ]]; then
    printf '%s\n' "$from_logs"
    return
  fi

  local xxh3_hex
  xxh3_hex="$(try_compute_xxh3_hex_python "$canonical" || true)"
  if [[ -n "$xxh3_hex" ]]; then
    printf '%s\n' "$vaults_root/$xxh3_hex"
    return
  fi

  if [[ -d "$vaults_root" ]]; then
    local count
    count="$(find "$vaults_root" -mindepth 1 -maxdepth 1 -type d | wc -l | tr -d ' ')"
    if [[ "$count" == "1" ]]; then
      find "$vaults_root" -mindepth 1 -maxdepth 1 -type d | head -n 1
      return
    fi
  fi

  die "Cannot resolve cloud db path for: $canonical (set KAJET_DB_PATH or install python xxhash module)"
}

resolve_kajet_bin() {
  local -a candidates=()

  if [[ -n "${KAJET_BIN:-}" ]]; then
    candidates+=("$KAJET_BIN")
  fi

  if command -v kajet >/dev/null 2>&1; then
    candidates+=("$(command -v kajet)")
  fi

  candidates+=(
    "$HOME/.cargo/bin/kajet"
    "$HOME/.cargo/target/release/kajet"
    "$HOME/.cargo/target/debug/kajet"
    "$SCRIPT_DIR/target/release/kajet"
    "$SCRIPT_DIR/target/debug/kajet"
  )

  local bin
  for bin in "${candidates[@]}"; do
    if [[ -x "$bin" ]]; then
      printf '%s\n' "$bin"
      return 0
    fi
  done

  return 1
}

ensure_kajet_bin() {
  local bin
  bin="$(resolve_kajet_bin || true)"
  if [[ -n "$bin" ]]; then
    printf '%s\n' "$bin"
    return
  fi

  die "kajet binary not found. Set KAJET_BIN or build/install kajet first."
}

shell_single_quote() {
  local raw="$1"
  local escaped
  escaped="$(printf '%s' "$raw" | sed "s/'/'\\\\''/g")"
  printf "'%s'" "$escaped"
}

split_target_and_rest() {
  # sets global vars: TARGET_ARG, REST_ARGS
  TARGET_ARG="1"
  REST_ARGS=()

  if [[ $# -eq 0 ]]; then
    return
  fi

  if [[ "$1" == "--" ]]; then
    shift
    REST_ARGS=("$@")
    return
  fi

  if [[ "$1" == -* ]]; then
    REST_ARGS=("$@")
    return
  fi

  TARGET_ARG="$1"
  shift

  if [[ ${1:-} == "--" ]]; then
    shift
  fi

  REST_ARGS=("$@")
}

cmd_run() {
  split_target_and_rest "$@"
  local vault
  vault="$(resolve_vault "$TARGET_ARG")"
  local bin
  bin="$(ensure_kajet_bin)"

  echo "# binary: $bin"
  echo "# vault:  $vault"
  exec "$bin" --vault "$vault" "${REST_ARGS[@]}"
}

cmd_inspector() {
  split_target_and_rest "$@"
  local vault
  vault="$(resolve_vault "$TARGET_ARG")"

  command -v npx >/dev/null 2>&1 || die "npx not found (install Node.js)"

  local bin
  bin="$(ensure_kajet_bin)"
  local preview
  preview="$(shell_single_quote "$bin") --vault $(shell_single_quote "$vault")"
  local arg
  for arg in "${REST_ARGS[@]}"; do
    preview+=" $(shell_single_quote "$arg")"
  done

  echo "# binary: $bin"
  echo "# vault:  $vault"
  echo "# exec:   npx @modelcontextprotocol/inspector $preview"
  exec npx @modelcontextprotocol/inspector "$bin" --vault "$vault" "${REST_ARGS[@]}"
}

pretty_print_log_full() {
  local file="$1"

  if command -v jq >/dev/null 2>&1; then
    jq -Rr '
      (try fromjson catch {timestamp:"",level:"RAW",message:.}) as $l |
      "\($l.timestamp) [\($l.level)] \(($l.fields.message // $l.message // ""))"
    ' "$file"
  else
    cat "$file"
  fi
}

pretty_print_log_tail() {
  local file="$1"
  local lines="${KAJET_TAIL_LINES:-120}"

  if command -v jq >/dev/null 2>&1; then
    tail -n "$lines" -f "$file" | jq -Rr '
      (try fromjson catch {timestamp:"",level:"RAW",message:.}) as $l |
      "\($l.timestamp) [\($l.level)] \(($l.fields.message // $l.message // ""))"
    '
  else
    tail -n "$lines" -f "$file"
  fi
}

cmd_log() {
  local target="1"
  local mode="tail"

  if [[ $# -ge 1 ]]; then
    if [[ "$1" == "full" ]]; then
      mode="full"
      shift
    else
      target="$1"
      shift
    fi
  fi
  if [[ $# -ge 1 ]]; then
    mode="$1"
  fi

  local vault
  vault="$(resolve_vault "$target")"
  local db_path
  db_path="$(resolve_db_path "$vault")"
  local log_file="$db_path/kajet.log"

  [[ -f "$log_file" ]] || die "Log file not found: $log_file"

  echo "# vault: $vault"
  echo "# db:    $db_path"
  echo "# log:   $log_file"

  if [[ "$mode" == "full" ]]; then
    pretty_print_log_full "$log_file"
  else
    pretty_print_log_tail "$log_file"
  fi
}

cmd_db() {
  local target="${1:-1}"
  local vault
  vault="$(resolve_vault "$target")"
  resolve_db_path "$vault"
}

cmd_status() {
  local target="${1:-1}"
  local vault
  vault="$(resolve_vault "$target")"
  local db_path
  db_path="$(resolve_db_path "$vault")"
  local config_file="$db_path/config.toml"
  local metadata_file="$db_path/metadata.json"
  local log_file="$db_path/kajet.log"
  local bin
  bin="$(resolve_kajet_bin || true)"

  echo "vault:      $vault"
  echo "db_path:    $db_path"
  echo "binary:     ${bin:-<not-found>}"

  if [[ -f "$config_file" ]]; then
    echo "config:     $config_file"
  else
    echo "config:     missing"
  fi

  if [[ -f "$metadata_file" ]]; then
    echo "metadata:   $metadata_file"
  else
    echo "metadata:   missing"
  fi

  if [[ -f "$log_file" ]]; then
    local lines
    lines="$(wc -l < "$log_file" | tr -d ' ')"
    echo "log:        $log_file ($lines lines)"
    echo "log_tail:"
    tail -n 3 "$log_file"
  else
    echo "log:        missing"
  fi
}

cmd_open() {
  local port="${1:-3579}"
  local url="http://localhost:$port"

  if command -v open >/dev/null 2>&1; then
    open "$url" >/dev/null 2>&1 || true
  fi
  echo "$url"
}

cmd_vaults() {
  [[ -f "$VAULTS_FILE" ]] || die "Vault file not found: $VAULTS_FILE"

  echo "Configured vaults from: $VAULTS_FILE"

  while IFS= read -r line; do
    [[ "$line" =~ ^[[:space:]]*# ]] && continue
    [[ -z "${line//[[:space:]]/}" ]] && continue

    if [[ "$line" =~ ^[[:space:]]*([0-9]+)[[:space:]]+(.+)$ ]]; then
      local num="${BASH_REMATCH[1]}"
      local path="${BASH_REMATCH[2]}"

      if [[ -d "$path" ]]; then
        local db_path
        db_path="$(resolve_db_path "$path" 2>/dev/null || true)"
        local log_note=""
        if [[ -n "$db_path" && -f "$db_path/kajet.log" ]]; then
          local lines
          lines="$(wc -l < "$db_path/kajet.log" | tr -d ' ')"
          log_note="log:${lines}"
        fi
        printf '%s\t%s\t%s\n' "$num" "$path" "${log_note:-ok}"
      else
        printf '%s\t%s\t%s\n' "$num" "$path" "missing-path"
      fi
    fi
  done < "$VAULTS_FILE"
}

cmd_doctor() {
  echo "# kajet-dev doctor"

  local ok="yes"

  if resolve_kajet_bin >/dev/null 2>&1; then
    echo "kajet_bin:  $(resolve_kajet_bin)"
  else
    echo "kajet_bin:  missing"
    ok="no"
  fi

  if command -v cargo >/dev/null 2>&1; then
    echo "cargo:      $(command -v cargo)"
  else
    echo "cargo:      missing"
    ok="no"
  fi

  if command -v npx >/dev/null 2>&1; then
    echo "npx:        $(command -v npx)"
  else
    echo "npx:        missing (needed for inspector)"
  fi

  if command -v jq >/dev/null 2>&1; then
    echo "jq:         $(command -v jq)"
  else
    echo "jq:         missing (optional, nicer log formatting)"
  fi

  if [[ -f "$VAULTS_FILE" ]]; then
    echo "vaults:     $VAULTS_FILE"
  else
    echo "vaults:     missing ($VAULTS_FILE)"
    ok="no"
  fi

  if [[ "$ok" == "yes" ]]; then
    echo "result:     OK"
  else
    echo "result:     WARN"
    return 1
  fi
}

main() {
  local cmd="${1:-help}"
  shift || true

  case "$cmd" in
    run|r)       cmd_run "$@" ;;
    inspector|i) cmd_inspector "$@" ;;
    log|l)       cmd_log "$@" ;;
    status|s)    cmd_status "$@" ;;
    db)          cmd_db "$@" ;;
    open|o)      cmd_open "$@" ;;
    vaults|v)    cmd_vaults ;;
    doctor|d)    cmd_doctor ;;
    help|-h|--help) usage ;;
    *) die "Unknown command: $cmd (run ./kajet-dev.sh help)" ;;
  esac
}

main "$@"
