#!/usr/bin/env bash
set -euo pipefail

branch="$(git branch --show-current)"
printf 'branch: %s\n' "$branch"

case "$branch" in
  vendor/upstream)
    echo "ERROR: vendor/upstream is clean upstream. Do not make downstream edits here." >&2
    exit 2
    ;;
  distro/nix)
    echo "OK: distro/nix. Packaging, flake, Home Manager, cache, and CI changes only."
    ;;
  main)
    echo "OK: main. Stable fork behavior and customizations belong here."
    ;;
  stack/*|pr/*|exp/*|shim/*)
    echo "OK: topic/review branch. Confirm intended base before merging."
    ;;
  archive/*|backup/*)
    echo "WARNING: archive/backup branches are safety snapshots, not active development." >&2
    ;;
  *)
    echo "WARNING: branch does not match the fork rail convention." >&2
    ;;
esac

echo
printf 'remotes:\n'
git remote -v
