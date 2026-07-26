#!/usr/bin/env python3
"""Route work to the model that is actually good at it.

Three agents with different strengths are installed and authenticated on this
machine, and the useful observation is that they are not interchangeable:

  codex / gpt-5.6-sol   writes code fast and literally. Best first draft.
  claude / opus-5       better instincts for what a codebase actually wants.
                        Best reviewer, and it runs the tests rather than
                        reading the diff and opining.
  claude / fable-5      clever, sometimes too clever. Held in reserve as a
                        fresh perspective when the primary pair is stuck.

The orchestrator (the caller) writes the plan and remains the acceptance gate.
This module only dispatches, and it never decides that work is done.

The pipeline is deliberately asymmetric. The author cannot mark its own work
complete, and the reviewer is required to execute the verification command:
a review whose verification did not run is reported as unverified rather than
being quietly counted as a pass.

Usage:
    pipeline.py implement --task TEXT --verify CMD [--files F...] [--rounds N]
    pipeline.py review    --claim TEXT --verify CMD [--files F...]
    pipeline.py consult   --question TEXT [--files F...]      # fable-5
    pipeline.py roles                                          # show routing

Exit status: 0 when the reviewer passes and its verification actually ran,
1 on a fail, 2 when no trustworthy verdict was obtained.
"""

from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
import tempfile
import time
from dataclasses import dataclass, field
from pathlib import Path

TIMEOUT_S = 1800  # implementation turns can legitimately take a while
MAX_OUT = 16000


@dataclass(frozen=True)
class Role:
    """A (cli, model, effort) routing decision, named for its job."""

    cli: str
    model: str
    effort: str
    why: str


# Routing table. Swapping roles is a supported move when the primary pair
# stalls, hence `--author`/`--reviewer` overrides on the subcommands below.
ROLES = {
    "author": Role("codex", "gpt-5.6-sol", "high",
                   "fast, literal first drafts"),
    "reviewer": Role("claude", "claude-opus-5", "high",
                     "runs the tests; good taste in review"),
    "consultant": Role("claude", "claude-fable-5", "high",
                       "fresh perspective when stuck"),
    # Inverted assignments, for when the default pairing is not working.
    "author-alt": Role("claude", "claude-opus-5", "high",
                       "when the task needs judgement over speed"),
    "reviewer-alt": Role("codex", "gpt-5.6-sol", "high",
                         "when review needs literal rigor"),
}

VERDICT_SCHEMA = {
    "type": "object",
    "properties": {
        "verdict": {"type": "string", "enum": ["pass", "fail"]},
        "ran_verification": {"type": "boolean"},
        "evidence": {"type": "string"},
        "issues": {"type": "array", "items": {"type": "string"}},
        "suggestions": {"type": "array", "items": {"type": "string"}},
    },
    "required": ["verdict", "ran_verification", "evidence", "issues",
                 "suggestions"],
    "additionalProperties": False,
}

AUTHOR_BRIEF = """You are implementing a change in an existing codebase. Write \
the code; do not ask for confirmation.

- Match the surrounding conventions exactly. Read neighbouring code first.
- Make the verification command pass. It is the definition of done.
- Do not weaken, skip, or delete tests to make them pass. That is a failure.
- Do not add dependencies unless the task explicitly calls for one.
- Keep the change tight; unrelated refactors make review harder.
- Explain non-obvious decisions in comments, not the obvious ones."""

REVIEWER_BRIEF = """You are reviewing someone else's change. You did not write \
it. Your job is to find what is wrong.

- FIRST run the verification command yourself. Do not skip this.
- Report `ran_verification` honestly. Claiming a pass you did not observe is \
the worst outcome here.
- Check the tests actually exercise the behaviour, not just that they are \
green. Weakened or deleted assertions are a fail.
- Judge whether this is what the codebase would want, not merely whether it \
works.
- Ground every issue in something you can point at.
- `pass` means the work is genuinely done. An empty issues list is fine when \
the work is clean."""


@dataclass
class Result:
    role: str
    model: str
    ok: bool
    output: str
    seconds: float
    verdict: dict = field(default_factory=dict)


def _truncate(text: str, limit: int = MAX_OUT) -> str:
    return text if len(text) <= limit else (
        f"... [{len(text) - limit} chars elided] ...\n{text[-limit:]}")


def _files_block(paths: list[str], cwd: Path) -> str:
    out = []
    for raw in paths:
        try:
            out.append(f"--- {raw} ---\n"
                       f"{(cwd / raw).read_text(encoding='utf-8', errors='replace')}")
        except OSError as exc:
            out.append(f"--- {raw} ---\n[unreadable: {exc}]")
    return "\n\n".join(out)


def run_codex(prompt: str, cwd: Path, role: Role, *, write: bool,
              schema: dict | None = None,
              allow: list[str] | None = None) -> tuple[str, str]:
    """Invoke codex. Returns (stdout, structured_json_or_empty).

    `allow` is accepted for signature parity with `run_claude` and ignored:
    codex governs command execution through its sandbox mode, not an allowlist.
    """
    del allow
    if not shutil.which("codex"):
        raise RuntimeError("codex CLI not installed")

    with tempfile.TemporaryDirectory() as tmp:
        cmd = ["codex", "exec",
               "-s", "workspace-write" if write else "read-only",
               "--skip-git-repo-check", "-C", str(cwd),
               "-m", role.model,
               "-c", f"model_reasoning_effort={role.effort}"]
        out_path = None
        if schema:
            sp, out_path = Path(tmp) / "s.json", Path(tmp) / "o.json"
            sp.write_text(json.dumps(schema), encoding="utf-8")
            cmd += ["--output-schema", str(sp), "-o", str(out_path)]
        cmd.append(prompt)

        proc = subprocess.run(cmd, capture_output=True, text=True,
                              timeout=TIMEOUT_S, stdin=subprocess.DEVNULL)
        structured = ""
        if out_path and out_path.exists():
            structured = out_path.read_text(encoding="utf-8").strip()
        return proc.stdout + proc.stderr, structured


def run_claude(prompt: str, cwd: Path, role: Role, *, write: bool,
               schema: dict | None = None,
               allow: list[str] | None = None) -> tuple[str, str]:
    """Invoke claude. No native schema flag, so shape is requested in-prompt.

    `allow` pre-approves specific tool invocations. A reviewer that is asked to
    run the tests but denied permission to do so will (correctly) report that it
    could not verify, so the command it needs must be granted explicitly. The
    grant is narrow by design rather than a blanket permission bypass.
    """
    if not shutil.which("claude"):
        raise RuntimeError("claude CLI not installed")

    if schema:
        prompt += ("\n\nEnd your reply with ONLY a JSON object matching:\n"
                   + json.dumps(schema))
    cmd = ["claude", "-p", prompt,
           "--model", role.model,
           "--effort", role.effort,
           "--permission-mode", "acceptEdits" if write else "plan"]
    if allow:
        cmd += ["--allowedTools", *allow]

    proc = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True,
                          timeout=TIMEOUT_S, stdin=subprocess.DEVNULL)
    text = proc.stdout + proc.stderr
    return text, _extract_json(proc.stdout) if schema else ""


def _extract_json(text: str) -> str:
    """Pull the verdict object out of prose. Reviewers narrate before answering.

    Scanning for the last balanced brace pair is not enough in practice: a
    reviewer suggesting code frequently writes a stray `{` or `}` in prose after
    the verdict, which desynchronises a single backward scan. So every candidate
    start offset is tried, newest first, and the first one that both parses and
    carries a `verdict` key wins. Validity is the selector, not position.
    """
    starts = [i for i, ch in enumerate(text) if ch == "{"]
    for start in reversed(starts):
        decoded, _ = _try_decode(text, start)
        if isinstance(decoded, dict) and "verdict" in decoded:
            return json.dumps(decoded)
    return ""


def _try_decode(text: str, start: int) -> tuple[object | None, int]:
    """Best-effort decode of one JSON value beginning at `start`."""
    try:
        return json.JSONDecoder().raw_decode(text, start)
    except (json.JSONDecodeError, ValueError):
        return None, start


DISPATCH = {"codex": run_codex, "claude": run_claude}


def _allow_for(verify_cmd: str) -> list[str]:
    """Allowlist entries permitting the reviewer to run the verification.

    Scoped to the binary the command actually invokes rather than granting
    blanket Bash, so the reviewer can prove its verdict without being handed
    the keys to the machine.
    """
    head = verify_cmd.strip().split()[0] if verify_cmd.strip() else ""
    binary = Path(head).name or head
    grants = {"Read", "Grep", "Glob"}
    if binary:
        # Guard against an empty binary producing a meaningless `Bash(:*)`
        # grant, which reads like a permission but matches nothing.
        grants.add(f"Bash({binary}:*)")
    # Test runs commonly shell out to these; without them a suite can fail for
    # reasons that have nothing to do with the change under review.
    grants.update(f"Bash({b}:*)" for b in ("cargo", "python3", "pytest", "make")
                  if b == binary or binary in ("cargo", "python3"))
    return sorted(grants)


def dispatch(role: Role, role_name: str, prompt: str, cwd: Path, *,
             write: bool, schema: dict | None = None,
             allow: list[str] | None = None) -> Result:
    start = time.monotonic()
    label = f"{role.cli}/{role.model}@{role.effort}"
    print(f"  -> {role_name}: {label}", file=sys.stderr, flush=True)
    try:
        text, structured = DISPATCH[role.cli](prompt, cwd, role, write=write,
                                              schema=schema, allow=allow)
        ok = True
    except (subprocess.TimeoutExpired, RuntimeError, OSError) as exc:
        text, structured, ok = f"[dispatch failed: {exc}]", "", False

    verdict = {}
    if structured:
        try:
            verdict = json.loads(structured)
        except json.JSONDecodeError:
            verdict = {}

    elapsed = time.monotonic() - start
    print(f"     done in {elapsed:.0f}s", file=sys.stderr, flush=True)
    return Result(role_name, label, ok, _truncate(text), elapsed, verdict)


def verify(cmd: str, cwd: Path) -> tuple[bool, str]:
    """Ground truth. The orchestrator trusts this, not the agents' narration."""
    try:
        p = subprocess.run(cmd, shell=True, cwd=cwd, capture_output=True,
                           text=True, timeout=TIMEOUT_S)
    except subprocess.TimeoutExpired:
        return False, f"[TIMED OUT after {TIMEOUT_S}s]"
    tail = _truncate((p.stdout + p.stderr).strip())
    return p.returncode == 0, f"[exit {p.returncode}]\n{tail}"


def do_review(claim: str, verify_cmd: str, files: list[str], cwd: Path,
              role: Role) -> Result:
    prompt = (f"{REVIEWER_BRIEF}\n\n## Change under review\n{claim}\n\n"
              f"## Verification command (run this yourself)\n`{verify_cmd}`\n")
    if files:
        prompt += f"\n## Relevant files\n{_files_block(files, cwd)}\n"
    # The reviewer is required to run the verification, so it must be
    # pre-approved: a denied reviewer reports "could not verify", which is
    # correct behaviour but useless as a gate. Write access is also needed
    # because test runs create build artifacts and temp dirs.
    return dispatch(role, "reviewer", prompt, cwd, write=True,
                    schema=VERDICT_SCHEMA, allow=_allow_for(verify_cmd))


def do_implement(task: str, verify_cmd: str, files: list[str], cwd: Path,
                 rounds: int, author: Role, reviewer: Role) -> int:
    feedback = ""
    for rnd in range(1, rounds + 1):
        print(f"\n=== round {rnd}/{rounds} ===", file=sys.stderr)

        prompt = (f"{AUTHOR_BRIEF}\n\n## Task\n{task}\n\n"
                  f"## Definition of done\n`{verify_cmd}` must pass.\n")
        if files:
            prompt += f"\n## Relevant files\n{_files_block(files, cwd)}\n"
        if feedback:
            prompt += (f"\n## Reviewer feedback on your previous attempt"
                       f"\nAddress these specifically.\n{feedback}\n")

        dispatch(author, "author", prompt, cwd, write=True)

        # Orchestrator's own check, independent of what either agent claims.
        passed, evidence = verify(verify_cmd, cwd)
        print(f"  orchestrator verify: {'PASS' if passed else 'FAIL'}",
              file=sys.stderr)
        if not passed:
            feedback = f"The verification command still fails:\n{evidence}"
            continue

        result = do_review(task, verify_cmd, files, cwd, reviewer)
        v = result.verdict
        if not v:
            print("\nReviewer returned no usable verdict.", file=sys.stderr)
            return 2

        print(f"\n--- reviewer ({result.model}) ---")
        print(f"verdict: {v.get('verdict')}  "
              f"ran_verification: {v.get('ran_verification')}")
        print(f"evidence: {v.get('evidence', '')[:400]}")
        for issue in v.get("issues") or []:
            print(f"  ! {issue}")
        for s in v.get("suggestions") or []:
            print(f"  ~ {s}")

        if v.get("verdict") == "pass":
            if not v.get("ran_verification"):
                # Passing without observing the tests is not a pass.
                print("\nUNVERIFIED: reviewer passed without running the "
                      "command. Orchestrator must adjudicate.", file=sys.stderr)
                return 2
            print("\nReviewer passed and confirmed it ran the verification.")
            print("Orchestrator: you are the acceptance gate. Inspect the diff.")
            return 0

        feedback = (f"{v.get('evidence', '')}\n"
                    + "\n".join(v.get("issues") or [])
                    + "\n" + "\n".join(v.get("suggestions") or []))

    print(f"\nExhausted {rounds} round(s) without a passing review.",
          file=sys.stderr)
    return 1


def main() -> int:
    ap = argparse.ArgumentParser(
        description=__doc__,
        formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)

    p = sub.add_parser("implement", help="author writes, reviewer verifies")
    p.add_argument("--task", required=True)
    p.add_argument("--verify", required=True, help="command defining done")
    p.add_argument("--files", nargs="*", default=[])
    p.add_argument("--rounds", type=int, default=2)
    p.add_argument("--author", default="author", choices=sorted(ROLES))
    p.add_argument("--reviewer", default="reviewer", choices=sorted(ROLES))
    p.add_argument("--cwd", default=".")

    p = sub.add_parser("review", help="review existing work")
    p.add_argument("--claim", required=True)
    p.add_argument("--verify", required=True)
    p.add_argument("--files", nargs="*", default=[])
    p.add_argument("--reviewer", default="reviewer", choices=sorted(ROLES))
    p.add_argument("--cwd", default=".")

    p = sub.add_parser("consult", help="ask fable-5 for a fresh perspective")
    p.add_argument("--question", required=True)
    p.add_argument("--files", nargs="*", default=[])
    p.add_argument("--cwd", default=".")

    sub.add_parser("roles", help="show the routing table")

    args = ap.parse_args()

    if args.cmd == "roles":
        print(f"{'role':<14}{'cli':<9}{'model':<18}{'effort':<8}why")
        for name, r in ROLES.items():
            print(f"{name:<14}{r.cli:<9}{r.model:<18}{r.effort:<8}{r.why}")
        return 0

    cwd = Path(args.cwd).resolve()

    if args.cmd == "consult":
        prompt = (f"Give a fresh perspective on this problem. Be direct about "
                  f"what you would do differently.\n\n{args.question}\n")
        if args.files:
            prompt += f"\n## Files\n{_files_block(args.files, cwd)}\n"
        res = dispatch(ROLES["consultant"], "consultant", prompt, cwd,
                       write=False)
        print(res.output)
        return 0 if res.ok else 2

    if args.cmd == "review":
        res = do_review(args.claim, args.verify, args.files, cwd,
                        ROLES[args.reviewer])
        v = res.verdict
        if not v:
            print(res.output)
            return 2
        print(json.dumps(v, indent=2))
        if v.get("verdict") == "pass" and not v.get("ran_verification"):
            return 2
        return 0 if v.get("verdict") == "pass" else 1

    return do_implement(args.task, args.verify, args.files, cwd, args.rounds,
                        ROLES[args.author], ROLES[args.reviewer])


if __name__ == "__main__":
    sys.exit(main())
