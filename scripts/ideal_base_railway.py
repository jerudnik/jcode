#!/usr/bin/env python3
"""Validate and checkpoint the ideal-base execution railway."""

from __future__ import annotations

import argparse
import fcntl
import fnmatch
import hashlib
import json
import os
import re
import subprocess
import sys
import tempfile
from collections import Counter, defaultdict, deque
from datetime import datetime
from pathlib import Path
from typing import Any

REPO_ROOT = Path(__file__).resolve().parents[1]
CONTROL_ROOT = REPO_ROOT / "docs/fork/ideal-base"
GRAPH_PATH = CONTROL_ROOT / "WORK_GRAPH.json"
STATE_PATH = CONTROL_ROOT / "STATE.json"
BOOTSTRAP_PATH = CONTROL_ROOT / "COORDINATOR_BOOTSTRAP.md"
PROTECTED_PROMPT = REPO_ROOT / "docs/fork/recovery/ORCHESTRATOR_PROMPT.md"
PROTECTED_PROMPT_SHA256 = (
    "ca3f19980b1e4fab0a734397d7c6f41ccd5c203a4fa209cfe9eef2f16beed5b6"
)

ALLOWED_STATES = {
    "pending",
    "in_progress",
    "implemented",
    "verifying",
    "accepted",
    "authorization_blocked",
    "superseded",
    "rejected",
    "blocked",
}
DEPENDENCY_COMPLETE = {"accepted", "authorization_blocked", "superseded"}
ARTIFACT_FIELDS = {
    "findings",
    "evidence",
    "edge_cases_considered",
    "validation",
    "open_questions",
    "confidence",
    "what_i_did_not_check",
}
MARKDOWN_LINK = re.compile(r"\[[^\]]*\]\(([^)]+)\)")
BOOTSTRAP_REQUIRED_TEXT = (
    "Read these files completely before mutation:",
    "python3 scripts/ideal_base_railway.py check",
    '`mode: "deep"`',
    "After each accepted node:",
    "Do not push.",
    "Continue until every mandatory deterministic node is accepted",
)
ARCHIVE_MARKER_PATHS = [
    REPO_ROOT / "docs/fork/README.md",
    REPO_ROOT / "docs/fork/archive/README.md",
    REPO_ROOT / "docs/fork/normalization/README.md",
    REPO_ROOT / "docs/fork/normalization/STATUS.md",
    REPO_ROOT / "docs/fork/normalization/KNOWN_GOOD_BASELINE.md",
    REPO_ROOT / "docs/fork/normalization/COMPLETION_STANDARD.md",
    REPO_ROOT / "docs/fork/normalization/QUALITY_DEBT.md",
    REPO_ROOT / "docs/fork/normalization/R03A_R02_CLOSURE.md",
    REPO_ROOT / "docs/fork/normalization/RUNTIME_AND_NIX_RUNBOOK.md",
    REPO_ROOT / "docs/fork/normalization/N1_STACK_PLAN.md",
    REPO_ROOT / "docs/fork/recovery/README.md",
    REPO_ROOT / "docs/fork/recovery/PRESCREEN.md",
    REPO_ROOT / "docs/fork/recovery/SEAM_LEDGER_TEMPLATE.md",
    REPO_ROOT / "docs/fork/recovery/seams/README.md",
]


class RailwayError(RuntimeError):
    """A deterministic railway validation failure."""


def load_json(path: Path) -> dict[str, Any]:
    try:
        data = json.loads(path.read_text())
    except FileNotFoundError as exc:
        raise RailwayError(
            f"missing required file: {path.relative_to(REPO_ROOT)}"
        ) from exc
    except json.JSONDecodeError as exc:
        raise RailwayError(
            f"invalid JSON in {path.relative_to(REPO_ROOT)}:{exc.lineno}:{exc.colno}: {exc.msg}"
        ) from exc
    if not isinstance(data, dict):
        raise RailwayError(
            f"top-level JSON value must be an object: {path.relative_to(REPO_ROOT)}"
        )
    return data


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def git_commit_reachable(commit: str) -> bool:
    result = subprocess.run(
        ["git", "cat-file", "-e", f"{commit}^{{commit}}"],
        cwd=REPO_ROOT,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        check=False,
    )
    return result.returncode == 0


def node_index(graph: dict[str, Any]) -> dict[str, dict[str, Any]]:
    roots = graph.get("root_nodes")
    children = graph.get("all_nodes")
    if not isinstance(roots, list) or not isinstance(children, list):
        raise RailwayError(
            "WORK_GRAPH.json must contain root_nodes and all_nodes arrays"
        )
    nodes: dict[str, dict[str, Any]] = {}
    for raw in [*roots, *children]:
        if not isinstance(raw, dict):
            raise RailwayError("every graph node must be an object")
        node_id = raw.get("id")
        if not isinstance(node_id, str) or not node_id:
            raise RailwayError("every graph node must have a non-empty string id")
        if node_id in nodes:
            raise RailwayError(f"duplicate graph node id: {node_id}")
        nodes[node_id] = raw
    return nodes


def validate_dag(nodes: dict[str, dict[str, Any]]) -> None:
    indegree = {node_id: 0 for node_id in nodes}
    outgoing: dict[str, list[str]] = defaultdict(list)
    for node_id, node in nodes.items():
        dependencies = node.get("depends_on", [])
        if not isinstance(dependencies, list) or not all(
            isinstance(item, str) for item in dependencies
        ):
            raise RailwayError(f"{node_id}: depends_on must be an array of node ids")
        if node_id in dependencies:
            raise RailwayError(f"{node_id}: node cannot depend on itself")
        for dependency in dependencies:
            if dependency not in nodes:
                raise RailwayError(f"{node_id}: unknown dependency {dependency}")
            outgoing[dependency].append(node_id)
            indegree[node_id] += 1
    queue = deque(
        sorted(node_id for node_id, degree in indegree.items() if degree == 0)
    )
    visited = []
    while queue:
        current = queue.popleft()
        visited.append(current)
        for child in outgoing[current]:
            indegree[child] -= 1
            if indegree[child] == 0:
                queue.append(child)
    if len(visited) != len(nodes):
        cyclic = sorted(node_id for node_id, degree in indegree.items() if degree > 0)
        raise RailwayError(
            f"task graph contains a dependency cycle involving: {', '.join(cyclic)}"
        )


def dependency_closure(nodes: dict[str, dict[str, Any]], start: str) -> set[str]:
    seen: set[str] = set()
    pending = list(nodes[start].get("depends_on", []))
    while pending:
        current = pending.pop()
        if current in seen:
            continue
        seen.add(current)
        pending.extend(nodes[current].get("depends_on", []))
    return seen


def ownership_prefix(pattern: str) -> str:
    indices = [pattern.find(character) for character in "*?[" if character in pattern]
    end = min(indices) if indices else len(pattern)
    return pattern[:end].rstrip("/")


def ownership_paths_overlap(left: str, right: str) -> bool:
    if left == right:
        return True
    left_is_glob = any(character in left for character in "*?[")
    right_is_glob = any(character in right for character in "*?[")
    if not left_is_glob and not right_is_glob:
        return False
    if left_is_glob and not right_is_glob:
        return fnmatch.fnmatchcase(right, left)
    if right_is_glob and not left_is_glob:
        return fnmatch.fnmatchcase(left, right)
    left_prefix = ownership_prefix(left)
    right_prefix = ownership_prefix(right)
    if not left_prefix or not right_prefix:
        return True
    return (
        left_prefix == right_prefix
        or left_prefix.startswith(f"{right_prefix}/")
        or right_prefix.startswith(f"{left_prefix}/")
    )


def validate_ownership(nodes: dict[str, dict[str, Any]]) -> None:
    ownerships: list[tuple[str, str]] = []
    root_ids = {node_id for node_id, node in nodes.items() if "parent" not in node}
    root_closures = {
        root_id: dependency_closure(nodes, root_id) for root_id in root_ids
    }

    def ordered(left: str, right: str) -> bool:
        left_parent = nodes[left].get("parent", left)
        right_parent = nodes[right].get("parent", right)
        if left_parent == right_parent:
            left_closure = dependency_closure(nodes, left)
            right_closure = dependency_closure(nodes, right)
            return left in right_closure or right in left_closure
        return left_parent in root_closures.get(
            right_parent, set()
        ) or right_parent in root_closures.get(left_parent, set())

    for node_id, node in nodes.items():
        paths = node.get("owned_paths", [])
        if paths and (
            not isinstance(paths, list)
            or not all(isinstance(path, str) and path for path in paths)
        ):
            raise RailwayError(
                f"{node_id}: owned_paths must be an array of non-empty strings"
            )
        for path in paths:
            ownerships.append((node_id, path))
    for index, (left_node, left_path) in enumerate(ownerships):
        for right_node, right_path in ownerships[index + 1 :]:
            if left_node == right_node or not ownership_paths_overlap(
                left_path, right_path
            ):
                continue
            if ordered(left_node, right_node):
                continue
            raise RailwayError(
                "unserialized ownership overlap: "
                f"{left_node} ({left_path}) and {right_node} ({right_path})"
            )


def validate_markdown_links() -> None:
    paths = set(CONTROL_ROOT.rglob("*.md")) | set(ARCHIVE_MARKER_PATHS)
    for path in sorted(paths):
        for target in MARKDOWN_LINK.findall(path.read_text()):
            target = target.strip().split("#", 1)[0]
            if not target or target.startswith(("http://", "https://", "mailto:")):
                continue
            resolved = (path.parent / target).resolve()
            try:
                resolved.relative_to(REPO_ROOT.resolve())
            except ValueError as exc:
                raise RailwayError(
                    f"link escapes repository: {path.relative_to(REPO_ROOT)} -> {target}"
                ) from exc
            if not resolved.exists():
                raise RailwayError(
                    f"broken link: {path.relative_to(REPO_ROOT)} -> {target}"
                )


def validate_bootstrap_prompt(path: Path = BOOTSTRAP_PATH) -> str:
    lines = path.read_text().splitlines()
    openings = [index for index, line in enumerate(lines) if line == "````markdown"]
    closings = [index for index, line in enumerate(lines) if line == "````"]
    if len(openings) != 1 or len(closings) != 1 or openings[0] >= closings[0]:
        raise RailwayError(
            "COORDINATOR_BOOTSTRAP.md must contain one complete copyable prompt"
        )
    if closings[0] != len(lines) - 1:
        raise RailwayError(
            "COORDINATOR_BOOTSTRAP.md prompt fence must close at end of file"
        )
    prompt = "\n".join(lines[openings[0] + 1 : closings[0]])
    missing = [text for text in BOOTSTRAP_REQUIRED_TEXT if text not in prompt]
    if missing:
        raise RailwayError(
            f"COORDINATOR_BOOTSTRAP.md copyable prompt is incomplete: {missing}"
        )
    return prompt


def validate_graph(graph: dict[str, Any]) -> dict[str, dict[str, Any]]:
    if graph.get("schema_version") != 1:
        raise RailwayError("unsupported WORK_GRAPH.json schema_version")
    if graph.get("graph_mode") != "deep":
        raise RailwayError("ideal-base graph must use deep mode")
    artifact = graph.get("artifact_schema", {})
    if set(artifact.get("required", [])) != ARTIFACT_FIELDS:
        raise RailwayError(
            "artifact_schema.required does not match the deep handoff contract"
        )
    nodes = node_index(graph)
    validate_dag(nodes)

    roots = graph["root_nodes"]
    root_ids = {node["id"] for node in roots}
    expansions = graph.get("expansions")
    if not isinstance(expansions, dict) or set(expansions) != root_ids:
        raise RailwayError(
            "expansions must contain exactly one entry for every root node"
        )
    if len(roots) > 10:
        raise RailwayError("root graph exceeds the deep-gate review budget")
    flattened = [child for children in expansions.values() for child in children]
    if flattened != graph["all_nodes"]:
        raise RailwayError("all_nodes must exactly flatten expansions in root order")
    for parent, children in expansions.items():
        if len(children) > 10:
            raise RailwayError(
                f"{parent}: expansion exceeds the deep-gate review budget"
            )
        for child in children:
            if child.get("parent") != parent:
                raise RailwayError(
                    f"{child.get('id')}: parent does not match expansion {parent}"
                )
            for required in (
                "content",
                "kind",
                "class",
                "owned_paths",
                "acceptance_gates",
                "evidence",
                "review_model",
            ):
                if required not in child:
                    raise RailwayError(
                        f"{child['id']}: missing contract field {required}"
                    )
            if child["class"] == "gated" and not child.get("authorization"):
                raise RailwayError(
                    f"{child['id']}: gated node must name its authorization boundary"
                )
    coordinator_paths = graph.get("coordinator_owned_paths")
    if not isinstance(coordinator_paths, list) or not coordinator_paths:
        raise RailwayError(
            "coordinator_owned_paths must reserve durable authority files"
        )
    for node_id, node in nodes.items():
        overlap = set(node.get("owned_paths", [])) & set(coordinator_paths)
        if overlap:
            raise RailwayError(
                f"{node_id}: child ownership includes coordinator path(s): {sorted(overlap)}"
            )
    validate_ownership(nodes)

    coverage = graph.get("audit_coverage")
    expected_audit_ids = [f"A{index:02d}" for index in range(1, 26)]
    if (
        not isinstance(coverage, list)
        or [row.get("id") for row in coverage] != expected_audit_ids
    ):
        raise RailwayError("audit_coverage must contain ordered IDs A01 through A25")
    covered_nodes: set[str] = set()
    for row in coverage:
        references = row.get("nodes")
        if not isinstance(references, list) or not references:
            raise RailwayError(f"{row['id']}: audit coverage must cite graph nodes")
        for node_id in references:
            if node_id not in nodes:
                raise RailwayError(f"{row['id']}: unknown graph node {node_id}")
            if not node_id.startswith(("F", "G")):
                raise RailwayError(
                    f"{row['id']}: coverage may cite only F/G executable nodes"
                )
            covered_nodes.add(node_id)
    executable_nodes = {node_id for node_id in nodes if node_id.startswith(("F", "G"))}
    if covered_nodes != executable_nodes:
        missing = sorted(executable_nodes - covered_nodes)
        extra = sorted(covered_nodes - executable_nodes)
        raise RailwayError(f"audit coverage mismatch; missing={missing}, extra={extra}")
    return nodes


def evidence_path(value: str) -> Path:
    path = Path(value)
    return path if path.is_absolute() else REPO_ROOT / path


def validate_state(state: dict[str, Any], nodes: dict[str, dict[str, Any]]) -> None:
    if state.get("schema_version") != 1:
        raise RailwayError("unsupported STATE.json schema_version")
    records = state.get("nodes")
    if not isinstance(records, dict):
        raise RailwayError("STATE.json nodes must be an object")
    if set(records) != set(nodes):
        missing = sorted(set(nodes) - set(records))
        extra = sorted(set(records) - set(nodes))
        raise RailwayError(
            f"STATE.json node mismatch; missing={missing}, extra={extra}"
        )
    for node_id, record in records.items():
        if not isinstance(record, dict):
            raise RailwayError(f"{node_id}: state record must be an object")
        disposition = record.get("state")
        if disposition not in ALLOWED_STATES:
            raise RailwayError(f"{node_id}: invalid state {disposition!r}")
        if disposition in DEPENDENCY_COMPLETE:
            commit = record.get("commit")
            evidence = record.get("evidence")
            if not isinstance(commit, str) or not git_commit_reachable(commit):
                raise RailwayError(
                    f"{node_id}: completed state must cite a reachable commit"
                )
            if not isinstance(evidence, list) or not evidence:
                raise RailwayError(f"{node_id}: completed state must cite evidence")
            for item in evidence:
                if not isinstance(item, str) or not evidence_path(item).exists():
                    raise RailwayError(f"{node_id}: missing evidence path {item!r}")
        if (
            disposition == "authorization_blocked"
            and nodes[node_id].get("class") != "gated"
        ):
            raise RailwayError(
                f"{node_id}: only gated nodes may be authorization_blocked"
            )


def validate_repository() -> tuple[
    dict[str, Any], dict[str, Any], dict[str, dict[str, Any]]
]:
    graph = load_json(GRAPH_PATH)
    state = load_json(STATE_PATH)
    nodes = validate_graph(graph)
    validate_state(state, nodes)
    actual_hash = sha256(PROTECTED_PROMPT)
    if actual_hash != PROTECTED_PROMPT_SHA256:
        raise RailwayError(
            "protected orchestrator prompt hash changed: "
            f"expected {PROTECTED_PROMPT_SHA256}, got {actual_hash}"
        )
    validate_bootstrap_prompt()
    validate_markdown_links()
    return graph, state, nodes


def ready_nodes(
    graph: dict[str, Any], state: dict[str, Any], nodes: dict[str, dict[str, Any]]
) -> list[dict[str, Any]]:
    records = state["nodes"]
    root_ids = [node["id"] for node in graph["root_nodes"]]
    ready: list[dict[str, Any]] = []
    for root_id in root_ids:
        root_state = records[root_id]["state"]
        root = nodes[root_id]
        root_dependencies_complete = all(
            records[dependency]["state"] in DEPENDENCY_COMPLETE
            for dependency in root.get("depends_on", [])
        )
        if root_state == "pending" and root_dependencies_complete:
            ready.append({**root, "action": "seed_and_expand"})
            continue
        if root_state not in {"in_progress", "implemented", "verifying", "blocked"}:
            continue
        children = graph["expansions"][root_id]
        runnable_children = []
        for child in children:
            if records[child["id"]]["state"] != "pending":
                continue
            if all(
                records[dependency]["state"] in DEPENDENCY_COMPLETE
                for dependency in child.get("depends_on", [])
            ):
                runnable_children.append({**child, "action": "dispatch"})
        ready.extend(runnable_children)
        if children and all(
            records[child["id"]]["state"] in DEPENDENCY_COMPLETE for child in children
        ):
            ready.append({**root, "action": "synthesize"})
    return ready


def command_check(_: argparse.Namespace) -> int:
    graph, state, nodes = validate_repository()
    print(
        "ideal-base railway OK: "
        f"{len(graph['root_nodes'])} roots, {len(graph['all_nodes'])} child nodes, "
        f"{len(state['nodes'])} state records, protected hash intact"
    )
    return 0


def command_status(_: argparse.Namespace) -> int:
    graph, state, nodes = validate_repository()
    counts = Counter(record["state"] for record in state["nodes"].values())
    print(f"program: {state['program']} ({state['program_state']})")
    print(f"nodes: {len(nodes)}")
    for disposition in sorted(counts):
        print(f"  {disposition}: {counts[disposition]}")
    ready = ready_nodes(graph, state, nodes)
    print("runnable:")
    for node in ready:
        print(f"  {node['id']}: {node['action']} - {node['content']}")
    if not ready:
        print("  none")
    return 0


def command_next(args: argparse.Namespace) -> int:
    graph, state, nodes = validate_repository()
    ready = ready_nodes(graph, state, nodes)
    if args.json:
        print(json.dumps(ready, indent=2))
    else:
        for node in ready:
            print(f"{node['id']}\t{node['action']}\t{node['kind']}\t{node['content']}")
    return 0


def atomic_write_json(path: Path, value: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(
        "w", dir=path.parent, prefix=f".{path.name}.", delete=False
    ) as handle:
        temporary = Path(handle.name)
        json.dump(value, handle, indent=2)
        handle.write("\n")
        handle.flush()
        os.fsync(handle.fileno())
    try:
        os.replace(temporary, path)
        directory_fd = os.open(path.parent, os.O_DIRECTORY)
        try:
            os.fsync(directory_fd)
        finally:
            os.close(directory_fd)
    finally:
        temporary.unlink(missing_ok=True)


def command_checkpoint(args: argparse.Namespace) -> int:
    graph, state, nodes = validate_repository()
    if args.node not in nodes:
        raise RailwayError(f"unknown node: {args.node}")
    if args.state not in ALLOWED_STATES:
        raise RailwayError(f"invalid state: {args.state}")
    if (
        args.state == "authorization_blocked"
        and nodes[args.node].get("class") != "gated"
    ):
        raise RailwayError("only gated nodes may be authorization_blocked")
    try:
        timestamp = datetime.fromisoformat(args.updated_at.replace("Z", "+00:00"))
    except ValueError as exc:
        raise RailwayError("--updated-at must be a valid RFC3339 timestamp") from exc
    if timestamp.tzinfo is None:
        raise RailwayError("--updated-at must include a timezone")
    if args.state in DEPENDENCY_COMPLETE:
        if not args.commit or not git_commit_reachable(args.commit):
            raise RailwayError("completed checkpoint requires a reachable --commit")
        if not args.evidence:
            raise RailwayError(
                "completed checkpoint requires at least one --evidence path"
            )
        for item in args.evidence:
            if not evidence_path(item).exists():
                raise RailwayError(f"evidence path does not exist: {item}")
    lock_value = subprocess.check_output(
        ["git", "rev-parse", "--git-path", "jcode-ideal-base-state.lock"],
        cwd=REPO_ROOT,
        text=True,
    ).strip()
    lock_path = Path(lock_value)
    if not lock_path.is_absolute():
        lock_path = REPO_ROOT / lock_path
    lock_path.parent.mkdir(parents=True, exist_ok=True)
    with lock_path.open("a+") as lock:
        fcntl.flock(lock.fileno(), fcntl.LOCK_EX)
        latest = load_json(STATE_PATH)
        record = latest["nodes"][args.node]
        record.update(
            {
                "state": args.state,
                "commit": args.commit,
                "evidence": args.evidence or [],
                "summary": args.summary,
                "updated_at": args.updated_at,
            }
        )
        latest["last_checkpoint"] = {
            "node": args.node,
            "state": args.state,
            "commit": args.commit,
            "updated_at": args.updated_at,
            "summary": args.summary,
        }
        atomic_write_json(STATE_PATH, latest)
    validate_repository()
    print(f"checkpointed {args.node} -> {args.state}")
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    check = subparsers.add_parser(
        "check", help="validate graph, state, links, evidence, and protected hash"
    )
    check.set_defaults(handler=command_check)
    status = subparsers.add_parser(
        "status", help="summarize durable node state and runnable work"
    )
    status.set_defaults(handler=command_status)
    next_parser = subparsers.add_parser(
        "next", help="print currently runnable graph nodes"
    )
    next_parser.add_argument(
        "--json", action="store_true", help="emit task-graph-ready JSON"
    )
    next_parser.set_defaults(handler=command_next)
    checkpoint = subparsers.add_parser(
        "checkpoint", help="atomically update one durable node record"
    )
    checkpoint.add_argument("node")
    checkpoint.add_argument("--state", required=True, choices=sorted(ALLOWED_STATES))
    checkpoint.add_argument("--commit")
    checkpoint.add_argument("--evidence", action="append")
    checkpoint.add_argument("--summary", required=True)
    checkpoint.add_argument("--updated-at", required=True, help="RFC3339 timestamp")
    checkpoint.set_defaults(handler=command_checkpoint)
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        return args.handler(args)
    except RailwayError as exc:
        print(f"ideal-base railway error: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
