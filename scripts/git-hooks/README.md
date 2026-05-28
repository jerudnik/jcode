# Git hooks

Warn-only Git hooks that help keep work surfaced as Backlog.md tasks instead
<!-- backlog-tracking-ignore -->
of ad-hoc TODO/FIXME markers or stray unchecked checklists.

## Install

```sh
bash scripts/install_git_hooks.sh
```

The installer symlinks each hook in this directory into `.git/hooks/` (or
copies it if symlinks are unsupported). It backs up any existing hook to
`*.backup-YYYYMMDD-HHMMSS` before overwriting. It is idempotent.

## What gets installed

- `pre-commit` -> runs `check-backlog-tracking.sh` against staged changes.
  Warn-only: prints warnings to stderr and always exits 0.

## Tracking-divergence check

`check-backlog-tracking.sh` flags newly-added:

1. Unchecked checklist items: `- [ ] ...`
<!-- backlog-tracking-ignore -->
2. Actionable markers: `TODO`, `FIXME`, `HACK`, `XXX` (case-insensitive).
3. Empty `Tracked in:` pointers.

Suggested remediation: file a [Backlog.md](https://backlog.md) task and
replace the marker with a pointer like `Tracked in: TASK-42`.

### Opt-out marker

If a finding is intentional, add one of these comments on the same line or
the immediately preceding line (case-insensitive match):

```
<!-- backlog-tracking-ignore -->
# backlog-tracking-ignore
// backlog-tracking-ignore
```

### Modes

```sh
# Staged-only (default, used by the pre-commit hook).
scripts/git-hooks/check-backlog-tracking.sh

# Scan the entire tracked tree (useful in CI).
scripts/git-hooks/check-backlog-tracking.sh --all

# Scan explicit paths (useful for smoke tests).
scripts/git-hooks/check-backlog-tracking.sh path/to/file.md

# Fail the run on any finding (warn-only by default).
scripts/git-hooks/check-backlog-tracking.sh --all --strict
EXIT_ON_FINDING=1 scripts/git-hooks/check-backlog-tracking.sh --all
```

### Making it blocking

The hook is intentionally warn-only. To make it block commits globally,
either export `EXIT_ON_FINDING=1` in your shell, or invoke the script with
`--strict` from your own hook variant.

### File-type coverage

`*.md`, `*.txt`, `*.rs`, `*.toml`, `*.nix`, `*.sh`, `*.py`, `*.js`, `*.ts`,
`*.tsx`. Other files are skipped. The `backlog/` and `.backlog/` trees are
excluded since they are the canonical task store and naturally contain
unchecked acceptance criteria; explicit-path invocations bypass this
exclusion.

### Dependencies

POSIX `sh`, `git`, and [ripgrep](https://github.com/BurntSushi/ripgrep)
(`rg`). All three are already required elsewhere in the repo workflow.

### CI enforcement

The `quality` job in `.github/workflows/ci.yml` runs two related gates
against every push and PR targeting `main`/`master`:

1. `scripts/git-hooks/check-backlog-tracking.sh --all --strict` fails
   on any new TODO/FIXME/HACK/XXX, unchecked checklist, or empty
   `Tracked in:` pointer that lacks one of the documented opt-out
   mechanisms.

2. `python3 scripts/backlog_pointer_verify.py check` is a bidirectional
   verifier that fails on dead pointers (typo'd or missing `TASK-NN`
   references). Topic-overlap warnings remain warnings so legitimate
   cross-references are not over-policed; see
   [`scripts/backlog_pointer_verify.py`](../backlog_pointer_verify.py).

The local pre-commit hook stays warn-only so developers get fast
feedback without blocking commits mid-work. The CI gates ensure the
baseline (0 findings as of commit `0678cd8d`) cannot regress at merge
time.
