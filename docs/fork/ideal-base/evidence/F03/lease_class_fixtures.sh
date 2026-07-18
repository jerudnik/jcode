#!/usr/bin/env bash
# F03 lease-class and exit-mode fixture matrix (F01 design 4.3 runtime plan).
#
# Runs against the real daemon binary in fully isolated environments
# (env -i, private JCODE_RUNTIME_DIR + JCODE_HOME per fixture).
#
# Coverage:
#   A. Per-lease-class no-provider fixtures: for EVERY ActivityClass, hold
#      the lease via the debug socket past a short temporary idle timeout,
#      assert the daemon stays alive; release, assert exit code 44 and zero
#      socket/hash/metadata residue.
#   B. Forced-exit fixture: injected cleanup hang + SIGTERM; assert the
#      coordinator-armed watchdog fires with exit code 70 and the durable
#      marker records "fired".
#   C. Parent-SIGKILL residue fixture: SIGKILL the daemon (no cleanup runs),
#      assert a successor daemon on the same runtime dir boots, reaps the
#      stale socket, and serves; then exits cleanly.
#   D. Drain-refusal fixture: covered by unit tests (idle-claim atomicity,
#      refusal typing); recorded here for the matrix ledger.
#
# Usage: lease_class_fixtures.sh /path/to/jcode-binary
set -u

BINARY="${1:?usage: lease_class_fixtures.sh /path/to/jcode}"
FAILURES=0

LEASE_CLASSES=(
  client-connection
  provider-turn
  startup-recovery
  debug-job
  background-task
  mcp-call
  swarm-waiter
  scheduled-delivery
)

note() { printf '%s\n' "$*"; }
fail() { printf 'FAIL: %s\n' "$*"; FAILURES=$((FAILURES + 1)); }
pass() { printf 'PASS: %s\n' "$*"; }

residue_check() {
  local dir="$1" label="$2"
  local leftover socks
  leftover=$(find "$dir" -type f \( -name '*.sock.hash' -o -name '*.server.json' \) 2>/dev/null)
  socks=$(find "$dir" -type s 2>/dev/null)
  if [ -n "$leftover$socks" ]; then
    fail "$label residue remains: $leftover $socks"
  else
    pass "$label zero socket/hash/metadata residue"
  fi
}

# run_daemon <pid_var> <runtime_dir> <home_dir> [extra env pairs...] -- <args...>
run_daemon() {
  local pid_var="$1" dir="$2" home="$3"; shift 3
  local envpairs=()
  while [ "$1" != "--" ]; do envpairs+=("$1"); shift; done
  shift
  mkdir -p "$dir" "$home"
  env -i PATH="$PATH" HOME="$HOME" TMPDIR="${TMPDIR:-/tmp}" \
      JCODE_RUNTIME_DIR="$dir" JCODE_HOME="$home" JCODE_NO_TELEMETRY=1 \
      JCODE_DISABLE_UPDATE_CHECK=1 JCODE_DEFERRED_AUTH_BOOTSTRAP=1 \
      JCODE_DEBUG_CONTROL=1 \
      "${envpairs[@]+"${envpairs[@]}"}" \
      "$BINARY" "$@" >"$dir/daemon.log" 2>&1 &
  printf -v "$pid_var" '%s' "$!"
}

wait_socket() {
  local dir="$1" timeout="${2:-30}"
  for _ in $(seq 1 "$timeout"); do
    if [ -S "$dir/jcode-debug.sock" ]; then return 0; fi
    sleep 1
  done
  return 1
}

# debug_cmd <runtime_dir> <home_dir> <command>
debug_cmd() {
  local dir="$1" home="$2" cmd="$3"
  env -i PATH="$PATH" HOME="$HOME" TMPDIR="${TMPDIR:-/tmp}" \
      JCODE_RUNTIME_DIR="$dir" JCODE_HOME="$home" JCODE_NO_TELEMETRY=1 \
      JCODE_DEBUG_CONTROL=1 \
      "$BINARY" debug --no-update --quiet --socket "$dir/jcode.sock" "$cmd" 2>/dev/null
}

# wait_exit_var <result_var> <pid> <timeout_s>
# Sets result_var to the exit code, or empty on timeout. Runs entirely in the
# parent shell so `wait` can reap the background child (a $() subshell cannot
# wait on the parent's children; that produced spurious 127s).
wait_exit_var() {
  local result_var="$1" pid="$2" timeout="$3"
  printf -v "$result_var" '%s' ""
  for _ in $(seq 1 "$timeout"); do
    if ! kill -0 "$pid" 2>/dev/null; then
      local code
      wait "$pid" 2>/dev/null
      code=$?
      printf -v "$result_var" '%s' "$code"
      return 0
    fi
    sleep 1
  done
}

# ---------------------------------------------------------------------------
note "== A. Per-lease-class hold/release fixtures (idle timeout 5s) =="
for class in "${LEASE_CLASSES[@]}"; do
  DIR=$(mktemp -d); HOMEDIR=$(mktemp -d)
  run_daemon PID "$DIR" "$HOMEDIR" -- serve --temporary-server --temp-idle-timeout-secs 5
  if ! wait_socket "$DIR"; then
    fail "[$class] server socket never appeared"; tail -3 "$DIR/daemon.log"
    kill -9 "$PID" 2>/dev/null; rm -rf "$DIR" "$HOMEDIR"; continue
  fi

  TOKEN=$(debug_cmd "$DIR" "$HOMEDIR" "shutdown:hold_lease:$class" | python3 -c 'import json,sys;print(json.load(sys.stdin)["token"])' 2>/dev/null)
  if [ -z "$TOKEN" ]; then
    fail "[$class] could not acquire fixture lease"; kill -9 "$PID" 2>/dev/null; rm -rf "$DIR" "$HOMEDIR"; continue
  fi

  # Hold well past timeout (5s) + poll interval (10s): 18s total.
  sleep 18
  if ! kill -0 "$PID" 2>/dev/null; then
    fail "[$class] daemon exited while the lease was held"
    rm -rf "$DIR" "$HOMEDIR"; continue
  fi
  pass "[$class] daemon alive past idle timeout while leased"

  debug_cmd "$DIR" "$HOMEDIR" "shutdown:release_lease:$TOKEN" >/dev/null
  # Release starts a FULL new idle window (quiescence epoch): 5s window +
  # 10s poll granularity + margin.
  wait_exit_var CODE "$PID" 40
  if [ -z "$CODE" ]; then
    fail "[$class] daemon did not exit after release"; tail -3 "$DIR/daemon.log"; kill -9 "$PID" 2>/dev/null
  elif [ "$CODE" -eq 44 ]; then
    pass "[$class] exit 44 after release + full idle window"
    residue_check "$DIR" "[$class]"
  else
    fail "[$class] exit code $CODE, expected 44"
  fi
  rm -rf "$DIR" "$HOMEDIR"
done

# ---------------------------------------------------------------------------
note "== B. Forced-exit fixture (cleanup hang + SIGTERM -> watchdog 70) =="
DIR=$(mktemp -d); HOMEDIR=$(mktemp -d)
run_daemon PID "$DIR" "$HOMEDIR" JCODE_TEST_SHUTDOWN_CLEANUP_HANG_MS=30000 -- \
  serve --temporary-server --temp-idle-timeout-secs 600
if ! wait_socket "$DIR"; then
  fail "[forced] server socket never appeared"; kill -9 "$PID" 2>/dev/null
else
  kill -TERM "$PID"
  wait_exit_var CODE "$PID" 20
  if [ -z "$CODE" ]; then
    fail "[forced] daemon did not exit within 20s"; kill -9 "$PID" 2>/dev/null
  elif [ "$CODE" -eq 70 ]; then
    pass "[forced] watchdog forced exit code 70"
  else
    fail "[forced] exit code $CODE, expected 70"
  fi
  MARKER="$HOMEDIR/state/shutdown-watchdog.json"
  if [ -f "$MARKER" ] && grep -q '"event":"fired"' "$MARKER"; then
    pass "[forced] durable marker records fired"
  else
    fail "[forced] marker missing or not fired: $(cat "$MARKER" 2>/dev/null)"
  fi
fi
rm -rf "$DIR" "$HOMEDIR"

# ---------------------------------------------------------------------------
note "== C. Parent-SIGKILL residue fixture (successor reaps stale socket) =="
DIR=$(mktemp -d); HOMEDIR=$(mktemp -d)
run_daemon PID "$DIR" "$HOMEDIR" -- serve --temporary-server --temp-idle-timeout-secs 600
if ! wait_socket "$DIR"; then
  fail "[sigkill] server socket never appeared"; kill -9 "$PID" 2>/dev/null
else
  kill -KILL "$PID"
  wait "$PID" 2>/dev/null
  if [ -S "$DIR/jcode.sock" ]; then
    pass "[sigkill] stale socket residue present as expected after SIGKILL"
  else
    note "[sigkill] note: socket already gone (OS cleanup)"
  fi
  # Successor must boot on the same runtime dir despite the stale socket/lock.
  run_daemon PID2 "$DIR" "$HOMEDIR" -- serve --temporary-server --temp-idle-timeout-secs 5
  if wait_socket "$DIR" 30 && kill -0 "$PID2" 2>/dev/null; then
    pass "[sigkill] successor daemon booted over stale residue"
    wait_exit_var CODE "$PID2" 40
    if [ -n "$CODE" ] && [ "$CODE" -eq 44 ]; then
      pass "[sigkill] successor idle-exited 44 with clean residue"
      residue_check "$DIR" "[sigkill-successor]"
    else
      fail "[sigkill] successor exit code '$CODE', expected 44"
      kill -9 "$PID2" 2>/dev/null
    fi
  else
    fail "[sigkill] successor failed to boot over stale residue"
    tail -5 "$DIR/daemon.log"
    kill -9 "$PID2" 2>/dev/null
  fi
fi
rm -rf "$DIR" "$HOMEDIR"

# ---------------------------------------------------------------------------
if [ "$FAILURES" -eq 0 ]; then
  note "ALL F03 FIXTURES PASSED"
  exit 0
else
  note "$FAILURES fixture failure(s)"
  exit 1
fi
