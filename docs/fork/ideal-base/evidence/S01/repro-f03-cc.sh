#!/usr/bin/env bash
# S01-FIX-1 reproducer for the intermittent F03 client-connection failure.
#
# Round 2 of the S01-FIX-1 sweep read:
#   FAIL: [client-connection] daemon exited within 4s of release
#         (idle window not restarted)
# at the same commit and binary where round 1 read PASS for the same
# assertion. This script replays ONLY that lease class, N times, and keeps
# the daemon log of every iteration so a failure is diagnosable. The F03
# fixture deletes its runtime dir on this failure path, which is why the
# sweep produced a verdict with no evidence behind it.
#
# Usage: repro-f03-cc.sh <iterations> [binary]
set -u

N="${1:?usage: repro-f03-cc.sh <iterations> [binary]}"
# HOLD is the differential knob. The mechanism claim is that failure depends on
# whether a poll tick with elapsed>=timeout lands while the lease is still held
# (which forces a refusal and restarts the window). Raising HOLD past one more
# 10s poll interval should drive the failure rate to zero without touching any
# product code. HOLD=18 is the F03 fixture value.
HOLD="${HOLD:-18}"
BINARY="${2:-$PWD/target/selfdev/jcode}"
[ -x "$BINARY" ] || { echo "FATAL: no binary at $BINARY"; exit 1; }

OUT=$(mktemp -d "${TMPDIR:-/tmp}/s01-repro-XXXXXX") || { echo "FATAL: mktemp"; exit 1; }
echo "repro output dir: $OUT"
echo "binary: $BINARY"
echo "hold: ${HOLD}s"

PASSES=0
FAILS=0

for i in $(seq 1 "$N"); do
  DIR="$OUT/iter$i/rt"; HOMEDIR="$OUT/iter$i/home"
  mkdir -p "$DIR" "$HOMEDIR"

  env -i PATH="$PATH" HOME="$HOME" TMPDIR="${TMPDIR:-/tmp}" \
      JCODE_RUNTIME_DIR="$DIR" JCODE_HOME="$HOMEDIR" JCODE_NO_TELEMETRY=1 \
      JCODE_DISABLE_UPDATE_CHECK=1 JCODE_DEFERRED_AUTH_BOOTSTRAP=1 \
      JCODE_DEBUG_CONTROL=1 \
      "$BINARY" serve --temporary-server --temp-idle-timeout-secs 5 \
      >"$DIR/daemon.log" 2>&1 &
  PID=$!

  ok=1
  for _ in $(seq 1 30); do
    [ -S "$DIR/jcode-debug.sock" ] && { ok=0; break; }
    sleep 1
  done
  if [ "$ok" -ne 0 ]; then
    echo "iter $i: SOCKET-NEVER-APPEARED"; FAILS=$((FAILS+1))
    kill -9 "$PID" 2>/dev/null; continue
  fi

  TOKEN=$(env -i PATH="$PATH" HOME="$HOME" TMPDIR="${TMPDIR:-/tmp}" \
      JCODE_RUNTIME_DIR="$DIR" JCODE_HOME="$HOMEDIR" JCODE_NO_TELEMETRY=1 \
      JCODE_DEBUG_CONTROL=1 \
      "$BINARY" debug --quiet --socket "$DIR/jcode.sock" \
      "shutdown:hold_lease:client-connection" 2>>"$DIR/debug.stderr" \
      | python3 -c 'import json,sys;print(json.load(sys.stdin)["token"])' 2>/dev/null)
  if [ -z "$TOKEN" ]; then
    echo "iter $i: NO-LEASE ($(tr '\n' ' ' < "$DIR/debug.stderr" | tail -c 200))"
    FAILS=$((FAILS+1)); kill -9 "$PID" 2>/dev/null; continue
  fi

  sleep "$HOLD"
  if ! kill -0 "$PID" 2>/dev/null; then
    echo "iter $i: DIED-WHILE-LEASED"; FAILS=$((FAILS+1)); continue
  fi

  # Timestamp the release so the daemon log can be read against it.
  date +%s.%N > "$DIR/release_at"
  env -i PATH="$PATH" HOME="$HOME" TMPDIR="${TMPDIR:-/tmp}" \
      JCODE_RUNTIME_DIR="$DIR" JCODE_HOME="$HOMEDIR" JCODE_NO_TELEMETRY=1 \
      JCODE_DEBUG_CONTROL=1 \
      "$BINARY" debug --quiet --socket "$DIR/jcode.sock" \
      "shutdown:release_lease:$TOKEN" >/dev/null 2>>"$DIR/debug.stderr"

  sleep 4
  if ! kill -0 "$PID" 2>/dev/null; then
    echo "iter $i: FAIL exited within 4s of release   (log: $DIR/daemon.log)"
    FAILS=$((FAILS+1))
    continue
  fi
  echo "iter $i: PASS"
  PASSES=$((PASSES+1))
  kill -9 "$PID" 2>/dev/null
  wait "$PID" 2>/dev/null
done

echo "S01_REPRO: hold=${HOLD}s pass=$PASSES fail=$FAILS of $N   dir=$OUT"
[ "$FAILS" -eq 0 ]
