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
# Opt-out mechanisms (any one is sufficient):
#
#   1. Line-level: the offending line OR the line immediately preceding it
#      contains one of these markers (case-insensitive):
#        <!-- backlog-tracking-ignore -->
#        # backlog-tracking-ignore
#        // backlog-tracking-ignore
#
#   2. File-level: any line in the first 15 lines of the file contains the
#      file-scope marker (case-insensitive):
#        <!-- backlog-tracking-ignore-file -->
#        # backlog-tracking-ignore-file
#        // backlog-tracking-ignore-file
#      Use this for meta-documentation that intentionally contains the
#      patterns this hook looks for (e.g. AGENTS.md describing Backlog.md).
#
#   3. Code-fence aware: in Markdown files (*.md), lines inside fenced
#      code blocks (``` ... ``` or ~~~ ... ~~~) are skipped. Documentation
#      examples that show TODO comments or checklists are not actionable.
#
#   4. Comment-context required: TODO/FIXME/HACK/XXX markers are only
#      flagged when they appear in a comment context (line starts with
#      //, #, *, <!--, /*, or `- ` and the marker is followed by `:`,
#      `(`, or `!`). This excludes Rust identifiers like `todo()`,
#      `TodoStore`, `set_todos`, and file paths like `todo.rs`.
#
#   5. Inline tracked-TODO form: the line contains `TODO(TASK-NN):`,
#      `FIXME(TASK-NN):`, `HACK(TASK-NN):`, or `XXX(TASK-NN):`. This is
#      the idiomatic Rust/C++ convention for a TODO that already has a
#      tracked backlog ticket. Example:
#        // TODO(TASK-47): implement with windows-sys crate
#      Alternatively use a separate `Tracked in: TASK-NN` line above or
#      on the same line as the marker.
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
# Comment-context-bounded TODO/FIXME/HACK/XXX marker. Requires the marker to
# appear inside a comment-like prefix (//, #, *, /*, <!--, leading `-` in
# Markdown, or as the literal first word of the line), AND to be followed by
# `:`, `(`, or `!` so we skip identifier uses like `todo_view`, `todo()`,
# `TodoStore`, file paths like `todo.rs`, and prose like "list of todos".
# Each branch is anchored at start-of-line (with optional indent) so we are
# certain we are in a real comment context, not arbitrary string content.
# backlog-tracking-ignore
PAT_TODO='(?i)^\s*(?://+|#+|<!--|/\*|\*\s|-\s)\s*\(?\s*\b(TODO|FIXME|HACK|XXX)\b\s*[:!(]'
PAT_TRACKED_EMPTY='^\s*Tracked in:\s*$'
# Two-tier ignore semantics:
#
# PAT_IGNORE_LINE: applies only to the offending line itself. Includes the
#   inline tracked-TODO form `TODO(TASK-NN):`, which is a per-line opt-out
#   and must NOT bleed into surrounding lines (otherwise a tracked TODO on
#   line N would silently suppress an untracked TODO on line N+1).
#
# PAT_IGNORE_BLOCK: applies to the offending line OR the line immediately
#   preceding it. Limited to explicit block-style markers that the author
#   typed deliberately to suppress a following finding.
#
#   1. Explicit ignore marker: `backlog-tracking-ignore` (line-level) or
#      `backlog-tracking-ignore-file` (whole-file, matched separately).
#   2. Positive tracking pointer: `Tracked in: TASK-NN`.
PAT_IGNORE_BLOCK='(?i)(backlog-tracking-ignore(?!-file)|Tracked in:\s*TASK-\d+)'
#   3. Inline tracked-TODO form: `TODO(TASK-NN):` etc. (per-line only).
PAT_IGNORE_LINE='(?i)(backlog-tracking-ignore(?!-file)|Tracked in:\s*TASK-\d+|\b(TODO|FIXME|HACK|XXX)\(TASK-\d+\))'
PAT_IGNORE_FILE='(?i)backlog-tracking-ignore-file'

emit() {
  # emit <file> <line> <reason> <content>
  printf '[backlog-tracking] warn %s:%s: %s: %s\n' "$1" "$2" "$3" "$4" >&2
  printf "  -> File a Backlog.md task (https://backlog.md) and reference it as 'Tracked in: TASK-NN' instead, or guard with <!-- backlog-tracking-ignore --> if intentional.\n" >&2
  printf 'x\n' >>"$FINDINGS_FILE"
}

has_ignore_line() {
  printf '%s' "$1" | rg -q -P "$PAT_IGNORE_LINE"
}

has_ignore_block() {
  printf '%s' "$1" | rg -q -P "$PAT_IGNORE_BLOCK"
}

# file_has_ignore <file>
# True iff one of the first 15 lines of the file contains the file-scope
# opt-out marker. Cheap: we read at most 15 lines via `head`.
file_has_ignore() {
  [ -f "$1" ] || return 1
  head -n 15 "$1" 2>/dev/null | rg -q -P "$PAT_IGNORE_FILE"
}

# is_markdown <file>
is_markdown() {
  case "$1" in
    *.md|*.markdown) return 0 ;;
    *) return 1 ;;
  esac
}

# update_fence_state <prev_state:0|1> <line>
# Toggles fence state when a line opens or closes a Markdown code fence.
# Recognized fences: ``` or ~~~ (3+ backticks/tildes), as the first non-
# whitespace content of the line. Returns the new state on stdout.
update_fence_state() {
  state=$1; line=$2
  if printf '%s' "$line" | rg -q -P '^\s*(```+|~~~+)'; then
    if [ "$state" = "1" ]; then printf '0'; else printf '1'; fi
  else
    printf '%s' "$state"
  fi
}

# inspect_line <file> <lineno> <content> <prev_line>
inspect_line() {
  file=$1; lineno=$2; content=$3; prev=$4
  if has_ignore_line "$content"; then return 0; fi
  if [ -n "$prev" ] && has_ignore_block "$prev"; then return 0; fi
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

# compute_fence_lines <markdown_file>
# Emits a newline-separated list of 1-indexed line numbers that fall inside
# Markdown fenced code blocks (``` or ~~~). Toggling, opening fence and
# closing fence inclusive (we want to suppress findings on the fence line
# itself too, e.g. when a code block starts with `\`\`\`text` containing a
# TODO marker on the immediately following line).
compute_fence_lines() {
  f=$1
  [ -f "$f" ] || return 0
  awk '
    /^[[:space:]]*(```+|~~~+)/ {
      inside = !inside
      print NR
      next
    }
    { if (inside) print NR }
  ' "$f" 2>/dev/null
}

scan_staged() {
  cur_file=""
  cur_lineno=0
  prev_line=""
  cur_fence_lines=""
  # -U0: no surrounding context so every `+` is a real addition.
  git diff --cached -U0 --no-color --diff-filter=AM | while IFS= read -r raw; do
    case "$raw" in
      '+++ b/'*)
        cur_file=${raw#+++ b/}
        cur_lineno=0
        prev_line=""
        cur_fence_lines=""
        if path_excluded "$cur_file"; then
          cur_file=""
        elif file_has_ignore "$cur_file"; then
          # Whole file is opted out; ignore all hunks in it.
          cur_file=""
        elif is_markdown "$cur_file"; then
          # Precompute fenced-code line numbers once per file. Joining with
          # a space and matching with a sentinel keeps the membership test
          # POSIX-shell-portable and fast for small N.
          cur_fence_lines=" $(compute_fence_lines "$cur_file" | tr '\n' ' ')"
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
          if [ -n "$cur_fence_lines" ] && \
             printf '%s' "$cur_fence_lines" | rg -qF " $cur_lineno "; then
            : # fenced code block in Markdown; skip
          else
            inspect_line "$cur_file" "$cur_lineno" "$content" "$prev_line"
          fi
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
# the context-sensitive ignore marker. For Markdown files, skip lines that
# fall inside fenced code blocks.
scan_file_with_context() {
  f=$1
  [ -f "$f" ] || return 0
  if file_has_ignore "$f"; then
    return 0
  fi
  fence_lines=""
  if is_markdown "$f"; then
    fence_lines=" $(compute_fence_lines "$f" | tr '\n' ' ')"
  fi
  rg -n --no-heading -P \
     -e "$PAT_CHECKBOX" \
     -e "$PAT_TODO" \
     -e "$PAT_TRACKED_EMPTY" \
     -- "$f" 2>/dev/null | while IFS=: read -r lineno content; do
    [ -n "$lineno" ] || continue
    case "$lineno" in
      ''|*[!0-9]*) continue ;;
    esac
    if [ -n "$fence_lines" ] && \
       printf '%s' "$fence_lines" | rg -qF " $lineno "; then
      continue
    fi
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
