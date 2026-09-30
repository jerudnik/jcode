#!/usr/bin/env bash
# Real-process reload fixture: no pooled or session-owned MCP child from the
# pre-exec daemon image may remain once the successor publishes socket-ready.

set -euo pipefail

if [[ $# -ne 1 ]]; then
  printf 'usage: %s <jcode-binary>\n' "$0" >&2
  exit 2
fi

JCODE_BIN=$1
if [[ ! -x "$JCODE_BIN" ]]; then
  printf 'reload MCP reap fixture: binary not executable: %s\n' "$JCODE_BIN" >&2
  exit 2
fi

REPO=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
ROOT=$(mktemp -d /tmp/jcode-reload-mcp-reap.XXXXXX)
HOME_DIR="$ROOT/home"
RUNTIME_DIR="$ROOT/runtime"
SOCKET="$RUNTIME_DIR/jcode.sock"
RELOAD_MARKER="$RUNTIME_DIR/jcode.reload"
SERVER_LOG="$ROOT/server.log"
FAKE_SERVER="$ROOT/fake-mcp-server.py"
POOLED_PID_FILE="$ROOT/pooled.pid"
OWNED_PID_FILE="$ROOT/owned.pid"
DAEMON_PID=
CHILD_PIDS=()

collect_child_pids() {
  local pid_file pid
  for pid_file in "$POOLED_PID_FILE" "$OWNED_PID_FILE"; do
    [[ -f "$pid_file" ]] || continue
    while IFS= read -r pid; do
      [[ -n "$pid" ]] && CHILD_PIDS+=("$pid")
    done <"$pid_file"
  done
}

cleanup() {
  local code=$?
  local pid
  trap - EXIT INT TERM
  collect_child_pids
  for pid in "${CHILD_PIDS[@]}"; do
    if [[ -n "$pid" ]] && kill -0 "$pid" >/dev/null 2>&1; then
      kill -TERM "$pid" >/dev/null 2>&1 || true
    fi
  done
  if [[ -n "$DAEMON_PID" ]] && kill -0 "$DAEMON_PID" >/dev/null 2>&1; then
    kill -TERM "$DAEMON_PID" >/dev/null 2>&1 || true
    wait "$DAEMON_PID" >/dev/null 2>&1 || true
  fi
  rm -rf "$ROOT"
  exit "$code"
}
trap cleanup EXIT INT TERM

mkdir -p "$HOME_DIR/.jcode" "$RUNTIME_DIR"

cat >"$FAKE_SERVER" <<'PY'
#!/usr/bin/env python3
import json
import os
import sys
import time

pid_file = sys.argv[1]
with open(pid_file, "a", encoding="utf-8") as handle:
    handle.write(f"{os.getpid()}\n")

while True:
    line = sys.stdin.readline()
    if not line:
        # A successful reload must explicitly reap us; inherited-image EOF is
        # intentionally ignored so the pre-fix leak remains observable.
        time.sleep(0.05)
        continue
    request = json.loads(line)
    method = request.get("method")
    request_id = request.get("id")
    if method == "initialize":
        response = {
            "jsonrpc": "2.0",
            "id": request_id,
            "result": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "serverInfo": {"name": "reload-reap-fixture", "version": "1"},
            },
        }
        print(json.dumps(response), flush=True)
    elif method == "tools/list":
        print(
            json.dumps(
                {"jsonrpc": "2.0", "id": request_id, "result": {"tools": []}}
            ),
            flush=True,
        )
    elif method == "shutdown":
        sys.exit(0)
PY
chmod +x "$FAKE_SERVER"

cat >"$HOME_DIR/.jcode/mcp.json" <<JSON
{
  "mcpServers": {
    "reload-reap-pooled": {
      "command": "$FAKE_SERVER",
      "args": ["$POOLED_PID_FILE"],
      "shared": true
    },
    "reload-reap-owned": {
      "command": "$FAKE_SERVER",
      "args": ["$OWNED_PID_FILE"],
      "shared": false
    }
  }
}
JSON

fixture_env=(
  env -i
  HOME="$HOME_DIR"
  JCODE_HOME="$HOME_DIR/.jcode"
  JCODE_RUNTIME_DIR="$RUNTIME_DIR"
  JCODE_DEBUG_CONTROL=1
  JCODE_DEFERRED_AUTH_BOOTSTRAP=1
  JCODE_NO_TELEMETRY=1
  JCODE_PROVIDER=jcode
  JCODE_REPO_DIR="$REPO"
  PATH="${PATH:-/usr/bin:/bin:/usr/sbin:/sbin}"
)

"${fixture_env[@]}" "$JCODE_BIN" serve \
  --provider jcode \
  --socket "$SOCKET" \
  --server-name f14-reload-mcp-reap \
  >"$SERVER_LOG" 2>&1 &
DAEMON_PID=$!

for _ in $(seq 1 200); do
  if "${fixture_env[@]}" "$JCODE_BIN" --quiet --socket "$SOCKET" \
      debug shutdown:state >/dev/null 2>&1; then
    break
  fi
  sleep 0.1
done
if ! "${fixture_env[@]}" "$JCODE_BIN" --quiet --socket "$SOCKET" \
    debug shutdown:state >/dev/null 2>&1; then
  printf 'reload MCP reap fixture: daemon socket never became ready\n' >&2
  tail -40 "$SERVER_LOG" >&2
  exit 1
fi

"${fixture_env[@]}" "$JCODE_BIN" --quiet --socket "$SOCKET" \
  debug "create_session:$REPO" >/dev/null

for _ in $(seq 1 200); do
  if [[ -s "$POOLED_PID_FILE" && -s "$OWNED_PID_FILE" ]]; then
    break
  fi
  sleep 0.1
done
if [[ ! -s "$POOLED_PID_FILE" || ! -s "$OWNED_PID_FILE" ]]; then
  printf 'reload MCP reap fixture: fake MCP children did not start\n' >&2
  tail -60 "$SERVER_LOG" >&2
  exit 1
fi

POOLED_PID=$(head -1 "$POOLED_PID_FILE")
OWNED_PID=$(head -1 "$OWNED_PID_FILE")
collect_child_pids
for pid in "${CHILD_PIDS[@]}"; do
  if ! kill -0 "$pid" >/dev/null 2>&1; then
    printf 'reload MCP reap fixture: fake MCP child %s exited before reload\n' "$pid" >&2
    exit 1
  fi
done

"${fixture_env[@]}" "$JCODE_BIN" --quiet --socket "$SOCKET" \
  server reload --force >/dev/null
collect_child_pids

for _ in $(seq 1 300); do
  if [[ -s "$RELOAD_MARKER" ]] \
      && jq -e '.phase == "socket_ready"' "$RELOAD_MARKER" >/dev/null 2>&1; then
    break
  fi
  sleep 0.1
done
if [[ ! -s "$RELOAD_MARKER" ]] \
    || ! jq -e '.phase == "socket_ready"' "$RELOAD_MARKER" >/dev/null 2>&1; then
  printf 'reload MCP reap fixture: successor never published socket-ready\n' >&2
  if [[ -s "$RELOAD_MARKER" ]]; then
    cat "$RELOAD_MARKER" >&2
  fi
  if kill -0 "$DAEMON_PID" >/dev/null 2>&1; then
    printf 'reload MCP reap fixture: daemon PID %s is still live\n' "$DAEMON_PID" >&2
  else
    printf 'reload MCP reap fixture: daemon PID %s exited\n' "$DAEMON_PID" >&2
  fi
  tail -80 "$SERVER_LOG" >&2
  find "$HOME_DIR/.jcode" -maxdepth 3 -type f -print >&2
  for diagnostic in "$HOME_DIR/.jcode/logs/"*.log "$HOME_DIR/.jcode/reload-traces/"*.jsonl; do
    if [[ -f "$diagnostic" ]]; then
      printf '%s\n' "--- $diagnostic" >&2
      tail -120 "$diagnostic" >&2
    fi
  done
  exit 1
fi

for pid in "${CHILD_PIDS[@]}"; do
  if kill -0 "$pid" >/dev/null 2>&1; then
    printf 'reload MCP reap fixture: pre-reload MCP child %s survived successor readiness\n' "$pid" >&2
    tail -80 "$SERVER_LOG" >&2
    exit 1
  fi
done

"${fixture_env[@]}" "$JCODE_BIN" --quiet --socket "$SOCKET" \
  server stop --force >/dev/null
wait "$DAEMON_PID"
DAEMON_PID=

printf 'reload MCP reap fixture: successor ready and pre-reload MCP children exited (%s, %s)\n' \
  "$POOLED_PID" "$OWNED_PID"
