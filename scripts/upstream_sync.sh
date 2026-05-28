#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_root"

execute=0
validate=0
base_ref="origin/master"
upstream_ref="upstream/master"
dev_branch="dev"
candidate_branch="sync/upstream-master"
allow_dirty=()

log() { printf '[upstream-sync] %s\n' "$*"; }
warn() { printf '[upstream-sync] warning: %s\n' "$*" >&2; }
die() { printf '[upstream-sync] error: %s\n' "$*" >&2; exit 1; }

usage() {
  cat <<'USAGE'
Usage: scripts/upstream_sync.sh [options]

Dry-run by default. Prints checks and planned commands without mutating history.

Options:
  --execute                 Run candidate fetch/switch/merge commands. Still never stages, commits, rebases dev, or pushes.
  --push                    Deprecated safety no-op. Prints manual push follow-up only; never pushes.
  --no-push                 Accepted for compatibility. Push execution is always disabled.
  --validate                Run lightweight self-validation for this helper only.
  --base <ref>              Candidate base ref, default origin/master.
  --upstream <ref>          Upstream ref to merge, default upstream/master.
  --dev <branch>            Dev branch name, default dev.
  --candidate <branch>      Local candidate branch, default sync/upstream-master.
  --allow-dirty <path>      Permit an uncommitted path during preflight. Repeatable.
  -h, --help                Show this help.

Safety model:
  - Defaults to dry-run.
  - Refuses mutating operations with unexpected dirty worktree paths.
  - Requires --execute for fetch/candidate-branch/merge planning to run.
  - Never rebases dev, stages, commits, conflict-resolves, or pushes.
  - Prints push commands only as explicit manual follow-up after operator review.
USAGE
}

quote_cmd() {
  printf '%q ' "$@"
  printf '\n'
}

run_or_print() {
  log "planned: $(quote_cmd "$@")"
  if [[ "$execute" -eq 1 ]]; then
    "$@"
  fi
}

path_allowed_dirty() {
  local path=$1 allowed normalized
  for allowed in "${allow_dirty[@]}"; do
    normalized=${allowed%/}
    [[ "$path" == "$normalized" ]] && return 0
    if [[ "$allowed" == */ && "$path" == "$normalized"/* ]]; then
      return 0
    fi
  done
  return 1
}

check_worktree() {
  local bad=0 line path
  while IFS= read -r line; do
    [[ -z "$line" ]] && continue
    path=${line:3}
    # Rename/copy statuses contain "old -> new". Treat the final path as the one to allow.
    if [[ "$path" == *" -> "* ]]; then
      path=${path##* -> }
    fi
    if ! path_allowed_dirty "$path"; then
      warn "dirty path not allowed: $line"
      bad=1
    fi
  done < <(git status --short --untracked-files=all)
  [[ "$bad" -eq 0 ]] || die "worktree has unexpected changes; commit/stash them or pass narrow --allow-dirty <path> for dry-run planning"
}

check_remotes() {
  git remote get-url origin >/dev/null 2>&1 || die "missing origin remote"
  git remote get-url upstream >/dev/null 2>&1 || die "missing upstream remote"

  local upstream_push
  upstream_push=$(git remote get-url --push upstream 2>/dev/null || true)
  if [[ -n "$upstream_push" && "$upstream_push" != "DISABLED" && "$upstream_push" != "no_push" && "$upstream_push" != "no_push_configured" ]]; then
    warn "upstream push URL is configured as: $upstream_push"
    warn "recommended: disable upstream pushes, e.g. git remote set-url --push upstream DISABLED"
    [[ "$execute" -eq 0 ]] || die "refusing --execute while upstream push URL is active"
  fi
}

check_rerere() {
  local enabled autoupdate
  enabled=$(git config --get rerere.enabled || true)
  autoupdate=$(git config --get rerere.autoupdate || true)
  [[ "$enabled" == "true" ]] || warn "rerere.enabled is not true; recommended: git config rerere.enabled true"
  [[ -z "$autoupdate" || "$autoupdate" == "false" ]] || warn "rerere.autoupdate is '$autoupdate'; recommended unset/false for reviewable conflict resolutions"
}

print_conflict_audit() {
  cat <<'AUDIT'
[upstream-sync] if conflicts or rerere reuse occur, audit before staging:
  git status --short
  git rerere status
  git rerere diff
  git rerere remaining
  git diff
  git rerere forget <pathspec>   # if a reused resolution is wrong
AUDIT
}

planned_validation() {
  cat <<'VALIDATION'
[upstream-sync] validation filters from docs/UPSTREAM_SYNC_RUNBOOK.md:
  Rust/Cargo/build:       scripts/dev_cargo.sh check; cargo build --profile selfdev -p jcode --bin jcode
  Scripts/workflows:      bash -n <script>; targeted --help or dry-run output
  Auth/provider/session:  scripts/auth_regression_matrix.sh or scripts/test_auth_e2e.sh when applicable
  Security-sensitive:     scripts/security_preflight.sh
  Budget-sensitive:       relevant scripts/check_*_budget.* helpers
  Docs-only:              markdown review and path/link sanity checks
VALIDATION
}

run_basic_validation() {
  log "running lightweight self-validation for this helper only"
  bash -n scripts/upstream_sync.sh
  planned_validation
}

parse_args() {
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --execute) execute=1; shift ;;
      --push) warn "--push is a safety no-op; this helper never pushes. Use printed manual follow-up after review."; shift ;;
      --no-push) shift ;;
      --validate) validate=1; shift ;;
      --base) base_ref=${2:?--base requires a ref}; shift 2 ;;
      --upstream) upstream_ref=${2:?--upstream requires a ref}; shift 2 ;;
      --dev) dev_branch=${2:?--dev requires a branch}; shift 2 ;;
      --candidate) candidate_branch=${2:?--candidate requires a branch}; shift 2 ;;
      --allow-dirty|--allow-dirty-doc) allow_dirty+=("${2:?--allow-dirty requires a path}"); shift 2 ;;
      -h|--help) usage; exit 0 ;;
      *) die "unknown option: $1" ;;
    esac
  done
}

main() {
  parse_args "$@"

  if [[ "$execute" -eq 1 ]]; then
    log "mode: execute"
  else
    log "mode: dry-run"
  fi
  log "push: disabled (manual follow-up only)"

  check_worktree
  check_remotes
  check_rerere

  log "base ref: $base_ref"
  log "upstream ref: $upstream_ref"
  log "dev branch: $dev_branch"
  log "candidate branch: $candidate_branch"

  run_or_print git fetch origin
  run_or_print git fetch upstream
  run_or_print git switch -C "$candidate_branch" "$base_ref"
  run_or_print git merge --no-commit --no-ff "$upstream_ref"
  print_conflict_audit

  if [[ "$execute" -eq 1 ]]; then
    cat <<'EXECUTE_NEXT'
[upstream-sync] execute mode stopped after candidate merge setup.
[upstream-sync] Next manual steps:
  - Audit merge/conflict state with the rerere and diff commands above.
  - Resolve conflicts if any, then run validation filters from the matrix below.
  - Commit the candidate integration only after review.
  - Update dev manually only after candidate validation, keeping local fixes separate.
EXECUTE_NEXT
    planned_validation
    if [[ "$validate" -eq 1 ]]; then
      run_basic_validation
    fi
    exit 0
  fi

  planned_validation
  if [[ "$validate" -eq 1 ]]; then
    run_basic_validation
  fi

  log "after candidate validation, planned dev update commands are:"
  log "planned: git switch $dev_branch"
  log "planned: git rebase $candidate_branch"
  print_conflict_audit

  log "manual follow-up push commands after review only:"
  log "manual: git push origin $candidate_branch"
  log "manual: git push --force-with-lease origin $dev_branch"

  cat <<'NEXT'
[upstream-sync] post-run checklist:
  - Review docs/UPSTREAM_SYNC_RUNBOOK.md sections 5-7.
  - Review git diff and git log --graph --oneline --decorate --max-count=30 --all.
  - Stage, commit, and push manually only after review.
NEXT
}

main "$@"
