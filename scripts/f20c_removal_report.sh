#!/usr/bin/env bash
# Regenerate the F20c removal grep-clean report.
#
# F20c retired the distribution surface: the GitHub-release acquisition
# subsystem and the multi-channel/version binary store. The claim that the
# removal is complete has to stay checkable after the fact, so this script
# regenerates the report from the tree rather than leaving a hand-written
# snapshot that silently rots as HEAD moves.
#
# It exits non-zero if any retired symbol has come back, which makes it usable
# as a regression check and not only as an evidence generator.
#
# Usage:
#   scripts/f20c_removal_report.sh              # write the evidence file
#   scripts/f20c_removal_report.sh --stdout     # print instead
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

OUT="docs/fork/ideal-base/evidence/F20c/removal-grep-clean.txt"
[ "${1:-}" = "--stdout" ] && OUT=/dev/stdout

# Symbols that made up the retired surface. Each must have zero references.
SYMBOLS=(
  install_binary_at_version install_version install_local_release
  update_stable_symlink update_current_symlink update_shared_server_symlink
  update_canary_symlink promote_version_to_shared_server
  advance_shared_server_if_tracking_stable repair_stale_shared_server_channel
  reconcile_stale_pending_activation PendingActivation CanaryStatus CrashInfo
  BinaryChoice stable_binary_path current_binary_path shared_server_binary_path
  canary_binary_path stable_version_file current_version_file
  read_stable_version jcode_update_core jcode-update-core
  version_matches_installed_channel normalize_version_marker
  update_launcher_symlink_to_stable cleanup_empty_dir
)

# Scope the scan to git-tracked files. Scanning the working directory instead
# picks up untracked scratch (stale clippy logs under .jcode-tmp/ still name
# the deleted crate), which is noise, not a surviving reference.
#
# docs/ records history and this script itself names every retired symbol, so
# both are excluded or the report would always fail.
# .rerere-cache/ holds recorded git merge resolutions: historical conflict
# blobs, not shipping source.
#
# `git grep` is used rather than assembling an argv of tracked paths: it is
# tracked-only by default, applies the pathspecs natively, and cannot blow
# ARG_MAX as the tree grows.
PATHSPECS=(-- . ':!:docs/**' ':!:.rerere-cache/**' ':!:scripts/f20c_removal_report.sh')

# Count tracked files containing $1. `git grep` exits 1 on no match, which is
# the expected (clean) case, so the status is deliberately discarded here and
# the count is what decides pass/fail.
count_refs() {
  git grep -I -l -F -e "$1" "${PATHSPECS[@]}" 2>/dev/null | wc -l | tr -d ' '
}

total=0
report=$(
  printf 'SYMBOL                                         REFS\n'
  printf -- '---------------------------------------------- ----\n'
  for symbol in "${SYMBOLS[@]}"; do
    count=$(count_refs "$symbol")
    printf '%-46s %s\n' "$symbol" "$count"
    total=$((total + count))
  done
  printf '\nTOTAL surviving references: %s\n' "$total"
)
# The loop above runs in a subshell, so recompute the total for the exit status.
total=$(printf '%s\n' "$report" | awk '/^TOTAL surviving/ {print $4}')

artifacts=$(
  printf 'Deleted artifacts:\n'
  for path in crates/jcode-update-core examples/promote_build.rs; do
    if [ -e "$path" ]; then
      printf '  FAIL  %s still present\n' "$path"
    else
      printf '  ok  %s absent\n' "$path"
    fi
  done
)

{
  printf 'F20c removal grep-clean report\n'
  printf 'generated: %s  HEAD=%s\n\n' \
    "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$(git rev-parse --short HEAD)"
  cat <<'PREAMBLE'
Every symbol below was part of the retired distribution surface:
the GitHub-release acquisition subsystem and the multi-channel/version
binary store. Scope: all git-tracked files, excluding docs/ (records
history), .rerere-cache/ (git merge-resolution blobs), and the generator
script itself (it names every symbol).

Regenerate with scripts/f20c_removal_report.sh, which exits non-zero if
any retired symbol returns.

PREAMBLE
  printf '%s\n\n' "$report"
  printf '%s\n' "$artifacts"
  cat <<'TRAILER'

Grep proves the symbols are gone; it cannot prove no writer recreates
the on-disk layout. That is asserted executably instead:

  tests/test_r10_release_acquisition.py
    after a full install, builds/{versions,stable,current,*-version} must
    not exist and no staged temp may survive
  scripts/test_install_release.sh
    same assertions for the local-release installer
  .github/scripts/verify_windows_install.ps1
    same assertions for the Windows installer
  scripts/test_reload.py (check 9)
    a live machine must have no retired channel present
  jcode-build-support tests::retired_version_store_is_detected_and_sized
    leftovers from a pre-F20c machine are found, sized, and reported
TRAILER
} > "$OUT"

if [ "$OUT" != /dev/stdout ]; then
  printf 'wrote %s (surviving references: %s)\n' "$OUT" "$total"
fi

if [ "$total" != "0" ] || printf '%s' "$artifacts" | grep -q FAIL; then
  # Name the offenders on stderr so the failure is actionable even when the
  # report itself is discarded (which is how preflight runs this).
  {
    printf 'ERROR: retired distribution surface has returned.\n'
    printf '%s\n' "$report" | awk '$2 ~ /^[1-9]/ {printf "  %s: %s reference(s)\n", $1, $2}'
    printf '%s\n' "$artifacts" | grep FAIL || true
  } >&2
  exit 1
fi
