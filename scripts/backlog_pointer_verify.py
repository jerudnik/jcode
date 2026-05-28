#!/usr/bin/env python3
"""Deterministic, bidirectional verifier for Backlog.md task pointers in repo docs.

Subcommands:
  index           Parse .backlog/tasks/*.md and emit the task index (JSON or table).
  audit           Verify every `Tracked in: TASK-NN[, ...]` pointer in tracked files
                  against the backlog index. Reports existence, bidirectional
                  evidence (task references the pointing doc), and a topical
                  Jaccard-overlap heuristic between the task title and the
                  pointer paragraph.
  reverse         For every backlog task, list pointing docs + referencing docs;
                  highlight orphans and one-sided links.
  check           CI-friendly: runs audit; exits non-zero only when an existence
                  check fails (warnings remain warnings).
  fix-suggest     For each topic-mismatch / weak-link finding, rank top-3
                  alternative TASK-NN candidates by Jaccard overlap.

No external Python dependencies (stdlib only). Requires `git` on PATH.

Pointer pattern (case-sensitive, anchored at start of significant content):
  Tracked in: TASK-NN[, TASK-MM[, ...]]

Opt-out marker (same as the pre-commit hook): a comment containing
`backlog-tracking-ignore` on the same line or the immediately preceding line
suppresses the topical/weak-link warnings for that pointer line. Existence
failures are still reported (to catch typo'd task IDs).
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
from dataclasses import dataclass, field, asdict
from pathlib import Path
from typing import Iterable

# ----- Constants -----

# Match the `Tracked in: TASK-1, TASK-23` pointer line (capture comma-list).
POINTER_RE = re.compile(r"Tracked in:\s*(TASK-\d+(?:\s*,\s*TASK-\d+)*)")
TASK_ID_RE = re.compile(r"TASK-(\d+)")
FRONTMATTER_RE = re.compile(r"^---\n(.*?)\n---\n", re.DOTALL)
IGNORE_MARKER = "backlog-tracking-ignore"

# Stopwords for Jaccard topic comparison. Conservative list; the goal is to
# strip out filler words and Backlog tagging noise that don't carry signal.
STOPWORDS = {
    "a", "an", "and", "are", "as", "at", "be", "but", "by", "for", "from",
    "has", "have", "in", "is", "it", "its", "of", "on", "or", "that", "the",
    "this", "to", "was", "were", "will", "with", "into", "across", "via",
    "tracked", "task", "phase", "phases", "open", "items", "future", "work",
    "follow", "follow-ups", "follow-up", "tbd", "todo", "fixme", "hack",
    "implement", "implementation", "add", "improve", "update", "support",
    "use", "etc", "see", "covered", "covers", "scope", "design", "section",
    "based", "consider", "decision", "decisions", "include", "includes",
    "no", "not", "yes", "if", "when", "after", "before", "than", "then",
    "all", "any", "some", "more", "less", "new", "old", "next", "first",
    "last", "also", "we", "you", "they", "i", "our", "your",
}

# File extensions scanned by `audit` / `check` / `fix-suggest` when no explicit
# paths are provided. Matches the pre-commit hook's tracked-file set.
DEFAULT_GLOBS = ("*.md", "*.txt", "*.rs", "*.toml", "*.nix", "*.sh", "*.py")


@dataclass
class Task:
    id: str  # canonical form: "TASK-31"
    num: int
    title: str
    status: str = ""
    priority: str = ""
    labels: list[str] = field(default_factory=list)
    refs: list[str] = field(default_factory=list)
    file: str = ""

    @property
    def keywords(self) -> set[str]:
        """Title + labels, tokenized and stop-filtered for Jaccard scoring."""
        return tokenize(self.title + " " + " ".join(self.labels))


@dataclass
class Pointer:
    file: str
    line: int
    raw_ids: list[str]
    context: str  # surrounding prose paragraph (~3 lines around pointer)
    ignored: bool  # opt-out marker present


# ----- Helpers -----


def repo_root() -> Path:
    out = subprocess.check_output(["git", "rev-parse", "--show-toplevel"]).decode().strip()
    return Path(out)


def relpath_to_root(path: Path, root: Path) -> str:
    """Return path relative to root if possible, else fall back to absolute path.

    We resolve through `os.path.relpath` on the real (resolved) paths so that
    out-of-repo files (useful for ad-hoc testing) and symlinked paths both
    produce a stable, deterministic identifier.
    """
    try:
        return str(path.resolve().relative_to(root.resolve()))
    except ValueError:
        return os.path.relpath(str(path.resolve()), start=str(root.resolve()))


def git_ls_files(root: Path, globs: Iterable[str]) -> list[Path]:
    args = ["git", "-C", str(root), "ls-files", "--"]
    args.extend(globs)
    out = subprocess.check_output(args).decode().splitlines()
    return [root / line for line in out if line.strip()]


def tokenize(text: str) -> set[str]:
    """Lowercase, alnum-word, strip stopwords/short tokens."""
    words = re.findall(r"[a-zA-Z][a-zA-Z0-9_]+", text.lower())
    return {w for w in words if len(w) >= 3 and w not in STOPWORDS}


def jaccard(a: set[str], b: set[str]) -> float:
    if not a or not b:
        return 0.0
    inter = len(a & b)
    union = len(a | b)
    return inter / union if union else 0.0


def parse_frontmatter(text: str) -> dict | None:
    """Tiny YAML-frontmatter parser sufficient for Backlog.md task files.

    Backlog frontmatter is well-structured: scalar keys, simple list keys
    (labels, dependencies, references), and block scalars (`>-`, `|-`,
    `|`, `>`) for long titles. We avoid pulling PyYAML.
    """
    m = FRONTMATTER_RE.match(text)
    if not m:
        return None
    body = m.group(1)
    out: dict = {}
    current_list_key: str | None = None
    block_key: str | None = None
    block_chomp: str = ""  # for completeness; we always join with space
    block_lines: list[str] = []

    def _flush_block() -> None:
        nonlocal block_key, block_lines
        if block_key is not None:
            # Folded (`>` / `>-`) and literal (`|` / `|-`) both reduce here to a
            # single joined string. Backlog stores titles as one logical line,
            # so this is good enough.
            joined = " ".join(s.strip() for s in block_lines if s.strip())
            out[block_key] = joined
            block_key = None
            block_lines = []

    lines = body.splitlines()
    for line in lines:
        # Continuing a block scalar?
        if block_key is not None:
            if line.startswith(" ") and line.strip() != "":
                block_lines.append(line)
                continue
            # Non-indented or empty line ends the block.
            _flush_block()
            # fall through to normal handling for `line`

        if not line.strip():
            current_list_key = None
            continue
        if line.startswith("  - ") and current_list_key is not None:
            val = line[4:].strip()
            if (val.startswith("'") and val.endswith("'")) or (
                val.startswith('"') and val.endswith('"')
            ):
                val = val[1:-1]
            out[current_list_key].append(val)
            continue
        if ":" in line and not line.startswith(" "):
            key, _, rest = line.partition(":")
            key = key.strip()
            rest = rest.strip()
            if not rest:
                # Empty list or about to start a list
                out[key] = []
                current_list_key = key
                continue
            # Block-scalar prefix: `>-`, `>`, `|-`, `|` (optionally followed by chomp/indent)
            if rest[0] in ("|", ">"):
                block_key = key
                block_chomp = rest
                block_lines = []
                current_list_key = None
                continue
            # Quoted scalar
            if (rest.startswith("'") and rest.endswith("'")) or (
                rest.startswith('"') and rest.endswith('"')
            ):
                rest = rest[1:-1]
            # Inline list `[a, b]`
            if rest.startswith("[") and rest.endswith("]"):
                inner = rest[1:-1].strip()
                out[key] = [s.strip().strip("'\"") for s in inner.split(",") if s.strip()]
            else:
                out[key] = rest
            current_list_key = None
    _flush_block()
    return out


def load_tasks(root: Path) -> dict[str, Task]:
    tasks: dict[str, Task] = {}
    backlog_dir = root / ".backlog" / "tasks"
    if not backlog_dir.is_dir():
        return tasks
    for path in sorted(backlog_dir.glob("task-*.md")):
        text = path.read_text(encoding="utf-8", errors="replace")
        fm = parse_frontmatter(text)
        if not fm:
            continue
        raw_id = str(fm.get("id", "")).strip()
        m = TASK_ID_RE.fullmatch(raw_id.upper()) if raw_id else None
        if not m:
            # Backlog also accepts lowercase `task-31`; normalize.
            m2 = re.fullmatch(r"task-(\d+)", raw_id)
            if not m2:
                continue
            num = int(m2.group(1))
        else:
            num = int(m.group(1))
        canonical = f"TASK-{num}"
        title = str(fm.get("title", "")).strip()
        labels = fm.get("labels", []) or []
        refs = fm.get("references", []) or []
        if isinstance(labels, str):
            labels = [labels]
        if isinstance(refs, str):
            refs = [refs]
        tasks[canonical] = Task(
            id=canonical,
            num=num,
            title=title,
            status=str(fm.get("status", "")).strip(),
            priority=str(fm.get("priority", "")).strip(),
            labels=[str(x) for x in labels],
            refs=[str(x) for x in refs],
            file=str(path.relative_to(root)),
        )
    return tasks


def collect_pointers(root: Path, paths: list[Path]) -> list[Pointer]:
    pointers: list[Pointer] = []
    for path in paths:
        try:
            text = path.read_text(encoding="utf-8", errors="replace")
        except OSError:
            continue
        rel = relpath_to_root(path, root)
        lines = text.splitlines()
        for idx, line in enumerate(lines):
            m = POINTER_RE.search(line)
            if not m:
                continue
            ids = TASK_ID_RE.findall(m.group(1))
            canonical_ids = [f"TASK-{n}" for n in ids]
            # Opt-out: same-line or immediately previous line contains the marker.
            ignored = (IGNORE_MARKER in line.lower()) or (
                idx > 0 and IGNORE_MARKER in lines[idx - 1].lower()
            )
            # Context window: walk backwards from the pointer line up to (and
            # including) the most recent markdown heading, capped at 30 lines
            # so we don't pull in unrelated prose. This captures the thematic
            # section the pointer terminates, which is the right signal for
            # ID-vs-topic matching.
            ctx_lines: list[str] = [line]
            j = idx - 1
            steps = 0
            while j >= 0 and steps < 30:
                prev = lines[j]
                ctx_lines.append(prev)
                steps += 1
                if re.match(r"^#{1,6}\s", prev):
                    break
                j -= 1
            ctx_lines.reverse()
            pointers.append(
                Pointer(
                    file=rel,
                    line=idx + 1,
                    raw_ids=canonical_ids,
                    context=" ".join(ctx_lines),
                    ignored=ignored,
                )
            )
    return pointers


def ref_matches_doc(refs: list[str], doc_rel: str) -> bool:
    """Does any task ref point at the doc holding the pointer?

    We strip a trailing `@<sha>` and any `:line` suffix, then match by suffix
    against the rel path. We also allow basename match for resilience.
    """
    doc_base = os.path.basename(doc_rel)
    for raw in refs:
        ref = raw.split("@", 1)[0]
        ref = ref.split(":", 1)[0]
        if not ref or ref.startswith("commit"):
            continue
        if doc_rel == ref or doc_rel.endswith("/" + ref) or ref.endswith("/" + doc_rel):
            return True
        if os.path.basename(ref) == doc_base:
            return True
    return False


# ----- Subcommand: index -----


def cmd_index(args: argparse.Namespace) -> int:
    root = repo_root()
    tasks = load_tasks(root)
    payload = {tid: asdict(t) for tid, t in tasks.items()}
    if args.format == "json":
        print(json.dumps(payload, indent=2, sort_keys=True))
    else:
        print(f"{'ID':<10} {'PRIO':<8} {'STATUS':<10} TITLE")
        for tid in sorted(tasks, key=lambda t: int(t.split("-")[1])):
            t = tasks[tid]
            print(f"{t.id:<10} {t.priority:<8} {t.status:<10} {t.title}")
    return 0


# ----- Subcommand: audit -----


def collect_target_paths(root: Path, explicit: list[str] | None) -> list[Path]:
    if explicit:
        return [Path(p).resolve() for p in explicit if Path(p).exists()]
    return git_ls_files(root, DEFAULT_GLOBS)


def audit_findings(
    tasks: dict[str, Task],
    pointers: list[Pointer],
    threshold: float,
) -> list[dict]:
    findings: list[dict] = []
    for ptr in pointers:
        for tid in ptr.raw_ids:
            t = tasks.get(tid)
            if t is None:
                findings.append(
                    {
                        "severity": "error",
                        "kind": "missing-task",
                        "file": ptr.file,
                        "line": ptr.line,
                        "task": tid,
                        "detail": f"{tid} not found in backlog",
                    }
                )
                continue
            if ptr.ignored:
                # We still ran existence (above). Skip the heuristics.
                continue
            if not ref_matches_doc(t.refs, ptr.file):
                findings.append(
                    {
                        "severity": "warn",
                        "kind": "weak-link",
                        "file": ptr.file,
                        "line": ptr.line,
                        "task": tid,
                        "detail": (
                            f"{tid} ('{t.title}') does not reference {ptr.file} "
                            f"in its task `references:`; pointer is one-sided"
                        ),
                    }
                )
            ctx_tokens = tokenize(ptr.context)
            score = jaccard(t.keywords, ctx_tokens)
            if score < threshold:
                findings.append(
                    {
                        "severity": "warn",
                        "kind": "topic-mismatch",
                        "file": ptr.file,
                        "line": ptr.line,
                        "task": tid,
                        "score": round(score, 3),
                        "threshold": threshold,
                        "detail": (
                            f"low overlap ({score:.2f}) between '{t.title}' and "
                            f"pointer context; verify the ID is correct"
                        ),
                    }
                )
    return findings


def print_findings_table(findings: list[dict]) -> None:
    if not findings:
        print("OK: no findings.")
        return
    for f in findings:
        sev = f["severity"].upper()
        kind = f["kind"]
        loc = f"{f['file']}:{f['line']}"
        extra = ""
        if "score" in f:
            extra = f" score={f['score']:.2f}<{f['threshold']:.2f}"
        print(f"[{sev}] {kind} {loc} {f['task']}{extra}")
        print(f"        {f['detail']}")


def cmd_audit(args: argparse.Namespace, *, strict: bool = False) -> int:
    root = repo_root()
    tasks = load_tasks(root)
    paths = collect_target_paths(root, args.paths)
    pointers = collect_pointers(root, paths)
    findings = audit_findings(tasks, pointers, args.threshold)
    if args.format == "json":
        print(
            json.dumps(
                {
                    "tasks": len(tasks),
                    "pointers": len(pointers),
                    "findings": findings,
                },
                indent=2,
                sort_keys=True,
            )
        )
    else:
        print(f"Scanned {len(pointers)} pointers across {len(paths)} files against {len(tasks)} tasks.")
        print_findings_table(findings)
    # `check` (strict): fail on existence errors only.
    has_error = any(f["severity"] == "error" for f in findings)
    if strict and has_error:
        return 1
    return 0


# ----- Subcommand: reverse -----


def cmd_reverse(args: argparse.Namespace) -> int:
    root = repo_root()
    tasks = load_tasks(root)
    paths = collect_target_paths(root, args.paths)
    pointers = collect_pointers(root, paths)

    # task -> docs that point at it
    pointed_by: dict[str, set[str]] = {tid: set() for tid in tasks}
    for ptr in pointers:
        for tid in ptr.raw_ids:
            if tid in pointed_by:
                pointed_by[tid].add(f"{ptr.file}:{ptr.line}")

    # task -> docs it references (extracted from the task `references:` list)
    referenced_docs: dict[str, set[str]] = {}
    for tid, t in tasks.items():
        docs = set()
        for raw in t.refs:
            ref = raw.split("@", 1)[0].split(":", 1)[0]
            if ref and not ref.startswith("commit") and (ref.endswith(".md") or ref.endswith(".txt")):
                docs.add(ref)
        referenced_docs[tid] = docs

    rows = []
    for tid in sorted(tasks, key=lambda t: int(t.split("-")[1])):
        t = tasks[tid]
        ptrs = sorted(pointed_by[tid])
        refs_set = referenced_docs[tid]
        refs = sorted(refs_set)
        ptr_set = {p.split(":", 1)[0] for p in ptrs}
        # Symmetric diff between pointing docs and referenced docs
        one_sided_ref = sorted(refs_set - ptr_set)  # task refs doc, doc has no pointer
        one_sided_ptr = sorted(ptr_set - refs_set)  # doc points to task, task lacks ref
        rows.append(
            {
                "task": tid,
                "title": t.title,
                "status": t.status,
                "pointers": ptrs,
                "doc_refs": refs,
                "one_sided_doc_no_pointer": one_sided_ref,
                "one_sided_pointer_no_ref": one_sided_ptr,
                "orphan": (not ptrs and not refs),
            }
        )

    if args.format == "json":
        print(json.dumps(rows, indent=2, sort_keys=True))
        return 0

    # Human table: only show interesting rows by default
    if not args.show_all:
        rows = [
            r for r in rows
            if r["orphan"] or r["one_sided_doc_no_pointer"] or r["one_sided_pointer_no_ref"]
        ]
    if not rows:
        print("OK: every task has a matched pointer/ref relationship.")
        return 0
    for r in rows:
        flag = []
        if r["orphan"]:
            flag.append("ORPHAN")
        if r["one_sided_doc_no_pointer"]:
            flag.append("REF-NO-POINTER")
        if r["one_sided_pointer_no_ref"]:
            flag.append("POINTER-NO-REF")
        print(f"{r['task']} [{r['status']}] {','.join(flag) or 'ok'} - {r['title']}")
        if r["pointers"]:
            print(f"  pointers: {', '.join(r['pointers'])}")
        if r["doc_refs"]:
            print(f"  doc refs: {', '.join(r['doc_refs'])}")
        if r["one_sided_doc_no_pointer"]:
            print(f"  refs-without-pointer: {', '.join(r['one_sided_doc_no_pointer'])}")
        if r["one_sided_pointer_no_ref"]:
            print(f"  pointer-without-ref: {', '.join(r['one_sided_pointer_no_ref'])}")
    return 0


# ----- Subcommand: fix-suggest -----


def cmd_fix_suggest(args: argparse.Namespace) -> int:
    root = repo_root()
    tasks = load_tasks(root)
    paths = collect_target_paths(root, args.paths)
    pointers = collect_pointers(root, paths)
    suggestions = []
    for ptr in pointers:
        if ptr.ignored:
            continue
        ctx_tokens = tokenize(ptr.context)
        if not ctx_tokens:
            continue
        for tid in ptr.raw_ids:
            t = tasks.get(tid)
            if t is None:
                # Existence error -> still try to suggest based on context.
                current_score = 0.0
            else:
                current_score = jaccard(t.keywords, ctx_tokens)
            if current_score >= args.threshold:
                continue
            # Rank all tasks by Jaccard with the pointer context.
            ranked = sorted(
                (
                    (jaccard(other.keywords, ctx_tokens), other)
                    for other in tasks.values()
                ),
                key=lambda x: x[0],
                reverse=True,
            )
            top = [(s, ot) for s, ot in ranked[:3] if s > current_score and s > 0]
            if not top:
                continue
            suggestions.append(
                {
                    "file": ptr.file,
                    "line": ptr.line,
                    "current": tid,
                    "current_score": round(current_score, 3),
                    "candidates": [
                        {"task": ot.id, "score": round(s, 3), "title": ot.title}
                        for s, ot in top
                    ],
                }
            )
    if args.format == "json":
        print(json.dumps(suggestions, indent=2, sort_keys=True))
    else:
        if not suggestions:
            print("No improvement suggestions (all pointers above threshold).")
            return 0
        for s in suggestions:
            print(f"{s['file']}:{s['line']}  {s['current']} (score={s['current_score']:.2f})")
            for c in s["candidates"]:
                print(f"  -> {c['task']} score={c['score']:.2f}  '{c['title']}'")
    return 0


# ----- CLI -----


def build_parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = p.add_subparsers(dest="cmd", required=True)

    p_index = sub.add_parser("index", help="Print the Backlog.md task index.")
    p_index.add_argument("--format", choices=("table", "json"), default="table")
    p_index.set_defaults(func=cmd_index)

    p_audit = sub.add_parser("audit", help="Audit `Tracked in:` pointers against the backlog index.")
    p_audit.add_argument("paths", nargs="*", help="Optional explicit paths; default: tracked files matching default globs.")
    p_audit.add_argument("--format", choices=("table", "json"), default="table")
    p_audit.add_argument("--threshold", type=float, default=0.10, help="Jaccard topic threshold (default 0.10).")
    p_audit.set_defaults(func=cmd_audit)

    p_check = sub.add_parser("check", help="CI-friendly audit; non-zero exit only on existence errors.")
    p_check.add_argument("paths", nargs="*", help="Optional explicit paths.")
    p_check.add_argument("--format", choices=("table", "json"), default="table")
    p_check.add_argument("--threshold", type=float, default=0.10)
    p_check.set_defaults(func=lambda a: cmd_audit(a, strict=True))

    p_reverse = sub.add_parser("reverse", help="Reverse map: pointers + refs per task; highlight orphans/one-sided links.")
    p_reverse.add_argument("paths", nargs="*", help="Optional explicit paths.")
    p_reverse.add_argument("--format", choices=("table", "json"), default="table")
    p_reverse.add_argument("--show-all", action="store_true", help="Show all tasks, not just flagged.")
    p_reverse.set_defaults(func=cmd_reverse)

    p_fix = sub.add_parser("fix-suggest", help="Suggest better TASK-NN candidates for low-overlap pointers.")
    p_fix.add_argument("paths", nargs="*", help="Optional explicit paths.")
    p_fix.add_argument("--format", choices=("table", "json"), default="table")
    p_fix.add_argument("--threshold", type=float, default=0.10)
    p_fix.set_defaults(func=cmd_fix_suggest)

    return p


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        return args.func(args)
    except subprocess.CalledProcessError as e:
        print(f"error: {e}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
