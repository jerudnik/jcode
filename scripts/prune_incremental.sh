#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
target_dir="${CARGO_TARGET_DIR:-$repo_root/target}"
cap_gib="${JCODE_INCREMENTAL_CAP_GIB:-24}"
interval_minutes="${JCODE_INCREMENTAL_PRUNE_INTERVAL_MINUTES:-60}"
lock_wait_seconds="${JCODE_INCREMENTAL_PRUNE_LOCK_WAIT_SECONDS:-300}"
apply=0
quiet=0
force=0
ignore_active_processes=0
use_apparent_size=0
handoff_build_pid=""
profiles=(debug selfdev)

usage() {
  cat <<'EOF'
Usage: scripts/prune_incremental.sh [options]

Keep disposable rustc incremental caches below a per-profile size cap without
removing dependency artifacts or final binaries.

Options:
  --apply                 Remove old incremental entries (default: dry run)
  --cap-gib N             Per-profile cap in GiB (default: 24)
  --interval-minutes N    Minimum interval between applied scans (default: 60)
  --profile NAME          Limit to one profile (repeatable)
  --target-dir PATH       Cargo target directory (default: $CARGO_TARGET_DIR or target/)
  --force                 Ignore the applied-scan interval
  --ignore-active-processes
                          Skip the global build guard (isolated test dirs only)
  --apparent-size         Count file lengths instead of allocated disk blocks
  --handoff-build-pid PID Register an active build before releasing the prune lock
  --quiet                 Suppress no-op status messages
  -h, --help              Show this help

Environment equivalents:
  JCODE_INCREMENTAL_CAP_GIB
  JCODE_INCREMENTAL_PRUNE_INTERVAL_MINUTES
EOF
}

selected_profiles=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    --apply)
      apply=1
      shift
      ;;
    --cap-gib)
      [[ $# -ge 2 ]] || { printf 'error: --cap-gib requires a value\n' >&2; exit 2; }
      cap_gib="$2"
      shift 2
      ;;
    --interval-minutes)
      [[ $# -ge 2 ]] || { printf 'error: --interval-minutes requires a value\n' >&2; exit 2; }
      interval_minutes="$2"
      shift 2
      ;;
    --profile)
      [[ $# -ge 2 ]] || { printf 'error: --profile requires a value\n' >&2; exit 2; }
      selected_profiles+=("$2")
      shift 2
      ;;
    --target-dir)
      [[ $# -ge 2 ]] || { printf 'error: --target-dir requires a value\n' >&2; exit 2; }
      target_dir="$2"
      shift 2
      ;;
    --force)
      force=1
      shift
      ;;
    --ignore-active-processes)
      ignore_active_processes=1
      shift
      ;;
    --apparent-size)
      use_apparent_size=1
      shift
      ;;
    --handoff-build-pid)
      [[ $# -ge 2 ]] || { printf 'error: --handoff-build-pid requires a value\n' >&2; exit 2; }
      handoff_build_pid="$2"
      shift 2
      ;;
    --quiet)
      quiet=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      printf 'error: unknown option: %s\n' "$1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

if [[ ${#selected_profiles[@]} -gt 0 ]]; then
  profiles=("${selected_profiles[@]}")
fi

if [[ ! "$cap_gib" =~ ^[0-9]+([.][0-9]+)?$ ]] \
  || ! awk -v value="$cap_gib" 'BEGIN { exit !(value > 0) }'; then
  printf 'error: cap must be a positive GiB value: %s\n' "$cap_gib" >&2
  exit 2
fi
if [[ ! "$interval_minutes" =~ ^[0-9]+$ ]]; then
  printf 'error: interval must be a non-negative integer: %s\n' "$interval_minutes" >&2
  exit 2
fi
if [[ ! "$lock_wait_seconds" =~ ^[0-9]+$ || "$lock_wait_seconds" -lt 1 ]]; then
  printf 'error: lock wait must be a positive integer: %s\n' "$lock_wait_seconds" >&2
  exit 2
fi
if [[ -n "$handoff_build_pid" && ! "$handoff_build_pid" =~ ^[0-9]+$ ]]; then
  printf 'error: handoff build PID must be numeric: %s\n' "$handoff_build_pid" >&2
  exit 2
fi

log() {
  if [[ "$quiet" -eq 0 ]]; then
    printf 'prune_incremental: %s\n' "$*" >&2
  fi
}

mtime_seconds() {
  local path="$1"
  if stat -f '%m' "$path" >/dev/null 2>&1; then
    stat -f '%m' "$path"
  else
    stat -c '%Y' "$path"
  fi
}

size_kib() {
  local path="$1"
  local kib
  kib=$(du -sk "$path" 2>/dev/null | awk '{print $1}')
  if [[ "${kib:-0}" -gt 0 && "$use_apparent_size" -eq 0 ]]; then
    printf '%s\n' "$kib"
    return 0
  fi

  # Some overlay/sandbox filesystems report almost no allocated blocks even for
  # populated files. Apparent-size mode keeps caps conservative there.
  local bytes=0 file file_bytes
  while IFS= read -r -d '' file; do
    if file_bytes=$(stat -f '%z' "$file" 2>/dev/null); then
      :
    else
      file_bytes=$(stat -c '%s' "$file")
    fi
    bytes=$((bytes + file_bytes))
  done < <(find "$path" -type f -print0)
  local apparent_kib=$(((bytes + 1023) / 1024))
  if (( apparent_kib > ${kib:-0} )); then
    printf '%s\n' "$apparent_kib"
  else
    printf '%s\n' "${kib:-0}"
  fi
}

format_gib() {
  awk -v kib="$1" 'BEGIN { printf "%.2f", kib / 1048576 }'
}

build_process_active() {
  local name
  for name in cargo rustc rustdoc; do
    if pgrep -x "$name" >/dev/null 2>&1; then
      return 0
    fi
  done
  return 1
}

active_build_marker_exists() {
  local marker pid
  local builds_dir="$target_dir/.jcode-active-builds"
  [[ -d "$builds_dir" ]] || return 1
  for marker in "$builds_dir"/*; do
    [[ -f "$marker" ]] || continue
    pid=$(cat "$marker" 2>/dev/null || true)
    if [[ "$pid" =~ ^[0-9]+$ ]] && kill -0 "$pid" 2>/dev/null; then
      return 0
    fi
    rm -f "$marker"
  done
  rmdir "$builds_dir" 2>/dev/null || true
  return 1
}

handoff_build_marker() {
  [[ -n "$handoff_build_pid" ]] || return 0
  local builds_dir="$target_dir/.jcode-active-builds"
  mkdir -p "$builds_dir"
  printf '%s\n' "$handoff_build_pid" >"$builds_dir/$handoff_build_pid"
}

mkdir -p "$target_dir"
stamp="$target_dir/.jcode-incremental-prune.stamp"
lock_dir="$target_dir/.jcode-incremental-prune.lock"

if [[ -z "$handoff_build_pid" && "$ignore_active_processes" -eq 0 ]] \
  && { active_build_marker_exists || build_process_active; }; then
  log "cargo/rustc process active; skipping"
  exit 0
fi

lock_deadline=$(($(date +%s) + lock_wait_seconds))
reclaim_dir="$target_dir/.jcode-incremental-prune.reclaim"
lock_token=""
reclaim_token=""

remove_owned_lock() {
  local dir="$1" token="$2"
  [[ -n "$token" ]] || return 0
  if [[ "$(cat "$dir/token" 2>/dev/null || true)" == "$token" ]]; then
    rm -rf "$dir"
  fi
}

cleanup() {
  remove_owned_lock "$lock_dir" "$lock_token"
  remove_owned_lock "$reclaim_dir" "$reclaim_token"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

while true; do
  if mkdir "$lock_dir" 2>/dev/null; then
    lock_token="$$.$RANDOM.$(date +%s)"
    printf '%s\n' "$$" >"$lock_dir/pid"
    printf '%s\n' "$lock_token" >"$lock_dir/token"

    # A stale-lock reclaimer may have removed the prior owner immediately
    # before this mkdir. Do not retain a new lock until that serialized
    # recovery has finished.
    if [[ -d "$reclaim_dir" ]]; then
      remove_owned_lock "$lock_dir" "$lock_token"
      lock_token=""
      if (( $(date +%s) >= lock_deadline )); then
        log "timed out waiting for stale-lock recovery; refusing to race a build"
        exit 75
      fi
      sleep 0.1
      continue
    fi
    break
  fi

  lock_owner=$(cat "$lock_dir/pid" 2>/dev/null || true)
  observed_token=$(cat "$lock_dir/token" 2>/dev/null || true)
  if [[ "$lock_owner" =~ ^[0-9]+$ ]] && ! kill -0 "$lock_owner" 2>/dev/null; then
    reclaim_token="$$.$RANDOM.$(date +%s)"
    if mkdir "$reclaim_dir" 2>/dev/null; then
      printf '%s\n' "$$" >"$reclaim_dir/pid"
      printf '%s\n' "$reclaim_token" >"$reclaim_dir/token"

      # Re-read after acquiring the recovery guard. Only delete the exact
      # dead owner observed before the guard was acquired.
      current_owner=$(cat "$lock_dir/pid" 2>/dev/null || true)
      current_token=$(cat "$lock_dir/token" 2>/dev/null || true)
      if [[ "$current_owner" == "$lock_owner" \
        && "$current_token" == "$observed_token" \
        && "$current_owner" =~ ^[0-9]+$ ]] \
        && ! kill -0 "$current_owner" 2>/dev/null; then
        rm -rf "$lock_dir"
      fi
      remove_owned_lock "$reclaim_dir" "$reclaim_token"
    fi
    continue
  fi
  if (( $(date +%s) >= lock_deadline )); then
    log "timed out waiting for active prune; refusing to race a build"
    exit 75
  fi
  sleep 0.1
done

if [[ -n "${JCODE_INCREMENTAL_PRUNE_TEST_HOLD_SECONDS:-}" ]]; then
  if [[ "${JCODE_INCREMENTAL_PRUNE_TEST_ROOT:-}" != "$target_dir" ]]; then
    printf 'error: test lock hold is restricted to JCODE_INCREMENTAL_PRUNE_TEST_ROOT\n' >&2
    exit 2
  fi
  test_critical_dir="$target_dir/.jcode-incremental-prune.test-critical"
  if ! mkdir "$test_critical_dir" 2>/dev/null; then
    printf 'error: concurrent prune critical sections detected\n' >&2
    exit 90
  fi
  sleep "$JCODE_INCREMENTAL_PRUNE_TEST_HOLD_SECONDS"
  rmdir "$test_critical_dir"
fi

if [[ "$apply" -eq 1 && "$force" -eq 0 && -f "$stamp" && "$interval_minutes" -gt 0 ]]; then
  now=$(date +%s)
  stamped=$(mtime_seconds "$stamp")
  age_seconds=$((now - stamped))
  if (( age_seconds >= 0 && age_seconds < interval_minutes * 60 )); then
    log "last applied scan is recent; skipping"
    handoff_build_marker
    exit 0
  fi
fi

# Close the race between the first process check and lock acquisition.
if [[ "$ignore_active_processes" -eq 0 ]] \
  && { active_build_marker_exists || build_process_active; }; then
  log "cargo/rustc process became active; skipping"
  handoff_build_marker
  exit 0
fi

cap_kib=$(awk -v gib="$cap_gib" 'BEGIN { value = int(gib * 1048576); print (value > 0 ? value : 1) }')
planned_kib=0
removed_kib=0

for profile in "${profiles[@]}"; do
  incremental_dir="$target_dir/$profile/incremental"
  [[ -d "$incremental_dir" ]] || continue

  total_kib=$(size_kib "$incremental_dir")
  if (( total_kib <= cap_kib )); then
    log "$profile incremental cache is $(format_gib "$total_kib") GiB (cap: ${cap_gib} GiB)"
    continue
  fi

  candidates=$(mktemp "${TMPDIR:-/tmp}/jcode-incremental-candidates.XXXXXX")
  while IFS= read -r -d '' entry; do
    entry_mtime=$(mtime_seconds "$entry")
    entry_kib=$(size_kib "$entry")
    printf '%s\t%s\t%s\n' "$entry_mtime" "$entry_kib" "$entry" >>"$candidates"
  done < <(find "$incremental_dir" -mindepth 1 -maxdepth 1 -type d -print0)

  while IFS=$'\t' read -r _ entry_kib entry; do
    (( total_kib > cap_kib )) || break
    planned_kib=$((planned_kib + entry_kib))
    if [[ "$apply" -eq 1 ]]; then
      rm -rf "$entry"
      removed_kib=$((removed_kib + entry_kib))
    fi
    total_kib=$((total_kib - entry_kib))
  done < <(sort -n "$candidates")
  rm -f "$candidates"

done

if [[ "$apply" -eq 1 ]]; then
  touch "$stamp"
  if (( removed_kib > 0 )); then
    printf 'prune_incremental: reclaimed approximately %s GiB; preserved deps and binaries\n' \
      "$(format_gib "$removed_kib")" >&2
  else
    log "nothing to prune"
  fi
elif (( planned_kib > 0 )); then
  printf 'prune_incremental: would reclaim approximately %s GiB; rerun with --apply\n' \
    "$(format_gib "$planned_kib")" >&2
else
  log "nothing to prune"
fi

handoff_build_marker
