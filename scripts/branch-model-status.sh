#!/usr/bin/env bash
set -euo pipefail

repo="${1:-jerudnik/jcode}"
upstream_remote="${UPSTREAM_REMOTE:-upstream}"
origin_remote="${ORIGIN_REMOTE:-origin}"
upstream_branch="${UPSTREAM_BRANCH:-master}"
main_branch="${MAIN_BRANCH:-main}"
nix_branch="${NIX_BRANCH:-nix-flake}"
scheduler_path=".github/workflows/fork-maintenance.yml"

section() {
  printf '\n== %s ==\n' "$1"
}

ref_exists() {
  git show-ref --verify --quiet "$1"
}

git fetch --prune "$origin_remote" >/dev/null
git fetch --prune "$upstream_remote" >/dev/null

origin_main="$origin_remote/$main_branch"
origin_nix="$origin_remote/$nix_branch"
upstream_ref="$upstream_remote/$upstream_branch"

section "Mirror status"
if ! ref_exists "refs/remotes/$origin_main"; then
  echo "missing $origin_main" >&2
  exit 1
fi
if ! ref_exists "refs/remotes/$upstream_ref"; then
  echo "missing $upstream_ref" >&2
  exit 1
fi

main_sha="$(git rev-parse "$origin_main")"
upstream_sha="$(git rev-parse "$upstream_ref")"
printf '%-18s %s\n' "$origin_main" "$main_sha"
printf '%-18s %s\n' "$upstream_ref" "$upstream_sha"

fork_only_paths="$(git diff --name-only "$upstream_ref..$origin_main" || true)"
unexpected_paths="$(printf '%s\n' "$fork_only_paths" | sed '/^$/d' | grep -v -x "$scheduler_path" || true)"

if [ "$main_sha" = "$upstream_sha" ]; then
  echo "OK: $origin_main exactly matches $upstream_ref"
elif [ -z "$unexpected_paths" ]; then
  echo "OK: $origin_main only differs by $scheduler_path"
else
  echo "WARN: $origin_main has unexpected fork-only changes"
  echo "unexpected paths:"
  echo "$unexpected_paths"
  echo "fork-only commits on $origin_main:"
  git log --oneline "$upstream_ref..$origin_main" || true
  echo "upstream commits not yet in $origin_main:"
  git log --oneline "$origin_main..$upstream_ref" || true
fi

section "$nix_branch payload beyond $main_branch"
if ref_exists "refs/remotes/$origin_nix"; then
  git log --oneline --decorate "$origin_main..$origin_nix" || true
  echo
  git diff --stat "$origin_main..$origin_nix" || true
else
  echo "missing $origin_nix"
fi

section "Open PRs by base"
if command -v gh >/dev/null 2>&1; then
  for base in "$main_branch" "$nix_branch"; do
    echo "-- base: $base"
    gh pr list --repo "$repo" --base "$base" --state open --json number,title,headRefName,isDraft \
      --jq '.[] | "#\(.number) [\(if .isDraft then "draft" else "ready" end)] \(.headRefName): \(.title)"' || true
  done

  section "Recent Nix workflow runs"
  gh run list --repo "$repo" --workflow Nix --limit 5 \
    --json databaseId,headBranch,status,conclusion,createdAt,event \
    --jq '.[] | "#\(.databaseId) \(.createdAt) \(.headBranch) \(.event): \(.status)/\(.conclusion // "-")"' || true
else
  echo "gh not found; skipping PR and workflow summaries"
fi
