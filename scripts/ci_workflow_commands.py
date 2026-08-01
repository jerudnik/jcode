#!/usr/bin/env python3
"""Extract the cargo build/test command list from a Fork CI job.

The 21-minute `Build & Test (macOS)` job and the `Linux Tests` job are the only
CI steps that actually build the release binary and run the test suite; the
ratchets/clippy that `preflight.sh` already mirrors are the cheap part. This
module reads the *exact* `cargo ...` invocations out of `.github/workflows/
fork-ci.yml` so `ci_local.sh` can run them on fleet hardware before a PR is
opened, and can never silently drift from what CI runs.

It is deliberately dependency-free (no PyYAML in the system Python): the
workflow is hand-formatted with a stable two-space step indentation, so a
narrow structural scan is more honest here than pulling in a parser we would
then have to trust to round-trip GitHub's YAML dialect. The scan is anchored to
`run:` blocks inside the named job and only ever emits lines that begin with
`cargo `, so a formatting change that breaks the assumption fails loudly (no
commands found) rather than silently running the wrong thing.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
WORKFLOW = REPO_ROOT / ".github/workflows/fork-ci.yml"

# A job header sits at 2-space indent: `  macos:` / `  linux-tests:`.
JOB_RE = re.compile(r"^  ([a-z0-9-]+):\s*$")
# `run:` step bodies are indented further; we only care about the cargo lines.
# Two shapes appear: a bare `cargo ...` line inside a `run: |` block, and an
# inline `run: cargo ...` on a single line. Both are captured.
CARGO_RE = re.compile(r"^\s*(?:run:\s*)?(cargo\s+.*)$")


def _job_span(lines: list[str], job: str) -> tuple[int, int]:
    """Return the [start, end) line span of `job:` within the workflow."""
    start = None
    for i, line in enumerate(lines):
        m = JOB_RE.match(line)
        if m and m.group(1) == job:
            start = i + 1
            continue
        if start is not None and JOB_RE.match(line):
            return start, i
    if start is None:
        raise SystemExit(f"ci_workflow_commands: job {job!r} not found in {WORKFLOW}")
    return start, len(lines)


def job_cargo_commands(job: str, workflow: Path = WORKFLOW) -> list[str]:
    """The ordered list of `cargo ...` commands CI runs in `job`."""
    lines = workflow.read_text(encoding="utf-8").splitlines()
    start, end = _job_span(lines, job)
    span = lines[start:end]

    commands: list[str] = []
    pending: str | None = None
    for raw in span:
        # A cargo line may be preceded on its own line by the timeout wrapper
        # ending in `\`; the next line(s) carry the cargo command.
        m = CARGO_RE.match(raw)
        if m and pending is None:
            pending = m.group(1)
        elif pending is not None:
            pending += " " + raw.strip()
        else:
            continue
        if pending.rstrip().endswith("\\"):
            pending = pending.rstrip()[:-1].rstrip()
            continue
        commands.append(re.sub(r"\s+", " ", pending).strip())
        pending = None
    if pending:
        commands.append(re.sub(r"\s+", " ", pending.replace("\\", "")).strip())
    return commands


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("job", help="workflow job id, e.g. macos or linux-tests")
    ap.add_argument(
        "--host-target",
        help="rewrite the pinned CI target triple to this one (e.g. the local host triple)",
    )
    args = ap.parse_args()

    commands = job_cargo_commands(args.job)
    if not commands:
        print(
            f"ci_workflow_commands: no cargo commands found in job {args.job!r}; "
            "the workflow format may have changed",
            file=sys.stderr,
        )
        return 1

    ci_targets = ("aarch64-apple-darwin", "x86_64-unknown-linux-gnu")
    for cmd in commands:
        out = cmd
        if args.host_target:
            for t in ci_targets:
                out = out.replace(f"--target {t}", f"--target {args.host_target}")
        print(out)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
