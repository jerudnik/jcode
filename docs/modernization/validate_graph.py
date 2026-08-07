#!/usr/bin/env python3
"""Validate and project the post-ideal-base modernization task graph.

This tool is intentionally read-only. The native swarm task graph owns live
execution state; Git commit trailers provide restart hints without recreating a
second scheduler or mutable railway ledger.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from collections import defaultdict, deque
from pathlib import Path
from typing import Any

GRAPH_PATH = Path(__file__).with_name("TASK_GRAPH.json")
ALLOWED_KINDS = {"explore", "implement", "verify", "fix", "synthesize"}
ALLOWED_PROFILES = {"context", "design", "implement", "verify", "synthesize"}
TRAILER_RE = re.compile(r"^Modernization-Node:\s*([A-Za-z0-9._-]+)\s*$", re.MULTILINE)
BARRIER_RE = re.compile(
    r"^Modernization-Barrier:\s*([A-Za-z0-9._-]+)\s*$", re.MULTILINE
)
REVERT_RE = re.compile(
    r"^Modernization-Node-Reverted:\s*([A-Za-z0-9._-]+)\s*$", re.MULTILINE
)
BARRIER_REVERT_RE = re.compile(
    r"^Modernization-Barrier-Reverted:\s*([A-Za-z0-9._-]+)\s*$", re.MULTILINE
)


class GraphError(RuntimeError):
    pass


def load_graph() -> dict[str, Any]:
    try:
        graph = json.loads(GRAPH_PATH.read_text())
    except (OSError, json.JSONDecodeError) as exc:
        raise GraphError(f"cannot load {GRAPH_PATH}: {exc}") from exc
    if not isinstance(graph, dict):
        raise GraphError("graph root must be an object")
    return graph


def node_index(graph: dict[str, Any]) -> dict[str, dict[str, Any]]:
    raw_nodes = graph.get("nodes")
    if not isinstance(raw_nodes, list) or not raw_nodes:
        raise GraphError("nodes must be a non-empty array")
    nodes: dict[str, dict[str, Any]] = {}
    for raw in raw_nodes:
        if not isinstance(raw, dict):
            raise GraphError("every node must be an object")
        node_id = raw.get("id")
        if not isinstance(node_id, str) or not node_id:
            raise GraphError("every node needs a non-empty string id")
        if node_id in nodes:
            raise GraphError(f"duplicate node id: {node_id}")
        nodes[node_id] = raw
    return nodes


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


def recursive_prefix(pattern: str) -> str | None:
    """Return the fixed directory prefix for a simple ``path/**`` glob."""
    if not pattern.endswith("/**"):
        return None
    prefix = pattern[:-3].rstrip("/")
    if not prefix or any(token in prefix for token in "*?["):
        return None
    return prefix


def path_contains(prefix: str, path: str) -> bool:
    return path == prefix or path.startswith(prefix + "/")


def paths_may_overlap(left: str, right: str) -> bool:
    """Conservatively detect exact and simple recursive ownership overlap."""
    left_glob = recursive_prefix(left)
    right_glob = recursive_prefix(right)
    left_exact = not any(token in left for token in "*?[")
    right_exact = not any(token in right for token in "*?[")
    if left_exact and right_exact:
        return left == right
    if left_glob and right_exact:
        return path_contains(left_glob, right)
    if right_glob and left_exact:
        return path_contains(right_glob, left)
    if left_glob and right_glob:
        return path_contains(left_glob, right_glob) or path_contains(
            right_glob, left_glob
        )
    return False


def validate(graph: dict[str, Any]) -> dict[str, Any]:
    errors: list[str] = []
    warnings: list[str] = []
    if graph.get("schema_version") != 1:
        errors.append("schema_version must be 1")
    if graph.get("graph_mode") not in {"deep", "light"}:
        errors.append("graph_mode must be deep or light")
    concurrency = graph.get("default_concurrency")
    maximum_concurrency = graph.get("maximum_concurrency")
    if concurrency != 6:
        errors.append("default_concurrency must remain 6")
    if maximum_concurrency != 8:
        errors.append("maximum_concurrency must remain 8")
    if isinstance(concurrency, int) and isinstance(maximum_concurrency, int):
        if concurrency > maximum_concurrency:
            errors.append("default_concurrency cannot exceed maximum_concurrency")

    try:
        nodes = node_index(graph)
    except GraphError as exc:
        return {"ok": False, "errors": [str(exc)], "warnings": []}

    if graph.get("declared_node_count") != len(nodes):
        errors.append(f"declared_node_count must equal {len(nodes)}")

    for node_id, node in nodes.items():
        if node.get("kind") not in ALLOWED_KINDS:
            errors.append(f"{node_id}: unsupported kind {node.get('kind')!r}")
        if node.get("worker_profile") not in ALLOWED_PROFILES:
            errors.append(
                f"{node_id}: unsupported worker_profile {node.get('worker_profile')!r}"
            )
        dependencies = node.get("depends_on")
        if not isinstance(dependencies, list) or not all(
            isinstance(dep, str) for dep in dependencies
        ):
            errors.append(f"{node_id}: depends_on must be an array of node ids")
            continue
        for dependency in dependencies:
            if dependency not in nodes:
                errors.append(f"{node_id}: unknown dependency {dependency}")
            if dependency == node_id:
                errors.append(f"{node_id}: self-dependency")
        priority = node.get("priority")
        if not isinstance(priority, int):
            errors.append(f"{node_id}: priority must be an integer")
        if not isinstance(node.get("content"), str) or not node["content"].strip():
            errors.append(f"{node_id}: content must be a non-empty string")
        if (
            not isinstance(node.get("acceptance"), list)
            or not node["acceptance"]
            or not all(
                isinstance(item, str) and item.strip() for item in node["acceptance"]
            )
        ):
            errors.append(f"{node_id}: acceptance must be a non-empty array")
        if (
            not isinstance(node.get("falsification"), str)
            or not node["falsification"].strip()
        ):
            errors.append(
                f"{node_id}: falsification must state what would disprove the approach"
            )
        paths = node.get("owned_paths", [])
        if not isinstance(paths, list) or not all(
            isinstance(path, str) and path for path in paths
        ):
            errors.append(
                f"{node_id}: owned_paths must be an array of non-empty strings"
            )
        mutexes = node.get("mutexes", [])
        if not isinstance(mutexes, list) or not all(
            isinstance(name, str) and name for name in mutexes
        ):
            errors.append(f"{node_id}: mutexes must be an array of non-empty strings")
        if node.get("external_write") and not node.get("requires_authorization"):
            errors.append(f"{node_id}: external_write nodes must require authorization")
        if node.get("requires_authorization") and not node.get("external_write"):
            errors.append(
                f"{node_id}: authorization is reserved for external_write nodes"
            )
        if node.get("read_only") and paths:
            warnings.append(f"{node_id}: read-only node declares owned paths")
        if "**" in paths and node_id != "I30":
            errors.append(f"{node_id}: only I30 may own the full repository with **")
        if node.get("expandable"):
            minimum = node.get("required_children_min")
            maximum = node.get("required_children_max")
            if (
                not isinstance(minimum, int)
                or not isinstance(maximum, int)
                or not 1 <= minimum <= maximum
            ):
                errors.append(
                    f"{node_id}: expandable nodes need valid child-count bounds"
                )

    if not errors:
        indegree = {node_id: 0 for node_id in nodes}
        dependents: dict[str, list[str]] = defaultdict(list)
        for node_id, node in nodes.items():
            for dependency in node.get("depends_on", []):
                indegree[node_id] += 1
                dependents[dependency].append(node_id)
        queue = deque(node_id for node_id, degree in indegree.items() if degree == 0)
        visited: list[str] = []
        while queue:
            current = queue.popleft()
            visited.append(current)
            for dependent in dependents[current]:
                indegree[dependent] -= 1
                if indegree[dependent] == 0:
                    queue.append(dependent)
        if len(visited) != len(nodes):
            errors.append("dependency graph contains a cycle")

    if not errors:
        by_mutex: dict[str, list[str]] = defaultdict(list)
        closures = {node_id: dependency_closure(nodes, node_id) for node_id in nodes}
        for node_id, node in nodes.items():
            for mutex in node.get("mutexes", []):
                by_mutex[mutex].append(node_id)
        for mutex, members in sorted(by_mutex.items()):
            for index, left in enumerate(members):
                for right in members[index + 1 :]:
                    ordered = left in closures[right] or right in closures[left]
                    if not ordered:
                        errors.append(
                            f"mutex {mutex!r} can overlap: {left} and {right} are not dependency-ordered"
                        )

        writers = [
            node_id for node_id, node in nodes.items() if not node.get("read_only")
        ]
        for index, left in enumerate(writers):
            for right in writers[index + 1 :]:
                ordered = left in closures[right] or right in closures[left]
                if ordered:
                    continue
                conflicts = [
                    (left_path, right_path)
                    for left_path in nodes[left].get("owned_paths", [])
                    for right_path in nodes[right].get("owned_paths", [])
                    if paths_may_overlap(left_path, right_path)
                ]
                if conflicts:
                    left_path, right_path = conflicts[0]
                    errors.append(
                        f"owned paths can overlap: {left} ({left_path}) and "
                        f"{right} ({right_path}) are not dependency-ordered"
                    )

    barriers = graph.get("barriers", [])
    if not isinstance(barriers, list):
        errors.append("barriers must be an array")
    else:
        if graph.get("declared_barrier_count") != len(barriers):
            errors.append(f"declared_barrier_count must equal {len(barriers)}")
        for barrier in barriers:
            if barrier not in nodes:
                errors.append(f"unknown barrier node: {barrier}")
            elif nodes[barrier].get("kind") != "synthesize":
                errors.append(f"barrier node {barrier} must have kind synthesize")

    configured_authorization = graph.get("defaults", {}).get(
        "operator_intervention_nodes", []
    )
    actual_authorization = sorted(
        node_id for node_id, node in nodes.items() if node.get("requires_authorization")
    )
    if not isinstance(configured_authorization, list):
        errors.append("defaults.operator_intervention_nodes must be an array")
    elif sorted(configured_authorization) != actual_authorization:
        errors.append(
            "defaults.operator_intervention_nodes must exactly match authorization nodes: "
            + ", ".join(actual_authorization)
        )

    return {
        "ok": not errors,
        "program": graph.get("program"),
        "node_count": len(nodes),
        "barrier_count": len(barriers) if isinstance(barriers, list) else 0,
        "errors": errors,
        "warnings": warnings,
    }


def topological_waves(
    graph: dict[str, Any], completed: set[str] | None = None
) -> list[list[str]]:
    nodes = node_index(graph)
    done = set(completed or ())
    unknown = done.difference(nodes)
    if unknown:
        raise GraphError(f"unknown completed node ids: {', '.join(sorted(unknown))}")
    remaining = set(nodes).difference(done)
    waves: list[list[str]] = []
    while remaining:
        ready = [
            node_id
            for node_id in remaining
            if set(nodes[node_id].get("depends_on", [])).issubset(done)
        ]
        if not ready:
            raise GraphError(
                "no runnable nodes remain; graph is cyclic or completion set is inconsistent"
            )
        ready.sort(key=lambda node_id: (-nodes[node_id]["priority"], node_id))
        waves.append(ready)
        done.update(ready)
        remaining.difference_update(ready)
    return waves


def wave_metrics(graph: dict[str, Any], waves: list[list[str]]) -> dict[str, Any]:
    node_count = sum(len(wave) for wave in waves)
    wave_count = len(waves)
    return {
        "wave_count": wave_count,
        "singleton_waves": sum(len(wave) == 1 for wave in waves),
        "max_width": max((len(wave) for wave in waves), default=0),
        "equal_duration_average_parallelism": (
            round(node_count / wave_count, 2) if wave_count else 0.0
        ),
        "configured_concurrency": graph.get("default_concurrency"),
    }


def git_trailers() -> tuple[set[str], set[str]]:
    try:
        messages = subprocess.check_output(
            ["git", "log", "--format=%B%x00"],
            cwd=GRAPH_PATH.parents[2],
            text=True,
            stderr=subprocess.DEVNULL,
        )
    except (OSError, subprocess.CalledProcessError) as exc:
        raise GraphError(f"cannot inspect Git completion trailers: {exc}") from exc
    node_state: dict[str, bool] = {}
    barrier_state: dict[str, bool] = {}
    for message in messages.split("\x00"):
        node_reverts = set(REVERT_RE.findall(message))
        node_completions = set(TRAILER_RE.findall(message))
        barrier_reverts = set(BARRIER_REVERT_RE.findall(message))
        barrier_completions = set(BARRIER_RE.findall(message))
        node_conflicts = node_reverts.intersection(node_completions)
        barrier_conflicts = barrier_reverts.intersection(barrier_completions)
        if node_conflicts or barrier_conflicts:
            conflicts = sorted(node_conflicts.union(barrier_conflicts))
            raise GraphError(
                "one commit contains completion and reversion trailers for: "
                + ", ".join(conflicts)
            )
        for node_id in node_reverts:
            node_state.setdefault(node_id, False)
        for node_id in node_completions:
            node_state.setdefault(node_id, True)
        for barrier_id in barrier_reverts:
            barrier_state.setdefault(barrier_id, False)
        for barrier_id in barrier_completions:
            barrier_state.setdefault(barrier_id, True)
    completed = {node_id for node_id, active in node_state.items() if active}
    barriers = {barrier_id for barrier_id, active in barrier_state.items() if active}
    return completed, barriers


def swarm_content(node: dict[str, Any], *, barrier: bool = False) -> str:
    sections = [node["content"]]
    if node.get("expandable"):
        sections.append(
            "EXECUTION SHAPE: COMPOSITE. Expand into the smallest sufficient set of "
            f"{node.get('required_children_min')} to {node.get('required_children_max')} "
            "independent child nodes with distinct outputs and disjoint ownership. Do not "
            "mutate before the expansion is accepted."
        )
    else:
        sections.append(
            "EXECUTION SHAPE: ATOMIC. This reviewed node is intended to be one bounded "
            "worker task. Do not expand it merely to consume the agent budget. Expand only "
            "if execution uncovers multiple independently verifiable concerns that cannot "
            "honestly close inside this node's scope."
        )
    acceptance = node.get("acceptance", [])
    if acceptance:
        sections.append("Acceptance:\n" + "\n".join(f"- {item}" for item in acceptance))
    sections.append(f"Falsification / stop condition:\n- {node['falsification']}")
    paths = node.get("owned_paths", [])
    if paths:
        path_text = "\n".join(f"- {path}" for path in paths)
    elif node.get("read_only"):
        path_text = "- none (read-only)"
    else:
        path_text = "- none (Git, integration, or external operation)"
    sections.append("Owned paths:\n" + path_text)
    mutexes = node.get("mutexes", [])
    sections.append(
        "Mutexes:\n" + ("\n".join(f"- {mutex}" for mutex in mutexes) or "- none")
    )
    sections.append(f"Worker profile hint: {node.get('worker_profile', 'default')}")
    if not node.get("read_only") and paths:
        sections.append(
            "SHARED-WORKTREE COMMIT RULE: stage only the owned paths listed above and commit "
            f"with trailer `Modernization-Node: {node['id']}`. Never use `git add -A`, "
            "`git add .`, or `git commit -a`. If the index, hook, or worktree contains an "
            "unresolvable concurrent conflict, report blocked so the coordinator can serialize "
            "or move the node to an isolated worktree."
        )
    if barrier:
        sections.append(
            f"BARRIER COMMIT RULE: after verifying all dependency artifacts, create a bounded "
            f"local commit carrying `Modernization-Barrier: {node['id']}` plus the completed "
            "leaf `Modernization-Node:` trailers not already recorded."
        )
    if node.get("requires_authorization"):
        sections.append(
            "MANUAL AUTHORIZATION GATE: report blocked until the coordinator obtains fresh, "
            "explicit user authorization in the originating session. Do not perform the external write first."
        )
    return "\n\n".join(sections)


def swarm_node(
    node: dict[str, Any], *, dependencies: bool = True, barrier: bool = False
) -> dict[str, Any]:
    return {
        "id": node["id"],
        "content": swarm_content(node, barrier=barrier),
        "kind": node["kind"],
        "depends_on": node.get("depends_on", []) if dependencies else [],
        "priority": node["priority"],
    }


def checked_completed(
    nodes: dict[str, dict[str, Any]], explicit: list[str]
) -> set[str]:
    completed, _ = git_trailers()
    unknown = set(explicit).difference(nodes)
    if unknown:
        raise GraphError(f"unknown completed node ids: {', '.join(sorted(unknown))}")
    completed.update(explicit)
    completed.intersection_update(nodes)
    return completed


def command_validate(graph: dict[str, Any], as_json: bool) -> int:
    report = validate(graph)
    if as_json:
        print(json.dumps(report, indent=2, sort_keys=True))
    else:
        verdict = "PASS" if report["ok"] else "FAIL"
        print(
            f"{verdict}: {report['program']} ({report['node_count']} nodes, {report['barrier_count']} barriers)"
        )
        for warning in report["warnings"]:
            print(f"warning: {warning}")
        for error in report["errors"]:
            print(f"error: {error}", file=sys.stderr)
    return 0 if report["ok"] else 1


def command_waves(graph: dict[str, Any], as_json: bool) -> int:
    waves = topological_waves(graph)
    metrics = wave_metrics(graph, waves)
    if as_json:
        print(json.dumps({"metrics": metrics, "waves": waves}, indent=2))
    else:
        for index, wave in enumerate(waves, start=1):
            print(f"wave {index:02d} width={len(wave)}: {' '.join(wave)}")
        print(
            "summary: "
            f"waves={metrics['wave_count']} "
            f"singleton={metrics['singleton_waves']} "
            f"max_width={metrics['max_width']} "
            f"equal_duration_avg={metrics['equal_duration_average_parallelism']:.2f} "
            f"configured_concurrency={metrics['configured_concurrency']}"
        )
    return 0


def command_seed(graph: dict[str, Any]) -> int:
    nodes = node_index(graph)
    barriers = set(graph["barriers"])
    payload = {
        "task_graph": {
            "mode": graph["graph_mode"],
            "replace_existing": True,
            "nodes": [
                swarm_node(node, barrier=node["id"] in barriers)
                for node in nodes.values()
            ],
        },
        "run_plan": {
            "mode": graph["graph_mode"],
            "concurrency_limit": graph["default_concurrency"],
            "background": True,
            "retain_agents": False,
        },
        "authorization_nodes": graph["defaults"]["operator_intervention_nodes"],
    }
    print(json.dumps(payload, indent=2))
    return 0


def command_status(graph: dict[str, Any], as_json: bool, explicit: list[str]) -> int:
    nodes = node_index(graph)
    completed = checked_completed(nodes, explicit)
    _, barriers = git_trailers()
    barriers.intersection_update(graph.get("barriers", []))
    ready = [
        node_id
        for node_id, node in nodes.items()
        if node_id not in completed
        and set(node.get("depends_on", [])).issubset(completed)
    ]
    ready.sort(key=lambda node_id: (-nodes[node_id]["priority"], node_id))
    report = {
        "completed": sorted(completed),
        "barriers": sorted(barriers),
        "ready": ready,
        "remaining": len(nodes) - len(completed),
    }
    if as_json:
        print(json.dumps(report, indent=2))
    else:
        print(f"completed={len(completed)} remaining={report['remaining']}")
        print(f"barriers={','.join(sorted(barriers)) or '-'}")
        print(f"ready={' '.join(ready) or '-'}")
    return 0


def command_next(graph: dict[str, Any], explicit: list[str]) -> int:
    nodes = node_index(graph)
    completed = checked_completed(nodes, explicit)
    ready = [
        node
        for node_id, node in nodes.items()
        if node_id not in completed
        and set(node.get("depends_on", [])).issubset(completed)
    ]
    ready.sort(key=lambda node: (-node["priority"], node["id"]))
    authorization_ready = [node for node in ready if node.get("requires_authorization")]
    ordinary_ready = [node for node in ready if not node.get("requires_authorization")]
    if authorization_ready and ordinary_ready:
        ready = ordinary_ready
    barriers = set(graph["barriers"])
    payload = {
        "task_graph": {
            "mode": graph["graph_mode"],
            "replace_existing": True,
            "nodes": [
                swarm_node(
                    node,
                    dependencies=False,
                    barrier=node["id"] in barriers,
                )
                for node in ready
            ],
        },
        "run_plan": {
            "mode": graph["graph_mode"],
            "concurrency_limit": min(graph["default_concurrency"], max(1, len(ready))),
            "background": True,
            "retain_agents": False,
        },
        "authorization_nodes": [
            node["id"] for node in ready if node.get("requires_authorization")
        ],
    }
    print(json.dumps(payload, indent=2))
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    validate_parser = subparsers.add_parser("validate")
    validate_parser.add_argument("--json", action="store_true")
    waves_parser = subparsers.add_parser("waves")
    waves_parser.add_argument("--json", action="store_true")
    subparsers.add_parser("seed")
    status_parser = subparsers.add_parser("status")
    status_parser.add_argument("--json", action="store_true")
    status_parser.add_argument("--completed", action="append", default=[])
    next_parser = subparsers.add_parser("next")
    next_parser.add_argument("--completed", action="append", default=[])
    args = parser.parse_args()

    try:
        graph = load_graph()
        report = validate(graph)
        if not report["ok"] and args.command != "validate":
            raise GraphError("graph validation failed; run validate for details")
        if args.command == "validate":
            return command_validate(graph, args.json)
        if args.command == "waves":
            return command_waves(graph, args.json)
        if args.command == "seed":
            return command_seed(graph)
        if args.command == "status":
            return command_status(graph, args.json, args.completed)
        if args.command == "next":
            return command_next(graph, args.completed)
    except GraphError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
