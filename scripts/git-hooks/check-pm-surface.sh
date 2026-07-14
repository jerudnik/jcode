#!/usr/bin/env bash
# check-pm-surface.sh — enforce the notes/repo surface contract.
#
# Contract (see ~/notes/projects/jcode/project.md ## Conventions):
#   notes (~/notes/projects/jcode) = PM/tracking, stated once authoritatively.
#   repo docs (~/labs/jcode/docs)  = documentation.
#
# PM/tracking must not accrete in the repo. This hook blocks a commit that adds
# a NEW docs/ file whose primary purpose is tracking (a backlog, next-session
# handoff, workplan, or a checkbox-heavy TODO list). Findings/analysis docs are
# fine — their open-item tracking belongs in notes project.md, but the analysis
# itself is documentation and stays.
#
# Scope: only newly-ADDED files under docs/ (not archive/, which is historical).
# Modifications to existing docs are not blocked (migrate deliberately, not by
# hook). Set PM_SURFACE_OK=1 to bypass for a deliberate exception.

set -euo pipefail

[ "${PM_SURFACE_OK:-}" = "1" ] && exit 0

# Newly-added markdown files under docs/, excluding docs/archive/.
# (portable readarray: macOS ships bash 3.2 without mapfile)
added=()
while IFS= read -r line; do
  [ -n "$line" ] && added+=("$line")
done < <(
  git diff --cached --name-only --diff-filter=A -- 'docs/' ':(exclude)docs/archive/' 2>/dev/null \
    | grep -E '\.md$' || true
)
[ "${#added[@]}" -eq 0 ] && exit 0

violations=()
for f in "${added[@]}"; do
  [ -f "$f" ] || continue
  base="$(basename "$f")"
  # Title/filename signal: the doc announces itself as tracking.
  title="$(head -20 "$f" | grep -m1 -E '^#|^title:' || true)"
  name_hit=0
  if printf '%s\n%s\n' "$base" "$title" \
     | grep -qiE 'backlog|next[ _-]session|next[ _-]steps|work[ _-]?plan|kickstart|action[ _-]items|to[ _-]?do[ _-]?list|handoff|sprint|roadmap|task[ _-]list'; then
    name_hit=1
  fi
  # Body signal: dominated by open checkboxes (a tracking list, not prose).
  boxes="$(grep -cE '^\s*[-*] \[[ xX]\]' "$f" || true)"
  lines="$(grep -cE '\S' "$f" || echo 1)"
  box_hit=0
  if [ "$boxes" -ge 6 ] && [ "$lines" -gt 0 ] && [ $(( boxes * 100 / lines )) -ge 15 ]; then
    box_hit=1
  fi
  if [ "$name_hit" = 1 ] || [ "$box_hit" = 1 ]; then
    reason=""
    [ "$name_hit" = 1 ] && reason="title/name reads as tracking"
    [ "$box_hit" = 1 ] && reason="${reason:+$reason; }checkbox-heavy ($boxes items)"
    violations+=("$f — $reason")
  fi
done

[ "${#violations[@]}" -eq 0 ] && exit 0

cat >&2 <<'EOF'

✗ PM surface contract violation (docs/ is for documentation, not tracking).

The following newly-added repo docs read like PM/tracking. Per the surface
contract, PM/tracking lives in ~/notes/projects/jcode, stated once:

EOF
for v in "${violations[@]}"; do printf '    - %s\n' "$v" >&2; done
cat >&2 <<'EOF'

Fix one of:
  • Move the tracking to ~/notes/projects/jcode/ (proposals/ or project.md) and
    drop it from the repo. A findings/analysis doc may stay in the repo, but its
    open-item list belongs in notes.
  • If this genuinely is code-adjacent documentation misread by the heuristic,
    rename it away from tracking vocabulary, or bypass once with:
        PM_SURFACE_OK=1 git commit ...

EOF
exit 1
