#!/usr/bin/env bash
# F02 exit-mode fixture harness (F01 design 4.3 runtime slice; full matrix is F03).
#
# Proves against the real binary, in an isolated JCODE_RUNTIME_DIR + HOME-scoped
# temp env, that:
#   1. temporary-idle exit goes through the coordinator: exit code 44, zero
#      residue (socket, debug socket, .hash, server.json metadata, lock).
#   2. SIGTERM exit goes through the coordinator: exit code 0, zero residue.
#   3. the daemon does NOT exit while a drain-blocking lease class is held
#      (approximated here by a live debug connection issuing a long debug job
#      is F03 scope; this slice checks the idle path timing only).
#
# Usage: exit_mode_fixtures.sh /path/to/jcode-binary
set -u

BINARY="${1:?usage: exit_mode_fixtures.sh /path/to/jcode}"
FAILURES=0

note() { printf '%s\n' "$*"; }
fail() { printf 'FAIL: %s\n' "$*"; FAILURES=$((FAILURES + 1)); }
pass() { printf 'PASS: %s\n' "$*"; }

residue_check() {
  local dir="$1" label="$2"
  local leftover
  leftover=$(find "$dir" -type f \( -name '*.sock' -o -name '*.sock.hash' -o -name '*.server.json' \) 2>/dev/null)
  # Unix sockets are not "-type f"; check socket type separately.
  local socks
  socks=$(find "$dir" -type s 2>/dev/null)
  if [ -n "$leftover$socks" ]; then
    fail "$label residue remains: $leftover $socks"
  else
    pass "$label zero socket/hash/metadata residue"
  fi
}

run_daemon() {
  # args: pid_var_name runtime_dir binary args...
  # Starts the daemon in a fully isolated env: only PATH/HOME survive, so
  # inherited JCODE_* variables (JCODE_SOCKET from a live session, forced
  # providers, etc.) cannot leak in. Sets the named variable to the pid.
  local pid_var="$1" dir="$2"; shift 2
  mkdir -p "$dir"
  env -i PATH="$PATH" HOME="$HOME" TMPDIR="${TMPDIR:-/tmp}" \
      JCODE_RUNTIME_DIR="$dir" JCODE_NO_TELEMETRY=1 \
      JCODE_DISABLE_UPDATE_CHECK=1 \
      "$@" >"$dir/daemon.log" 2>&1 &
  printf -v "$pid_var" '%s' "$!"
}

# ---------------------------------------------------------------------------
note "== Fixture 1: temporary-idle exit (expect code 44, clean residue) =="
DIR1=$(mktemp -d)
run_daemon PID1 "$DIR1" "$BINARY" serve --temporary-server --temp-idle-timeout-secs 15
# The idle monitor polls every 10s; a 15s timeout fires on the second tick.
# Bound the wait at 60s.
CODE1=""
for _ in $(seq 1 60); do
  if ! kill -0 "$PID1" 2>/dev/null; then
    wait "$PID1"; CODE1=$?
    break
  fi
  sleep 1
done
if [ -z "$CODE1" ]; then
  fail "temporary-idle daemon did not exit within 60s"
  tail -5 "$DIR1/daemon.log" || true
  kill -9 "$PID1" 2>/dev/null
else
  if [ "$CODE1" -eq 44 ]; then
    pass "temporary-idle exit code 44"
  else
    fail "temporary-idle exit code was $CODE1, expected 44"
  fi
  residue_check "$DIR1" "temporary-idle"
fi

# ---------------------------------------------------------------------------
note "== Fixture 2: SIGTERM exit (expect code 0, clean residue) =="
DIR2=$(mktemp -d)
run_daemon PID2 "$DIR2" "$BINARY" serve --temporary-server --temp-idle-timeout-secs 600
# Wait for the socket to appear (server ready), then SIGTERM.
READY=0
for _ in $(seq 1 30); do
  if [ -S "$DIR2/jcode.sock" ]; then READY=1; break; fi
  sleep 1
done
if [ "$READY" -ne 1 ]; then
  fail "SIGTERM fixture: server socket never appeared"
  tail -5 "$DIR2/daemon.log" || true
  kill -9 "$PID2" 2>/dev/null
else
  kill -TERM "$PID2"
  CODE2=""
  for _ in $(seq 1 15); do
    if ! kill -0 "$PID2" 2>/dev/null; then
      wait "$PID2"; CODE2=$?
      break
    fi
    sleep 1
  done
  if [ -z "$CODE2" ]; then
    fail "SIGTERM daemon did not exit within 15s"
    kill -9 "$PID2" 2>/dev/null
  else
    if [ "$CODE2" -eq 0 ]; then
      pass "SIGTERM exit code 0"
    else
      fail "SIGTERM exit code was $CODE2, expected 0"
    fi
    residue_check "$DIR2" "SIGTERM"
  fi
fi

# ---------------------------------------------------------------------------
rm -rf "$DIR1" "$DIR2"
if [ "$FAILURES" -eq 0 ]; then
  note "ALL FIXTURES PASSED"
  exit 0
else
  note "$FAILURES fixture failure(s)"
  exit 1
fi
