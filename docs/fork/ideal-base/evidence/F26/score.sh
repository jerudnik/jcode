#!/usr/bin/env bash
# F26 scoreboard. Recomputes gate/matrix progress from command-log.txt.
# Usage: score.sh            -> print score
#        score.sh log "cmd"  -> run cmd, append result line, print score
set -uo pipefail
cd "$(dirname "$0")"
LOG=command-log.txt
touch "$LOG"
if [ "${1:-}" = "log" ]; then
  shift
  printf '=== %s\n$ %s\n' "$(date -u +%FT%TZ)" "$*" >> "$LOG"
  ( cd ../../../../.. && eval "$*" ) 2>&1 | tee /tmp/f26-last.txt | tail -40
  rc=${PIPESTATUS[0]}
  grep -E "^(test result|error(\[|:)|warning: unused)" /tmp/f26-last.txt >> "$LOG" || true
  printf 'exit=%s\n' "$rc" >> "$LOG"
fi
g=0
for k in GATE1 GATE2 GATE3; do
  if grep -q "^${k}: fail-before" "$LOG" && grep -q "^${k}: pass-after" "$LOG"; then g=$((g+1)); fi
done
m=$(grep -c "^MATRIX-ROW: " "$LOG" || true)
b=$(grep -c "^GATE3: workspace-build exit=0" "$LOG" || true)
echo "SCORE gates=${g}/3 matrix_rows=${m}/5 workspace_build_green=${b}"
