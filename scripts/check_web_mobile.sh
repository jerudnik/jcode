#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP="$ROOT/web/jcode-mobile/app.js"
STATE="$ROOT/web/jcode-mobile/surface_state.mjs"
STATE_TEST="$ROOT/web/jcode-mobile/surface_state.test.mjs"
INDEX="$ROOT/web/jcode-mobile/index.html"
STYLE="$ROOT/web/jcode-mobile/style.css"
DOC="$ROOT/docs/WEB_MOBILE_MVP.md"

for file in "$APP" "$STATE" "$STATE_TEST" "$INDEX" "$STYLE" "$DOC"; do
  [[ -s "$file" ]] || { echo "missing or empty: $file" >&2; exit 1; }
done

node --check "$APP"
node --check "$STATE"
node --check "$STATE_TEST"
node --test "$STATE_TEST"

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

if grep -Eq '\?\.|\?\?|color-mix' "$APP" "$STATE" "$STYLE"; then
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
