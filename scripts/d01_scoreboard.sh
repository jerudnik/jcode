#!/usr/bin/env bash
# D01 scoreboard: five integers that must all reach 0.
# Rerun after every change. Progress is these numbers, never prose.
set -uo pipefail
cd "$(dirname "$0")/.."

AUDIT=docs/fork/ideal-base/D01_DOCUMENTATION_AUDIT.md
list=$(python3 scripts/check_docs_references.py --list 2>/dev/null)

broken=$(printf '%s\n' "$list" | grep -c '^broken-link:' || true)
machine=$(printf '%s\n' "$list" | grep -c '^machine-local:' || true)
retired=$(printf '%s\n' "$list" | grep -c '^retired-rail:' || true)

# Open findings: disposition rows still marked `confirmed`.
open=$(grep -cE '^\| `D01-F[0-9]+` \| `confirmed`' "$AUDIT" || true)
partial=$(grep -cE '^\| `D01-F[0-9]+` \| `partially delivered`' "$AUDIT" || true)
open=$((open + partial))

# Product defects found by D01 but owned elsewhere. Not part of the D01 total,
# printed so that referring a defect can never be a way of hiding it.
referred=$(grep -cE '^\| `D01-F[0-9]+` \| `referred`' "$AUDIT" || true)

# Enforcement: 0 = advisory (prevents nothing), 1 = gating.
enforced=$(grep -c 'check_docs_references' .github/workflows/fork-ci.yml 2>/dev/null || true)
[ "$enforced" -gt 0 ] && enforced=1
unenforced=$((1 - enforced))

printf 'broken-link       %3d  (fatal; must stay 0)\n' "$broken"
printf 'retired-rail      %3d  (fatal; must stay 0)\n' "$retired"
printf 'machine-local     %3d  (ratchet; must reach 0)\n' "$machine"
printf 'open findings     %3d  (confirmed + partially delivered)\n' "$open"
printf 'not-enforced      %3d  (1 until wired into fork-ci.yml)\n' "$unenforced"
printf 'referred out      %3d  (product defects; tracked, not counted here)\n' "$referred"

total=$((broken + retired + machine + open + unenforced))
printf '\nD01 TOTAL         %3d  %s\n' "$total" \
  "$([ "$total" -eq 0 ] && echo 'COMPLETE' || echo 'incomplete')"
[ "$total" -eq 0 ]
