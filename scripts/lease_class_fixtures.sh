#!/usr/bin/env bash
# F03 runtime fixture matrix for lease classes and exit-mode verification.
#
# This runs the lease-class runtime matrix using the live daemon and current
# debug socket commands. It keeps the runtime isolated under a private
# temp home/runtime root, exercises all ActivityClass variants, and verifies the
# forced-exit and crash-recovery residue cases.

set -euo pipefail

if [[ $# -lt 1 ]]; then
  printf 'usage: %s <jcode-binary>\n' "$0" >&2
  exit 2
fi

JCODE_BIN=$1
shift || true

if [[ ! -x "$JCODE_BIN" ]]; then
  printf 'lease fixture matrix: binary not executable: %s\n' "$JCODE_BIN" >&2
  exit 2
fi

lease_classes=(
  client-connection
  provider-turn
  startup-recovery
  debug-job
  background-task
  mcp-call
  swarm-waiter
  scheduled-delivery
)

PATH="${PATH:-/usr/bin:/bin:/usr/sbin:/sbin}"

log() {
  printf '[%s] %s\n' "$(date -u '+%H:%M:%S')" "$*"
}

wait_for_socket() {
  local socket=$1
  local tries=0
  while (( tries < 200 )); do
    if env JCODE_DEBUG_CONTROL=1 "$JCODE_BIN" --quiet --socket "$socket" debug shutdown:state >/dev/null 2>&1; then
      return 0
    fi
    tries=$((tries + 1))
    sleep 0.1
  done
  printf 'lease fixture matrix: server socket never became ready: %s\n' "$socket" >&2
  return 1
}

start_temp_server() {
  local root=$1
  local name=$2
  local idle_secs=$3
  local extra_env_name=${4:-}
  local extra_env_value=${5:-}

  mkdir -p "$root/home" "$root/runtime"
  local home="$root/home"
  local runtime="$root/runtime"
  local socket="$runtime/jcode.sock"
  local log_file="$root/server.log"

  local -a env_args=(
    env -i
    HOME="$home"
    JCODE_HOME="$home/.jcode"
    JCODE_RUNTIME_DIR="$runtime"
    JCODE_DEBUG_CONTROL=1
    JCODE_TEMP_SERVER=1
    JCODE_SERVER_SCOPE=temporary
    JCODE_SERVER_OWNER_PID="$$"
    JCODE_TEMP_SERVER_IDLE_SECS="$idle_secs"
    PATH="$PATH"
  )
  if [[ -n "$extra_env_name" ]]; then
    env_args+=("$extra_env_name=$extra_env_value")
  fi

  "${env_args[@]}" "$JCODE_BIN" serve \
    --provider jcode \
    --socket "$socket" \
    --temporary-server \
    --owner-pid "$$" \
    --temp-idle-timeout-secs "$idle_secs" \
    --server-name "$name" \
    >"$log_file" 2>&1 &
  START_TEMP_SERVER_PID=$!
}

extract_token() {
  sed -n 's/.*"token":[[:space:]]*\([0-9][0-9]*\).*/\1/p'
}

lease_cycle_for_class() {
  local class=$1
  local root socket pid response token
  root=$(mktemp -d "/tmp/jcode-f03-${class}.XXXXXX")
  socket="$root/runtime/jcode.sock"

  cleanup_roots+=("$root")
  start_temp_server "$root" "f03-$class" 5
  pid=$START_TEMP_SERVER_PID
  wait_for_socket "$socket"

  response=$(env JCODE_DEBUG_CONTROL=1 "$JCODE_BIN" --quiet --socket "$socket" debug shutdown:hold_lease:"$class")
  token=$(printf '%s' "$response" | extract_token)
  if [[ -z "$token" ]]; then
    printf 'lease fixture matrix: failed to parse token for %s from %s\n' "$class" "$response" >&2
    kill "$pid" >/dev/null 2>&1 || true
    wait "$pid" >/dev/null 2>&1 || true
    return 1
  fi

  sleep 6
  if ! kill -0 "$pid" >/dev/null 2>&1; then
    printf 'lease fixture matrix: %s exited while lease was held\n' "$class" >&2
    wait "$pid" >/dev/null 2>&1 || true
    return 1
  fi

  env JCODE_DEBUG_CONTROL=1 "$JCODE_BIN" --quiet --socket "$socket" debug shutdown:release_lease:"$token" >/dev/null

  if wait "$pid"; then
    status=0
  else
    status=$?
  fi
  if [[ "$status" -ne 44 ]]; then
    printf 'lease fixture matrix: %s exited with %d after release, expected 44\n' "$class" "$status" >&2
    return 1
  fi

  if [[ -e "$socket" ]]; then
    printf 'lease fixture matrix: %s left socket residue: %s\n' "$class" "$socket" >&2
    return 1
  fi
  if [[ -e "$root/runtime/jcode-debug.sock" ]]; then
    printf 'lease fixture matrix: %s left debug socket residue: %s\n' "$class" "$root/runtime/jcode-debug.sock" >&2
    return 1
  fi
  if [[ -e "$root/home/.jcode/state/shutdown-watchdog.json" ]]; then
    printf 'lease fixture matrix: %s left shutdown marker residue: %s\n' "$class" "$root/home/.jcode/state/shutdown-watchdog.json" >&2
    return 1
  fi
  rm -rf "$root"
}

forced_exit_and_sigkill_cycles() {
  local root pid socket marker response token status successor_pid
  root=$(mktemp -d "/tmp/jcode-f03-forced.XXXXXX")
  socket="$root/runtime/jcode.sock"
  marker="$root/home/.jcode/state/shutdown-watchdog.json"

  cleanup_roots+=("$root")
  start_temp_server "$root" "f03-forced-exit" 5 JCODE_TEST_SHUTDOWN_CLEANUP_HANG_MS 30000
  pid=$START_TEMP_SERVER_PID
  wait_for_socket "$socket"
  kill -TERM "$pid"
  if wait "$pid"; then
    status=0
  else
    status=$?
  fi
  if [[ "$status" -ne 70 ]]; then
    printf 'lease fixture matrix: forced-exit watchdog exited with %d, expected 70\n' "$status" >&2
    return 1
  fi
  [[ -e "$marker" ]]
  grep -q '"event":"fired"' "$marker"

  start_temp_server "$root" "f03-forced-successor" 5
  successor_pid=$START_TEMP_SERVER_PID
  wait_for_socket "$socket"
  env JCODE_DEBUG_CONTROL=1 "$JCODE_BIN" --quiet --socket "$socket" debug shutdown:state >/dev/null
  if wait "$successor_pid"; then
    status=0
  else
    status=$?
  fi
  if [[ "$status" -ne 44 ]]; then
    printf 'lease fixture matrix: forced-exit successor exited with %d, expected 44\n' "$status" >&2
    return 1
  fi
  if [[ -e "$socket" ]]; then
    printf 'lease fixture matrix: forced-exit successor left socket residue: %s\n' "$socket" >&2
    return 1
  fi
  if [[ -e "$root/runtime/jcode-debug.sock" ]]; then
    printf 'lease fixture matrix: forced-exit successor left debug socket residue: %s\n' "$root/runtime/jcode-debug.sock" >&2
    return 1
  fi
  if [[ -e "$marker" ]]; then
    printf 'lease fixture matrix: forced-exit successor left shutdown marker residue: %s\n' "$marker" >&2
    return 1
  fi
  rm -rf "$root"

  root=$(mktemp -d "/tmp/jcode-f03-kill.XXXXXX")
  socket="$root/runtime/jcode.sock"
  cleanup_roots+=("$root")
  start_temp_server "$root" "f03-sigkill" 5
  pid=$START_TEMP_SERVER_PID
  wait_for_socket "$socket"
  kill -KILL "$pid"
  wait "$pid" || true
  if [[ ! -e "$socket" ]]; then
    printf 'lease fixture matrix: SIGKILL residue missing expected socket: %s\n' "$socket" >&2
    return 1
  fi

  start_temp_server "$root" "f03-sigkill-successor" 5
  successor_pid=$START_TEMP_SERVER_PID
  wait_for_socket "$socket"
  env JCODE_DEBUG_CONTROL=1 "$JCODE_BIN" --quiet --socket "$socket" debug shutdown:state >/dev/null
  if wait "$successor_pid"; then
    status=0
  else
    status=$?
  fi
  if [[ "$status" -ne 44 ]]; then
    printf 'lease fixture matrix: SIGKILL successor exited with %d, expected 44\n' "$status" >&2
    return 1
  fi
  if [[ -e "$socket" ]]; then
    printf 'lease fixture matrix: SIGKILL successor left socket residue: %s\n' "$socket" >&2
    return 1
  fi
  if [[ -e "$root/runtime/jcode-debug.sock" ]]; then
    printf 'lease fixture matrix: SIGKILL successor left debug socket residue: %s\n' "$root/runtime/jcode-debug.sock" >&2
    return 1
  fi
  rm -rf "$root"
}

cleanup_roots=()
cleanup() {
  local code=$?
  local root
  trap - EXIT INT TERM
  for root in "${cleanup_roots[@]}"; do
    rm -rf "$root"
  done
  exit "$code"
}
trap cleanup EXIT INT TERM

log "F03 lease fixture matrix: binary=$JCODE_BIN"
for class in "${lease_classes[@]}"; do
  log "lease class $class"
  lease_cycle_for_class "$class"
done

forced_exit_and_sigkill_cycles

log 'ALL F03 FIXTURES PASSED'
