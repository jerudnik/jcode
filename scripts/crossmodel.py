#!/usr/bin/env python3
"""Ask an independent model to adjudicate a claim, and make it show its work.

The coordinator agent cannot review its own output usefully: it shares every
blind spot with the thing under review. This drives the two other CLIs already
installed and authenticated on this machine (`codex exec` on GPT-5.x and
`claude -p` on Claude Opus) as second opinions from genuinely different model
families, so their failure modes are uncorrelated with the coordinator's.

The design constraint that matters is that a reviewer which only reads a diff
produces opinions, not findings. So a review here is anchored to a command:
the harness runs it first, feeds the real exit code and output to the reviewer,
and requires a structured verdict referencing that evidence. A reviewer that
contradicts a passing test has to say so explicitly, which is a claim that can
itself be checked.

Reviewers are sandboxed read-only. They observe and judge; they do not edit.

Usage:
    crossmodel.py review  --claim TEXT [--verify CMD] [--files F...] [--model M]
    crossmodel.py compare --claim TEXT [--verify CMD] [--files F...]
    crossmodel.py ask     --prompt TEXT [--reviewer codex|claude]

Exit status is 0 when every reviewer returns `pass`, 1 on any `fail`, and 2 if
a reviewer could not be reached at all (never silently treated as agreement).
"""

from __future__ import annotations

import argparse
import concurrent.futures
import json
import shutil
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path

# Reviewers get a hard wall-clock bound. A hung reviewer must surface as an
# error rather than stalling the caller that is waiting on its verdict.
TIMEOUT_S = 600
# Evidence is truncated so a chatty test suite cannot push the actual claim out
# of the reviewer's context window.
MAX_EVIDENCE = 12000
MAX_FILE_BYTES = 60000

VERDICT_SCHEMA = {
    "type": "object",
    "properties": {
        "verdict": {"type": "string", "enum": ["pass", "fail"]},
        "confidence": {"type": "string", "enum": ["low", "medium", "high"]},
        "reason": {"type": "string"},
        "issues": {"type": "array", "items": {"type": "string"}},
    },
    "required": ["verdict", "confidence", "reason", "issues"],
    "additionalProperties": False,
}

RUBRIC = """You are an independent reviewer. You did NOT write this work, and \
your job is to find what is wrong with it, not to be agreeable.

Rules:
- Ground every issue in the evidence below. Do not speculate about code you \
cannot see.
- If verification output is included, it is authoritative. To return `fail` \
despite a passing command, you must state exactly what the command fails to \
cover.
- `pass` means: the claim is supported by the evidence. It does not mean the \
code is perfect.
- `fail` means: you found a concrete, named defect or a specific gap between \
the claim and the evidence.
- Prefer a small number of real findings over a long list of style nits.
- Report only issues you can point at. An empty `issues` list is a valid and \
respectable answer."""


@dataclass
class Verdict:
    """One reviewer's answer, or the reason there isn't one."""

    reviewer: str
    verdict: str  # pass | fail | error
    confidence: str
    reason: str
    issues: list[str]

    @property
    def ok(self) -> bool:
        return self.verdict == "pass"

    @classmethod
    def error(cls, reviewer: str, reason: str) -> "Verdict":
        return cls(reviewer, "error", "low", reason, [])


def _truncate(text: str, limit: int) -> str:
    """Keep the tail: failures and summaries land at the end of most output."""
    if len(text) <= limit:
        return text
    return f"... [{len(text) - limit} chars elided] ...\n{text[-limit:]}"


def run_verification(cmd: str, cwd: Path) -> tuple[bool, str]:
    """Run the anchor command, returning (passed, transcript).

    A reviewer is only as good as the ground truth it is handed, so the exit
    code is reported verbatim rather than being inferred from output text.
    """
    try:
        proc = subprocess.run(
            cmd,
            shell=True,
            cwd=cwd,
            capture_output=True,
            text=True,
            timeout=TIMEOUT_S,
        )
    except subprocess.TimeoutExpired:
        return False, f"$ {cmd}\n[TIMED OUT after {TIMEOUT_S}s]"
    except OSError as exc:  # command not found, permission, etc.
        return False, f"$ {cmd}\n[could not execute: {exc}]"

    body = _truncate((proc.stdout + proc.stderr).strip(), MAX_EVIDENCE)
    status = "PASSED" if proc.returncode == 0 else f"FAILED (exit {proc.returncode})"
    return proc.returncode == 0, f"$ {cmd}\n[{status}]\n{body}"


def gather_files(paths: list[str], cwd: Path) -> str:
    """Inline the files under review; reviewers run read-only and cannot fetch."""
    chunks = []
    for raw in paths:
        path = (cwd / raw).resolve()
        try:
            text = path.read_text(encoding="utf-8", errors="replace")
        except OSError as exc:
            chunks.append(f"--- {raw} ---\n[unreadable: {exc}]")
            continue
        chunks.append(f"--- {raw} ---\n{_truncate(text, MAX_FILE_BYTES)}")
    return "\n\n".join(chunks)


def build_prompt(claim: str, evidence: str, files: str) -> str:
    parts = [RUBRIC, f"\n## Claim under review\n{claim}"]
    if evidence:
        parts.append(f"\n## Verification output (authoritative)\n{evidence}")
    if files:
        parts.append(f"\n## Files\n{files}")
    parts.append(
        "\nReturn your verdict as JSON matching the required schema. "
        "Judge only the claim above."
    )
    return "\n".join(parts)


def _parse_verdict(reviewer: str, raw: str) -> Verdict:
    """Coerce a reviewer's reply into a Verdict.

    Unparseable output is an error, never a pass: a reviewer that cannot be
    understood has not agreed with anything.
    """
    try:
        data = json.loads(raw)
    except (json.JSONDecodeError, TypeError):
        return Verdict.error(reviewer, f"unparseable reply: {raw[:200]}")
    if not isinstance(data, dict) or "verdict" not in data:
        return Verdict.error(reviewer, f"reply missing verdict: {raw[:200]}")
    return Verdict(
        reviewer=reviewer,
        verdict=str(data.get("verdict", "error")),
        confidence=str(data.get("confidence", "low")),
        reason=str(data.get("reason", "")),
        issues=[str(i) for i in data.get("issues") or []],
    )


def review_codex(prompt: str, cwd: Path, model: str | None) -> Verdict:
    """Codex enforces the schema natively via --output-schema."""
    if not shutil.which("codex"):
        return Verdict.error("codex", "codex CLI not installed")

    with tempfile.TemporaryDirectory() as tmp:
        schema = Path(tmp) / "schema.json"
        out = Path(tmp) / "out.json"
        schema.write_text(json.dumps(VERDICT_SCHEMA), encoding="utf-8")

        cmd = ["codex", "exec", "-s", "read-only", "--skip-git-repo-check",
               "-C", str(cwd), "--output-schema", str(schema),
               "-o", str(out)]
        if model:
            cmd += ["-m", model]
        cmd.append(prompt)

        try:
            subprocess.run(
                cmd, capture_output=True, text=True,
                timeout=TIMEOUT_S, stdin=subprocess.DEVNULL,
            )
        except subprocess.TimeoutExpired:
            return Verdict.error("codex", f"timed out after {TIMEOUT_S}s")
        except OSError as exc:
            return Verdict.error("codex", f"could not execute: {exc}")

        if not out.exists():
            return Verdict.error("codex", "produced no output file")
        return _parse_verdict("codex", out.read_text(encoding="utf-8").strip())


def review_claude(prompt: str, cwd: Path, model: str | None) -> Verdict:
    """Claude Code has no schema flag, so the shape is requested in-prompt."""
    if not shutil.which("claude"):
        return Verdict.error("claude", "claude CLI not installed")

    schema_hint = (
        f"{prompt}\n\nRespond with ONLY a JSON object (no markdown fence) "
        f"matching this schema:\n{json.dumps(VERDICT_SCHEMA)}"
    )
    cmd = ["claude", "-p", schema_hint, "--permission-mode", "plan"]
    if model:
        cmd += ["--model", model]

    try:
        proc = subprocess.run(
            cmd, cwd=cwd, capture_output=True, text=True,
            timeout=TIMEOUT_S, stdin=subprocess.DEVNULL,
        )
    except subprocess.TimeoutExpired:
        return Verdict.error("claude", f"timed out after {TIMEOUT_S}s")
    except OSError as exc:
        return Verdict.error("claude", f"could not execute: {exc}")

    text = proc.stdout.strip()
    # Tolerate a markdown fence even though the prompt forbids one.
    if text.startswith("```"):
        text = text.split("```")[1] if "```" in text[3:] else text
        text = text.removeprefix("json").strip()
    # Fall back to the outermost brace pair if prose leaked in around it.
    if not text.startswith("{") and "{" in text:
        text = text[text.index("{"): text.rindex("}") + 1]
    return _parse_verdict("claude", text)


REVIEWERS = {"codex": review_codex, "claude": review_claude}


def render(verdicts: list[Verdict], anchored: bool) -> int:
    """Print a report and derive the exit code. Errors never count as pass."""
    print("\n" + "=" * 68)
    print("CROSS-MODEL REVIEW" + ("" if anchored else "  (no verification command: opinion only)"))
    print("=" * 68)

    for v in verdicts:
        mark = {"pass": "PASS", "fail": "FAIL"}.get(v.verdict, "ERROR")
        print(f"\n[{mark}] {v.reviewer}  (confidence: {v.confidence})")
        print(f"  {v.reason}")
        for issue in v.issues:
            print(f"    - {issue}")

    passed = [v for v in verdicts if v.verdict == "pass"]
    failed = [v for v in verdicts if v.verdict == "fail"]
    errored = [v for v in verdicts if v.verdict == "error"]

    print("\n" + "-" * 68)
    print(f"{len(passed)} pass, {len(failed)} fail, {len(errored)} error")

    # Disagreement is the signal worth surfacing loudest: it means at least one
    # reviewer saw something the other did not.
    if passed and failed:
        print("REVIEWERS DISAGREE -- inspect both arguments before proceeding.")
    if errored:
        print("Some reviewers failed to respond; absence of a verdict is not agreement.")

    if errored and not failed:
        return 2
    return 1 if failed else 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)

    for name in ("review", "compare"):
        p = sub.add_parser(name)
        p.add_argument("--claim", required=True, help="the assertion to adjudicate")
        p.add_argument("--verify", help="command whose result anchors the review")
        p.add_argument("--files", nargs="*", default=[], help="files to inline")
        p.add_argument("--cwd", default=".", help="working directory")
        if name == "review":
            p.add_argument("--reviewer", default="codex", choices=sorted(REVIEWERS))
            p.add_argument("--model", help="override the reviewer's model")

    p = sub.add_parser("ask")
    p.add_argument("--prompt", required=True)
    p.add_argument("--reviewer", default="codex", choices=sorted(REVIEWERS))
    p.add_argument("--model")
    p.add_argument("--cwd", default=".")

    args = ap.parse_args()
    cwd = Path(args.cwd).resolve()

    if args.cmd == "ask":
        v = REVIEWERS[args.reviewer](args.prompt, cwd, args.model)
        print(json.dumps(v.__dict__, indent=2))
        return 0 if v.ok else 1

    evidence = ""
    if args.verify:
        print(f"Running verification: {args.verify}", file=sys.stderr)
        ok, evidence = run_verification(args.verify, cwd)
        print(f"  -> {'passed' if ok else 'FAILED'}", file=sys.stderr)

    prompt = build_prompt(args.claim, evidence, gather_files(args.files, cwd))

    if args.cmd == "review":
        print(f"Asking {args.reviewer}...", file=sys.stderr)
        verdicts = [REVIEWERS[args.reviewer](prompt, cwd, args.model)]
    else:
        # Run both families concurrently; they are independent by construction.
        print(f"Asking {', '.join(sorted(REVIEWERS))} in parallel...", file=sys.stderr)
        with concurrent.futures.ThreadPoolExecutor(max_workers=len(REVIEWERS)) as pool:
            futures = {name: pool.submit(fn, prompt, cwd, None)
                       for name, fn in sorted(REVIEWERS.items())}
            verdicts = [f.result() for f in futures.values()]

    return render(verdicts, anchored=bool(args.verify))


if __name__ == "__main__":
    sys.exit(main())
