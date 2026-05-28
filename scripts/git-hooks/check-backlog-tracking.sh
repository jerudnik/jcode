#!/usr/bin/env sh
# check-backlog-tracking.sh
#
# Warn-only detector for "tracking divergence": newly-added unchecked
# backlog-tracking-ignore
# checklists, unguarded actionable markers (TODO/FIXME/HACK/XXX), and empty
# "Tracked in:" pointers in tracked files. These should be filed as
# Backlog.md tasks (https://backlog.md) and referenced as
# `Tracked in: TASK-NN`, or explicitly opted-out with a comment marker.
#
# Modes:
#   (no args)          scan staged changes (intended for pre-commit hook)
#   --all              scan all tracked files (intended for CI)
#   <path> [<path>...] scan explicit files (intended for ad-hoc / smoke tests)
#
# Flags:
#   --strict           exit non-zero on findings (useful in CI)
#
# Environment:
#   EXIT_ON_FINDING=1  same effect as --strict
#
# Warn-only by default. To convert into a blocking check globally, either
# pass --strict, set EXIT_ON_FINDING=1, or change the final `exit 0` below
# to `exit "$rc"`.
#
# Opt-out: if the offending line OR the line immediately preceding it
# contains one of these markers (case-insensitive), the line is ignored:
#   <!-- backlog-tracking-ignore -->
#   # backlog-tracking-ignore
#   // backlog-tracking-ignore
#
# Dependencies: POSIX sh, git, ripgrep (rg).

set -u

STRICT=0
if [ "${EXIT_ON_FINDING:-0}" = "1" ]; then
  STRICT=1
fi

MODE="staged"
# Use a temp file for explicit paths so subshells can read it portably.
EXPLICIT_LIST=""

while [ $# -gt 0 ]; do
  case "$1" in
    --all)    MODE="all" ;;
    --strict) STRICT=1 ;;
    --help|-h)
      sed -n '2,33p' "$0"
      exit 0
      ;;
    --)
      shift
      while [ $# -gt 0 ]; do
        [ -z "$EXPLICIT_LIST" ] && EXPLICIT_LIST="$(mktemp -t backlog-tracking-paths.XXXXXX)"
        printf '%s\n' "$1" >>"$EXPLICIT_LIST"
        MODE="explicit"
        shift
      done
      ;;
    -*) printf '[backlog-tracking] unknown flag: %s\n' "$1" >&2; exit 2 ;;
    *)
      [ -z "$EXPLICIT_LIST" ] && EXPLICIT_LIST="$(mktemp -t backlog-tracking-paths.XXXXXX)"
      printf '%s\n' "$1" >>"$EXPLICIT_LIST"
      MODE="explicit"
      ;;
  esac
  shift
done

cleanup() { [ -n "$EXPLICIT_LIST" ] && rm -f "$EXPLICIT_LIST"; rm -f "${FINDINGS_FILE:-}"; }
trap cleanup EXIT INT TERM

if ! command -v rg >/dev/null 2>&1; then
  printf '[backlog-tracking] warn: ripgrep (rg) not found; skipping checks\n' >&2
  exit 0
fi

REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || true)"
if [ -z "$REPO_ROOT" ]; then
  printf '[backlog-tracking] warn: not inside a git repository; skipping checks\n' >&2
  exit 0
fi
cd "$REPO_ROOT" || exit 0

# Findings tally lives in a temp file so subshell-piped loops can append to it.
FINDINGS_FILE="$(mktemp -t backlog-tracking-findings.XXXXXX)"

# Extension filter.
ext_match() {
  case "$1" in
    *.md|*.txt|*.rs|*.toml|*.nix|*.sh|*.py|*.js|*.ts|*.tsx) return 0 ;;
    *) return 1 ;;
  esac
}

# Path filter: skip the Backlog.md store itself (it is the source of truth
# for tasks and naturally contains unchecked ACs).
path_excluded() {
  case "$1" in
    .backlog/*|backlog/*) return 0 ;;
    *) return 1 ;;
  esac
}

# Patterns (PCRE2 / rg).
PAT_CHECKBOX='^\s*-\s*\[\s\]'
# backlog-tracking-ignore
PAT_TODO='(?i)\b(todo|fixme|hack|xxx)\b'
PAT_TRACKED_EMPTY='^\s*Tracked in:\s*$'
PAT_IGNORE='(?i)backlog-tracking-ignore'

emit() {
  # emit <file> <line> <reason> <content>
  printf '[backlog-tracking] warn %s:%s: %s: %s\n' "$1" "$2" "$3" "$4" >&2
  printf "  -> File a Backlog.md task (https://backlog.md) and reference it as 'Tracked in: TASK-NN' instead, or guard with <!-- backlog-tracking-ignore --> if intentional.\n" >&2
  printf 'x\n' >>"$FINDINGS_FILE"
}

has_ignore() {
  printf '%s' "$1" | rg -q -P "$PAT_IGNORE"
}

# inspect_line <file> <lineno> <content> <prev_line>
inspect_line() {
  file=$1; lineno=$2; content=$3; prev=$4
  if has_ignore "$content"; then return 0; fi
  if [ -n "$prev" ] && has_ignore "$prev"; then return 0; fi
  if printf '%s' "$content" | rg -q -P "$PAT_CHECKBOX"; then
    emit "$file" "$lineno" "new unchecked checklist item" "$content"
    return 0
  fi
  if printf '%s' "$content" | rg -q -P "$PAT_TRACKED_EMPTY"; then
    emit "$file" "$lineno" "empty Tracked-in pointer" "$content"
    return 0
  fi
  if printf '%s' "$content" | rg -q -P "$PAT_TODO"; then
    emit "$file" "$lineno" "unguarded actionable marker" "$content"
    return 0
  fi
}

scan_staged() {
  cur_file=""
  cur_lineno=0
  prev_line=""
  # -U0: no surrounding context so every `+` is a real addition.
  git diff --cached -U0 --no-color --diff-filter=AM | while IFS= read -r raw; do
    case "$raw" in
      '+++ b/'*)
        cur_file=${raw#+++ b/}
        cur_lineno=0
        prev_line=""
        if path_excluded "$cur_file"; then
          cur_file=""
        fi
        ;;
      '+++ /dev/null'|'--- '*)
        :
        ;;
      '@@ '*)
        hunk=${raw#@@ }
        plus=${hunk#*+}
        plus=${plus%% *}
        case "$plus" in
          *,*) cur_lineno=${plus%,*} ;;
          *)   cur_lineno=$plus ;;
        esac
        prev_line=""
        ;;
      '+'*)
        content=${raw#+}
        if [ -n "$cur_file" ] && ext_match "$cur_file"; then
          inspect_line "$cur_file" "$cur_lineno" "$content" "$prev_line"
        fi
        prev_line=$content
        cur_lineno=$((cur_lineno + 1))
        ;;
      *)
        :
        ;;
    esac
  done
}

# scan_file_with_context <file>
# Use rg to find candidate lines, then re-read previous physical line for
# the context-sensitive ignore marker.
scan_file_with_context() {
  f=$1
  [ -f "$f" ] || return 0
  rg -n --no-heading -P \
     -e "$PAT_CHECKBOX" \
     -e "$PAT_TODO" \
     -e "$PAT_TRACKED_EMPTY" \
     -- "$f" 2>/dev/null | while IFS=: read -r lineno content; do
    [ -n "$lineno" ] || continue
    case "$lineno" in
      ''|*[!0-9]*) continue ;;
    esac
    prev=""
    if [ "$lineno" -gt 1 ]; then
      prev=$(sed -n "$((lineno - 1))p" "$f" 2>/dev/null || true)
    fi
    inspect_line "$f" "$lineno" "$content" "$prev"
  done
}

scan_all() {
  git ls-files | while IFS= read -r f; do
    ext_match "$f" || continue
    path_excluded "$f" && continue
    scan_file_with_context "$f"
  done
}

scan_explicit() {
  [ -n "$EXPLICIT_LIST" ] || return 0
  while IFS= read -r f; do
    [ -n "$f" ] || continue
    scan_file_with_context "$f"
  done <"$EXPLICIT_LIST"
}

case "$MODE" in
  staged)   scan_staged ;;
  all)      scan_all ;;
  explicit) scan_explicit ;;
esac

n=$(wc -l <"$FINDINGS_FILE" 2>/dev/null | tr -d ' ' || echo 0)
: "${n:=0}"

if [ "$STRICT" = "1" ] && [ "$n" -gt 0 ]; then
  printf '[backlog-tracking] strict mode: %s finding(s); failing.\n' "$n" >&2
  exit 1
fi

# Warn-only: always exit 0.
exit 0
