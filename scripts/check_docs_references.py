#!/usr/bin/env python3
"""Verify that active documentation points at things that exist.

Documentation drifts silently. Nothing in the repository re-reads a document
after it is written, so a path that was correct when the prose was authored
stays in the tree long after the file moved. `D01-F10` and `D01-F11` are both
that defect: `OAUTH.md` and `docs/MEMORY_BUDGET.md` kept pre-crate-split paths,
and `TELEMETRY.md` pointed at `src/telemetry.rs` for a crate that now lives at
`crates/jcode-telemetry-core/`. Each was corrected by hand, and correcting by
hand only resets the clock. This checker is the part that does not.

Rules (each independently fatal):

  disallowed-path    tracked Markdown exists under a retired repository docs
                     surface: docs/archive/, docs/fork/, or docs/proposals/
  issue-frontmatter  docs/issues/*.md is missing its required YAML fields, or
                     retains a solved status instead of being deleted
  broken-link       a Markdown link to a repository-relative path that does
                    not exist on disk
  machine-local     a reference to `~/notes/...` or a `/Users/...` home path,
                    which no consumer of this repository can resolve. Matched
                    in ANY form, not only link syntax: `D01-F08`'s own recount
                    conflated "10 links" with "14 mentions" and read the
                    difference as growth, so counting only links would miss
                    four real cases.

                    This rule is a RATCHET, not a wall. 25 such references
                    exist today and `D01-F08` is still open, so failing the
                    build on all of them would make CI red on a defect that
                    has not been dispositioned yet. The baseline in
                    `scripts/docs_references_budget.json` pins the current
                    count per file: a new one fails, and fixing one must be
                    followed by `--update`, which can only ratchet DOWN.
                    `--update` refuses to raise any file's count, so the
                    baseline cannot be used to launder a regression.

  retired-rail      an active document telling a reader to use a distribution
                    rail this fork retired (Homebrew, AUR, curl|sh installers,
                    Cargo registry install, TestFlight/App Store)

The retired-rail rule matches an INSTRUCTION, not a mention. `README.md` and
`RELEASING.md` must be able to say "we do not ship via Homebrew or AUR" without
tripping a checker whose whole purpose is enforcing that sentence, so a line is
only flagged when it reads as a directive and does not read as a prohibition.
Getting this backwards would make the rule fire on the contract it enforces.

Scope: tracked Markdown. Generated instruction files are ignored because they
are not committed; dependency and build trees remain excluded.

Usage:
  scripts/check_docs_references.py
  scripts/check_docs_references.py --root /path/to/tree
  scripts/check_docs_references.py --list        # print findings, exit 0
  scripts/check_docs_references.py --update      # ratchet the baseline down
"""

from __future__ import annotations

import argparse
import json
import functools
import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path

# Non-repository content trees. Every tracked Markdown file under docs/ is now
# current documentation or an open issue, so there are no historical-doc
# exclusions left to hide findings.
EXCLUDED_PREFIXES = (
    "node_modules/",
    "target/",
    "vendor/",
    "web/jcode-mobile/node_modules/",
)

DISALLOWED_DOC_PREFIXES = ("docs/archive/", "docs/fork/", "docs/proposals/")
ISSUE_DIR = "docs/issues/"
REQUIRED_ISSUE_FIELDS = ("status", "priority", "owner", "opened")
SOLVED_ISSUE_STATUSES = {"closed", "fixed", "wontfix"}

LINK = re.compile(r"(?<!!)\[[^]]*]\(([^)]+)\)")
# D01-F12. A backticked path into the source tree that no longer exists. The
# modularization moved whole subsystems into crates/, so `src/platform.rs` now
# lives at crates/jcode-base/src/platform.rs and every citation of the old path
# silently sends the reader nowhere.
CODE_PATH = re.compile(
    r"`((?:crates|src|scripts|tests)/[A-Za-z0-9_./-]+\.(?:rs|py|sh|nix))(?::\d+)?`"
)

# There is no whole-file exemption, deliberately. Frozen records (dated audits,
# the retired upstream-tracking model) do legitimately cite paths that no longer
# exist, but exempting the FILE to protect those citations also blinds the rule
# to every citation added to it later, including the ones that resolve today and
# will rot the next time code moves. The per-file ratchet already says "this
# debt is inherited and may not grow" without giving up the file: a frozen
# record sits at its measured count forever, while a new stale citation in it
# still fails. Measured when the exemption was removed: those 6 files carried
# 359 backticked code citations, 42 resolving to live code and 317 stale; the
# exemption hid all 359, and only the 317 stale ones became the seeded baseline.
MACHINE_LOCAL = re.compile(r"~/(?:notes|Documents|Desktop)/|/Users/[A-Za-z0-9._-]+/")
BASELINE_FILE = Path(__file__).resolve().parent / "docs_references_budget.json"
BASELINE_KEYS = {
    "machine-local": "machine_local_by_file",
    "stale-code-path": "stale_code_paths_by_file",
}

# A retired rail is only a finding when the prose tells someone to use it.
RETIRED_RAILS = (
    ("homebrew", re.compile(r"\bbrew\s+(install|tap)\b")),
    ("aur", re.compile(r"\b(yay|paru|makepkg)\s+-\w*S?\b")),
    ("curl-installer", re.compile(r"curl\s[^\n|]*\|\s*(ba|z)?sh\b")),
    ("cargo-install", re.compile(r"\bcargo\s+install\s+jcode\b")),
    ("testflight", re.compile(r"\b(install|download|join|via)\b[^.\n]*\bTestFlight\b", re.I)),
)

# Words that mark a line as saying "we do NOT do this". Checked before the rail
# patterns, so the policy statement that retires a rail is never itself flagged.
PROHIBITION = re.compile(
    r"\b(not|never|no|retired|removed|do not|don't|without|instead of|"
    r"rather than|unsupported|deprecated|must not|prohibit\w*|forbid\w*)\b",
    re.I,
)


@dataclass(frozen=True)
class Finding:
    rule: str
    location: str
    detail: str

    def __str__(self) -> str:
        return f"{self.rule}: {self.location}: {self.detail}"


def in_scope(rel: str) -> bool:
    return not any(rel.startswith(p) for p in EXCLUDED_PREFIXES)


def markdown_files(root: Path) -> list[Path]:
    """Active documents, as the repository defines them.

    Ask git, not the filesystem. `apm compile` generates AGENTS.md, CLAUDE.md
    and GEMINI.md into the worktree and .gitignore excludes them, so an rglob
    scans 9 files here that do not exist in a clean clone: the local run
    reported 146 documents where CI reported 137. Same tree, two numbers, and
    only one of them is the repository. A gate that governs the checkout rather
    than the commit gives different verdicts to different people.

    Falls back to the filesystem when git is unavailable, since a weaker scan
    beats a gate that cannot run at all.
    """
    # `_tracked_files` signals "no git here" with an empty set, which is what
    # the other two consumers already branch on. Testing `is None` instead
    # meant a tree without a repo scanned zero documents and the gate passed
    # everything -- the same "guard silently answers fine" failure this file
    # exists to prevent, so it is spelled the same way in all three places.
    tracked = _tracked_files(str(root))
    if not tracked:
        candidates = sorted(root.rglob("*.md"))
    else:
        candidates = sorted(root / rel for rel in tracked if rel.endswith(".md"))
    out = []
    for path in candidates:
        rel = path.relative_to(root).as_posix()
        if in_scope(rel) and path.is_file():
            out.append(path)
    return out


def check_links(root: Path, path: Path, text: str) -> list[Finding]:
    rel = path.relative_to(root).as_posix()
    findings = []
    for raw in LINK.findall(text):
        target = raw.split(maxsplit=1)[0].strip("<>")
        if not target or target.startswith(("http://", "https://", "mailto:", "#")):
            continue
        if target.startswith("~/") or target.startswith("/Users/"):
            continue  # reported by the machine-local rule with better context
        path_text = target.split("#", 1)[0]
        if not path_text:
            continue
        resolved = (path.parent / path_text).resolve()
        if not resolved.exists():
            findings.append(Finding("broken-link", rel, f"link target does not exist: {target}"))
            continue
        # A file that exists only in this working copy is not a link a reader
        # can follow. CI caught this on a clean checkout: docs/README.md links
        # to docs/AGENTS.md, which is generated by `apm compile` and gitignored,
        # so it resolved on the author's machine and 404s for everyone else.
        # Judging the committed tree is the whole point of the rule, so an
        # untracked target is broken even when os.path says otherwise.
        if _is_untracked(root, resolved):
            findings.append(
                Finding(
                    "broken-link",
                    rel,
                    f"link target is not committed, so it resolves only on machines that "
                    f"have generated it: {target}",
                )
            )
    return findings


@functools.lru_cache(maxsize=1)
def _tracked_files(root: str) -> frozenset[str]:
    try:
        out = subprocess.run(
            ["git", "-C", root, "ls-files", "-z"],
            capture_output=True,
            text=True,
            check=True,
        ).stdout
    except (OSError, subprocess.CalledProcessError):
        # No git available: fall back to filesystem truth rather than failing
        # every link. A weaker check beats a checker that cannot run at all.
        return frozenset()
    return frozenset(p for p in out.split("\0") if p)


def _is_untracked(root: Path, resolved: Path) -> bool:
    tracked = _tracked_files(str(root))
    if not tracked:
        return False
    if resolved.is_dir():
        return False
    # Resolve the root too. On macOS a tempdir is /var/... while resolve()
    # returns /private/var/..., so comparing a resolved path against an
    # unresolved root raises ValueError and the rule silently answers "fine".
    # That is the worst possible default for a guard, and it is not
    # test-only: any repo reached through a symlink hits it.
    try:
        rel = resolved.relative_to(root.resolve()).as_posix()
    except ValueError:
        return False  # genuinely outside the repo; existence check already ran
    return rel not in tracked


def check_machine_local(root: Path, path: Path, text: str) -> list[Finding]:
    rel = path.relative_to(root).as_posix()
    findings = []
    for lineno, line in enumerate(text.splitlines(), 1):
        match = MACHINE_LOCAL.search(line)
        if match:
            findings.append(
                Finding(
                    "machine-local",
                    f"{rel}:{lineno}",
                    f"reference no consumer can resolve: {match.group(0)}...",
                )
            )
    return findings


def check_disallowed_path(root: Path, path: Path) -> list[Finding]:
    rel = path.relative_to(root).as_posix()
    if not any(rel.startswith(prefix) for prefix in DISALLOWED_DOC_PREFIXES):
        return []
    return [
        Finding(
            "disallowed-path",
            rel,
            "tracked Markdown must be current docs or an open issue; move or delete this file",
        )
    ]


def issue_frontmatter(text: str) -> dict[str, str] | None:
    if not text.startswith("---\n"):
        return None
    end = text.find("\n---\n", 4)
    if end < 0:
        return None
    fields: dict[str, str] = {}
    for line in text[4:end].splitlines():
        if ":" not in line or line[:1].isspace():
            continue
        key, value = line.split(":", 1)
        fields[key.strip().lower()] = value.strip().strip("\"'")
    return fields


def check_issue_frontmatter(root: Path, path: Path, text: str) -> list[Finding]:
    rel = path.relative_to(root).as_posix()
    if not rel.startswith(ISSUE_DIR) or "/" in rel[len(ISSUE_DIR) :]:
        return []
    fields = issue_frontmatter(text)
    if fields is None:
        return [Finding("issue-frontmatter", rel, "missing YAML frontmatter")]
    missing = [field for field in REQUIRED_ISSUE_FIELDS if not fields.get(field)]
    if missing:
        return [
            Finding(
                "issue-frontmatter",
                rel,
                f"missing required field(s): {', '.join(missing)}",
            )
        ]
    if fields["status"].lower() in SOLVED_ISSUE_STATUSES:
        return [
            Finding(
                "issue-frontmatter",
                rel,
                "delete solved issues instead of archiving them.",
            )
        ]
    return []


def check_code_paths(root: Path, path: Path, text: str, tracked: frozenset[str]) -> list[Finding]:
    """Citations of source files that are not in the tree (D01-F12)."""
    rel = path.relative_to(root).as_posix()
    if not tracked:
        return []
    findings = []
    for lineno, line in enumerate(text.splitlines(), 1):
        for match in CODE_PATH.finditer(line):
            cited = match.group(1)
            if cited not in tracked:
                findings.append(
                    Finding(
                        "stale-code-path",
                        f"{rel}:{lineno}",
                        f"cites a source path that does not exist: {cited}",
                    )
                )
    return findings


def check_retired_rails(root: Path, path: Path, text: str) -> list[Finding]:
    rel = path.relative_to(root).as_posix()
    findings = []
    for lineno, line in enumerate(text.splitlines(), 1):
        if PROHIBITION.search(line):
            continue
        for name, pattern in RETIRED_RAILS:
            if pattern.search(line):
                findings.append(
                    Finding("retired-rail", f"{rel}:{lineno}", f"instructs use of retired rail: {name}")
                )
    return findings


def run(root: Path) -> list[Finding]:
    findings: list[Finding] = []
    tracked = _tracked_files(str(root))
    for path in markdown_files(root):
        try:
            text = path.read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError) as exc:
            findings.append(Finding("unreadable", path.relative_to(root).as_posix(), str(exc)))
            continue
        findings.extend(check_disallowed_path(root, path))
        findings.extend(check_issue_frontmatter(root, path, text))
        findings.extend(check_links(root, path, text))
        findings.extend(check_machine_local(root, path, text))
        findings.extend(check_code_paths(root, path, text, tracked))
        findings.extend(check_retired_rails(root, path, text))
    return findings


# Rules measured as a per-file ratchet rather than failed on first sight.
# Both are debts inherited at a nonzero count, so a fatal rule would just be
# permanently red. Each may only fall.
RATCHETED = ("machine-local", "stale-code-path")


def rule_counts(findings: list[Finding], rule: str) -> dict[str, int]:
    """Per-file counts of one ratcheted rule, keyed by path without line number."""
    counts: dict[str, int] = {}
    for finding in findings:
        if finding.rule != rule:
            continue
        rel = finding.location.rsplit(":", 1)[0]
        counts[rel] = counts.get(rel, 0) + 1
    return counts


def machine_local_counts(findings: list[Finding]) -> dict[str, int]:
    return rule_counts(findings, "machine-local")


def load_baseline(rule: str = "machine-local") -> dict[str, int]:
    if not BASELINE_FILE.exists():
        return {}
    data = json.loads(BASELINE_FILE.read_text(encoding="utf-8"))
    counts = data.get(BASELINE_KEYS[rule])
    if counts is None and rule == "stale-code-path":
        return {}  # key predates this rule; treated as unmeasured, not as zero
    if not isinstance(counts, dict):
        raise SystemExit(f"error: invalid baseline file format: {BASELINE_FILE}")
    return {str(k): int(v) for k, v in counts.items()}


def rule_measured(rule: str) -> bool:
    """True when the baseline file has recorded this rule at all.

    A rule driven to zero has an EMPTY per-file dict, which is indistinguishable
    from "never measured" if you only look at the dict. The companion
    ``<key>_total`` is written on every refresh, so its presence is what marks a
    rule as measured. Without this, reaching zero would silently disarm the
    ratchet: the next --update would accept any regression as a first
    measurement.
    """
    if not BASELINE_FILE.exists():
        return False
    data = json.loads(BASELINE_FILE.read_text(encoding="utf-8"))
    return f"{BASELINE_KEYS[rule]}_total" in data


def write_baseline(counts: dict[str, int], previous: dict[str, int]) -> None:
    write_baselines({"machine-local": counts}, {"machine-local": previous})


def write_baselines(
    counts: dict[str, dict[str, int]], previous: dict[str, dict[str, int]]
) -> None:
    # A baseline that can rise is not a ratchet, it is a laundering mechanism.
    raised = {}
    for rule, per_file in counts.items():
        prev = previous.get(rule, {})
        if not prev and not rule_measured(rule):
            continue  # first measurement of a rule establishes its ceiling
        for f, now in per_file.items():
            if now > prev.get(f, 0):
                raised[f"{rule} {f}"] = (prev.get(f, 0), now)
    if raised:
        detail = ", ".join(f"{f}: {was} -> {now}" for f, (was, now) in sorted(raised.items()))
        raise SystemExit(
            f"error: --update refuses to raise the baseline ({detail}). "
            "Fix the new references instead."
        )
    payload = {
        "_comment": (
            "Per-file ratchets over documentation references. "
            "machine_local_by_file is D01-F08; stale_code_paths_by_file is "
            "D01-F12, citations of source files that no longer exist. Both may "
            "only decrease. Refresh with scripts/check_docs_references.py "
            "--update; --update refuses to raise any file's count."
        ),
    }
    for rule, key in BASELINE_KEYS.items():
        per_file = counts.get(rule, {})
        payload[key] = dict(sorted(per_file.items()))
        payload[f"{key}_total"] = sum(per_file.values())
    BASELINE_FILE.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")


REMEDY = {
    "machine-local": "Import the content or drop the reference",
    "stale-code-path": "Update the citation to where the code moved, or drop it",
}


def ratchet_violations(
    counts: dict[str, int], baseline: dict[str, int], rule: str = "machine-local"
) -> list[str]:
    problems = []
    for path in sorted(set(counts) | set(baseline)):
        now, allowed = counts.get(path, 0), baseline.get(path, 0)
        if now > allowed:
            problems.append(
                f"{rule}: {path}: {now} reference(s), baseline allows {allowed}. "
                f"{REMEDY[rule]}; do not raise the baseline."
            )
    return problems


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", default=None, help="tree to check (default: repository root)")
    parser.add_argument("--list", action="store_true", help="print findings and exit 0")
    parser.add_argument("--update", action="store_true", help="ratchet the baseline down")
    args = parser.parse_args()

    root = Path(args.root).resolve() if args.root else Path(__file__).resolve().parent.parent
    findings = run(root)

    if args.list:
        for finding in findings:
            print(finding)
        return 0

    counts = {rule: rule_counts(findings, rule) for rule in RATCHETED}
    baselines = {rule: load_baseline(rule) for rule in RATCHETED}

    if args.update:
        write_baselines(counts, baselines)
        summary = ", ".join(f"{sum(counts[r].values())} {r}" for r in RATCHETED)
        print(f"docs-references: baseline updated ({summary})")
        return 0

    # Every rule except the ratcheted ones is fatal on first occurrence.
    hard = [f for f in findings if f.rule not in RATCHETED]
    problems = [str(f) for f in hard]
    for rule in RATCHETED:
        problems += ratchet_violations(counts[rule], baselines[rule], rule)

    if not problems:
        summary = ", ".join(f"{sum(counts[r].values())} {r}" for r in RATCHETED)
        print(
            f"docs-references: OK ({len(markdown_files(root))} active documents, "
            f"{summary} at baseline)"
        )
        return 0

    for problem in problems:
        print(problem, file=sys.stderr)
    print(f"\ndocs-references: {len(problems)} finding(s)", file=sys.stderr)
    return 1


if __name__ == "__main__":
    sys.exit(main())
