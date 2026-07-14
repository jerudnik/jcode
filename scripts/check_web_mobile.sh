#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP="$ROOT/web/jcode-mobile/app.js"
STATE="$ROOT/web/jcode-mobile/surface_state.mjs"
STATE_TEST="$ROOT/web/jcode-mobile/surface_state.test.mjs"
COMMANDS="$ROOT/web/jcode-mobile/surface_commands.mjs"
COMMANDS_TEST="$ROOT/web/jcode-mobile/surface_commands.test.mjs"
WORKSPACE_STORE="$ROOT/web/jcode-mobile/surface_workspace_store.mjs"
WORKSPACE_STORE_TEST="$ROOT/web/jcode-mobile/surface_workspace_store.test.mjs"
RENDERED_SMOKE="$ROOT/scripts/check_web_mobile_rendered.mjs"
INDEX="$ROOT/web/jcode-mobile/index.html"
STYLE="$ROOT/web/jcode-mobile/style.css"
DOC="$HOME/notes/projects/jcode/proposals/mobile-interface/web-mobile-mvp.md"

for file in "$APP" "$STATE" "$STATE_TEST" "$COMMANDS" "$COMMANDS_TEST" "$WORKSPACE_STORE" "$WORKSPACE_STORE_TEST" "$RENDERED_SMOKE" "$INDEX" "$STYLE" "$DOC"; do
  [[ -s "$file" ]] || { echo "missing or empty: $file" >&2; exit 1; }
done

node --check "$APP"
node --check "$STATE"
node --check "$STATE_TEST"
node --check "$COMMANDS"
node --check "$COMMANDS_TEST"
node --check "$WORKSPACE_STORE"
node --check "$WORKSPACE_STORE_TEST"
node --check "$RENDERED_SMOKE"
node --test "$STATE_TEST"
node --test "$COMMANDS_TEST"
node --test "$WORKSPACE_STORE_TEST"

required=(
  '@arrow-js/core@1.0.6'
  'POST'
  '/pair'
  'new WebSocket'
  '/ws?token='
  'type: "subscribe"'
  'type: "get_history"'
  'type: "message"'
  'type: "cancel"'
  'localStorage'
  'visibilitychange'
  'pageshow'
  'pagehide'
  'online'
  'offline'
  'pendingCommands'
)

for needle in "${required[@]}"; do
  grep -Fq "$needle" "$APP" "$STATE" || { echo "mobile app missing required token: $needle" >&2; exit 1; }
done

if grep -Eq '\?\.|\?\?|color-mix' "$APP" "$STATE" "$COMMANDS" "$WORKSPACE_STORE" "$STYLE"; then
  echo "found optional chaining/nullish/color-mix, avoid for older Android browser compatibility" >&2
  exit 1
fi

python3 - <<'PY' "$INDEX" "$DOC"
from pathlib import Path
import sys
for path in map(Path, sys.argv[1:]):
    text = path.read_text()
    if "jcode" not in text.lower():
        raise SystemExit(f"{path} does not look like a jcode artifact")
print("web mobile checks passed")
PY
