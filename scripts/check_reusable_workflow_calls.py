#!/usr/bin/env python3
"""Validate GitHub reusable-workflow call syntax and graph limits.

actionlint validates most workflow structure, but releases through 1.7.12 and
current main accept several invalid reusable-workflow call forms. This checker
is deliberately independent and fail-closed for that policy surface.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path
from typing import Any

from governance_compare import WorkflowParseError, parse_workflow


MAX_WORKFLOW_LEVELS = 10
MAX_UNIQUE_LOCAL_CALLEES = 50
CALL_JOB_KEYS = frozenset(
    {"name", "uses", "with", "secrets", "strategy", "needs", "if", "concurrency", "permissions"}
)
LOCAL_CALL_RE = re.compile(r"^(?:\./|\$/)\.github/workflows/[^/@]+\.ya?ml$")
REMOTE_CALL_RE = re.compile(
    r"^(?P<owner>[A-Za-z0-9_.-]+)/(?P<repo>[A-Za-z0-9_.-]+)/"
    r"\.github/workflows/(?P<file>[^/@]+\.ya?ml)@(?P<ref>[^\s@]+)$"
)
REMOTE_WORKFLOW_HINT_RE = re.compile(r"(?:^|/)\.github/workflows/")


class ReusableWorkflowCallError(Exception):
    """A workflow violates reusable-workflow call policy."""


class ReusableWorkflowCallChecker:
    def __init__(self, root: Path) -> None:
        self.root = root.resolve()
        self.workflow_dir = self.root / ".github" / "workflows"
        self.documents: dict[Path, dict[str, Any]] = {}

    def _relative(self, path: Path) -> str:
        return path.relative_to(self.root).as_posix()

    def _fail(self, path: Path, message: str) -> None:
        raise ReusableWorkflowCallError(f"{self._relative(path)}: {message}")

    def _load(self, path: Path) -> dict[str, Any]:
        path = path.resolve()
        if path in self.documents:
            return self.documents[path]
        try:
            document = parse_workflow(path.read_text(encoding="utf-8"))
        except (OSError, WorkflowParseError) as error:
            raise ReusableWorkflowCallError(
                f"{self._relative(path)}: cannot parse workflow: {error}"
            ) from error
        self.documents[path] = document
        return document

    def _jobs(self, path: Path, document: dict[str, Any]) -> dict[str, dict[str, Any]]:
        jobs = document.get("jobs")
        if not isinstance(jobs, dict):
            self._fail(path, "workflow has no jobs mapping")
        if not all(isinstance(job, dict) for job in jobs.values()):
            self._fail(path, "workflow job is not a mapping")
        return jobs

    def _local_target(self, path: Path, uses: Any, location: str) -> Path | None:
        if not isinstance(uses, str):
            self._fail(path, f"{location}.uses must be a string")
        if "${{" in uses:
            self._fail(path, f"{location}.uses must not contain an expression: {uses!r}")

        if uses.startswith(("./", "$/")):
            if not LOCAL_CALL_RE.fullmatch(uses):
                self._fail(
                    path,
                    f"{location}.uses local reusable workflow must be exactly "
                    f"'./.github/workflows/<file>.yml' or "
                    f"'$/.github/workflows/<file>.yml' (also .yaml) without @ref: {uses!r}",
                )
            return (self.root / uses[2:]).resolve()

        match = REMOTE_CALL_RE.fullmatch(uses)
        if match:
            ref = match.group("ref")
            if ref.startswith(("refs/heads/", "refs/tags/")):
                self._fail(path, f"{location}.uses ref must omit refs/heads or refs/tags: {uses!r}")
            return None

        if REMOTE_WORKFLOW_HINT_RE.search(uses):
            self._fail(
                path,
                f"{location}.uses remote reusable workflow must be "
                f"'<owner>/<repo>/.github/workflows/<file>.yml@<ref>': {uses!r}",
            )
        self._fail(path, f"{location}.uses is not a reusable-workflow reference: {uses!r}")

    def _check_steps(self, path: Path, job_name: str, job: dict[str, Any]) -> None:
        steps = job.get("steps")
        if not isinstance(steps, list):
            return
        for index, step in enumerate(steps):
            if not isinstance(step, dict) or "uses" not in step:
                continue
            uses = step["uses"]
            location = f"jobs.{job_name}.steps[{index}]"
            if not isinstance(uses, str):
                self._fail(path, f"{location}.uses must be a string")
            if "${{" in uses:
                self._fail(path, f"{location}.uses must not contain an expression: {uses!r}")
            if uses.startswith(
                ("./.github/workflows/", "$/.github/workflows/")
            ) or REMOTE_WORKFLOW_HINT_RE.search(uses):
                self._fail(path, f"{location}.uses cannot call a reusable workflow; move it to job-level uses")

    @staticmethod
    def _accepts_workflow_call(document: dict[str, Any]) -> bool:
        events = document.get("on")
        return isinstance(events, dict) and "workflow_call" in events

    def _build_graph(self, paths: list[Path]) -> dict[Path, tuple[Path, ...]]:
        graph: dict[Path, tuple[Path, ...]] = {}
        for path in paths:
            document = self._load(path)
            targets: list[Path] = []
            for job_name, job in self._jobs(path, document).items():
                self._check_steps(path, job_name, job)
                if "uses" not in job:
                    continue
                unexpected = sorted(set(job) - CALL_JOB_KEYS)
                if unexpected:
                    self._fail(
                        path,
                        f"jobs.{job_name} uses a reusable workflow but has forbidden key(s): "
                        + ", ".join(unexpected),
                    )
                target = self._local_target(path, job["uses"], f"jobs.{job_name}")
                if target is not None:
                    if not target.is_file():
                        self._fail(path, f"jobs.{job_name}.uses local target does not exist: {self._relative(target)}")
                    target_document = self._load(target)
                    if not self._accepts_workflow_call(target_document):
                        self._fail(path, f"jobs.{job_name}.uses target does not declare on.workflow_call: {self._relative(target)}")
                    targets.append(target)
            graph[path.resolve()] = tuple(target.resolve() for target in targets)
        return graph

    def _check_cycles(self, graph: dict[Path, tuple[Path, ...]]) -> None:
        visited: set[Path] = set()
        active: list[Path] = []

        def visit(path: Path) -> None:
            if path in active:
                start = active.index(path)
                cycle = " -> ".join(self._relative(item) for item in (*active[start:], path))
                raise ReusableWorkflowCallError(f"reusable-workflow cycle: {cycle}")
            if path in visited:
                return
            active.append(path)
            for target in graph.get(path, ()):
                visit(target)
            active.pop()
            visited.add(path)

        for path in graph:
            visit(path)

    def _check_limits(self, graph: dict[Path, tuple[Path, ...]]) -> None:
        called = {target for targets in graph.values() for target in targets}
        roots = [path for path in graph if path not in called]

        def visit(path: Path, level: int, unique: set[Path]) -> None:
            if level > MAX_WORKFLOW_LEVELS:
                raise ReusableWorkflowCallError(
                    f"{self._relative(path)}: reusable-workflow chain exceeds "
                    f"{MAX_WORKFLOW_LEVELS} total workflow levels"
                )
            for target in graph.get(path, ()):
                unique.add(target)
                if len(unique) > MAX_UNIQUE_LOCAL_CALLEES:
                    raise ReusableWorkflowCallError(
                        f"{self._relative(path)}: call graph exceeds "
                        f"{MAX_UNIQUE_LOCAL_CALLEES} unique local reusable workflows"
                    )
                visit(target, level + 1, unique)

        for root in roots:
            visit(root, 1, set())

    def check(self) -> None:
        paths = sorted((*self.workflow_dir.glob("*.yml"), *self.workflow_dir.glob("*.yaml")))
        graph = self._build_graph(paths)
        self._check_cycles(graph)
        self._check_limits(graph)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("root", nargs="?", type=Path, default=Path.cwd())
    args = parser.parse_args(argv)
    try:
        ReusableWorkflowCallChecker(args.root).check()
    except ReusableWorkflowCallError as error:
        print(f"reusable workflow call policy violation: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
