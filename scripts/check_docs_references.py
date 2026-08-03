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

Scope: active documentation only. Historical archives, frozen evidence, and
accepted reviews are excluded by path, because rewriting them to satisfy a
current-policy rule would destroy the record they exist to preserve.

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

# Frozen or historical material. Current-policy rules do not apply: these
# records are supposed to still say what was true when they were accepted.
EXCLUDED_PREFIXES = (
    "docs/archive/",
    # Self-declared frozen forensic records. `docs/fork/recovery/README.md`
    # says "frozen in place for forensic integrity" and
    # `docs/fork/normalization/BASELINE.md` says "historical append-only
    # snapshot"; both last changed 2026-07-18. Their absolute paths record
    # where work actually happened and are evidence, not instructions.
    "docs/fork/recovery/",
    "docs/fork/normalization/",
    "docs/fork/ideal-base/evidence/",
    "docs/fork/ideal-base/reviews/",
    "docs/fork/ideal-base/investigations/",
    "docs/fork/ideal-base/human-noticed-issues/",
    "node_modules/",
    "target/",
    "vendor/",
    "web/jcode-mobile/node_modules/",
)

# The audit register itself documents the machine-local references as findings,
# and tabulates the counting variants. It must be able to name them.
MACHINE_LOCAL_EXEMPT = (
    "docs/fork/ideal-base/D01_DOCUMENTATION_AUDIT.md",
    "docs/fork/ideal-base/POST_DISTRIBUTION_ORCHESTRATOR_PLAN.md",
)

LINK = re.compile(r"(?<!!)\[[^]]*]\(([^)]+)\)")
MACHINE_LOCAL = re.compile(r"~/(?:notes|Documents|Desktop)/|/Users/[A-Za-z0-9._-]+/")
BASELINE_FILE = Path(__file__).resolve().parent / "docs_references_budget.json"

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
    out = []
    for path in sorted(root.rglob("*.md")):
        rel = path.relative_to(root).as_posix()
        if in_scope(rel):
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
    if rel in MACHINE_LOCAL_EXEMPT:
        return []
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
    for path in markdown_files(root):
        try:
            text = path.read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError) as exc:
            findings.append(Finding("unreadable", path.relative_to(root).as_posix(), str(exc)))
            continue
        findings.extend(check_links(root, path, text))
        findings.extend(check_machine_local(root, path, text))
        findings.extend(check_retired_rails(root, path, text))
    return findings


def machine_local_counts(findings: list[Finding]) -> dict[str, int]:
    """Per-file counts of the ratcheted rule, keyed by path without line number."""
    counts: dict[str, int] = {}
    for finding in findings:
        if finding.rule != "machine-local":
            continue
        rel = finding.location.rsplit(":", 1)[0]
        counts[rel] = counts.get(rel, 0) + 1
    return counts


def load_baseline() -> dict[str, int]:
    if not BASELINE_FILE.exists():
        return {}
    data = json.loads(BASELINE_FILE.read_text(encoding="utf-8"))
    counts = data.get("machine_local_by_file")
    if not isinstance(counts, dict):
        raise SystemExit(f"error: invalid baseline file format: {BASELINE_FILE}")
    return {str(k): int(v) for k, v in counts.items()}


def write_baseline(counts: dict[str, int], previous: dict[str, int]) -> None:
    # A baseline that can rise is not a ratchet, it is a laundering mechanism.
    raised = {
        f: (previous.get(f, 0), counts[f]) for f in counts if counts[f] > previous.get(f, 0)
    }
    if raised and previous:
        detail = ", ".join(f"{f}: {was} -> {now}" for f, (was, now) in sorted(raised.items()))
        raise SystemExit(
            f"error: --update refuses to raise the baseline ({detail}). "
            "Fix the new references instead."
        )
    payload = {
        "_comment": (
            "Per-file count of machine-local documentation references (D01-F08). "
            "This ratchet may only decrease. Refresh with "
            "scripts/check_docs_references.py --update after removing or importing "
            "a reference; --update refuses to raise any file's count."
        ),
        "machine_local_by_file": dict(sorted(counts.items())),
        "total": sum(counts.values()),
    }
    BASELINE_FILE.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")


def ratchet_violations(counts: dict[str, int], baseline: dict[str, int]) -> list[str]:
    problems = []
    for path in sorted(set(counts) | set(baseline)):
        now, allowed = counts.get(path, 0), baseline.get(path, 0)
        if now > allowed:
            problems.append(
                f"machine-local: {path}: {now} reference(s), baseline allows {allowed}. "
                "Import the content or drop the reference; do not raise the baseline."
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

    counts = machine_local_counts(findings)

    if args.update:
        write_baseline(counts, load_baseline())
        print(f"docs-references: baseline updated ({sum(counts.values())} machine-local)")
        return 0

    # Every rule except the ratcheted one is fatal on first occurrence.
    hard = [f for f in findings if f.rule != "machine-local"]
    problems = [str(f) for f in hard] + ratchet_violations(counts, load_baseline())

    if not problems:
        print(
            f"docs-references: OK ({len(markdown_files(root))} active documents, "
            f"{sum(counts.values())} machine-local at baseline)"
        )
        return 0

    for problem in problems:
        print(problem, file=sys.stderr)
    print(f"\ndocs-references: {len(problems)} finding(s)", file=sys.stderr)
    return 1


if __name__ == "__main__":
    sys.exit(main())
