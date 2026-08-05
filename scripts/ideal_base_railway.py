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
FULL_SHA = re.compile(r"^[0-9a-f]{40}$")
MARKDOWN_LINK = re.compile(r"\[[^\]]*\]\(([^)]+)\)")
# Schema-v2 default publication ref (R07 design §8). CI passes
# refs/remotes/origin/main explicitly (actions/checkout names its remote
# `origin`); local clones may name the canonical remote differently (the
# canonical checkout uses `github`), so the default tries each known remote
# name and keeps the first that resolves. Explicit --published-ref always
# wins and is never rewritten.
_PUBLISHED_REF_CANDIDATES = (
    "refs/remotes/origin/main",
    "refs/remotes/github/main",
)


def _default_published_ref() -> str:
    for candidate in _PUBLISHED_REF_CANDIDATES:
        proc = subprocess.run(
            ["git", "rev-parse", "--verify", "--quiet", candidate],
            cwd=REPO_ROOT,
            capture_output=True,
            text=True,
            check=False,
        )
        if proc.returncode == 0:
            return candidate
    # Fall through to the historical default so the error message still names
    # a concrete ref instead of an empty string.
    return _PUBLISHED_REF_CANDIDATES[0]


DEFAULT_PUBLISHED_REF = _default_published_ref()
# Post-distribution amendment (D030): W4 carries eleven children after the
# R06 sticky-server and F30 distribution-verification nodes were added, so the
# per-wave deep-gate review budget moved from 10 to 12.
MAX_EXPANSION_CHILDREN = 12
# Audit items A26 (documentation truth) and A27 (Nix-only distribution
# verification) were added by the post-distribution amendment (D030).
AUDIT_ID_COUNT = 27
# Coverage may cite D (documentation), F, and G executable nodes.
AUDIT_COVERAGE_PREFIXES = ("D", "F", "G")
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


MISSING_REVIEWED_OBJECTS_ENV = "JCODE_RAILWAY_ALLOW_MISSING_REVIEWED_OBJECTS"


def allow_missing_reviewed_objects() -> bool:
    """Explicit opt-in lenient mode for clones that cannot hold the reviewed
    objects (CI). Off by default; see the step-1 comment in
    _validate_state_v2."""
    return os.environ.get(MISSING_REVIEWED_OBJECTS_ENV) == "1"


def git_commit_object_exists(commit: str, *, cwd: Path = REPO_ROOT) -> bool:
    """True iff ``commit`` names a commit object that exists locally.

    This is object existence, not reachability from any particular ref: an
    unreferenced local object (e.g. a stash, a reflog-only commit, or a
    fetched-then-abandoned branch tip) satisfies it. Schema v2 (design §8)
    requires this check never be reported as proving publication; use
    ``git_commit_is_ancestor`` for that.
    """
    result = subprocess.run(
        ["git", "cat-file", "-e", f"{commit}^{{commit}}"],
        cwd=cwd,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        check=False,
    )
    return result.returncode == 0


def git_ref_resolves(ref: str, *, cwd: Path = REPO_ROOT) -> bool:
    result = subprocess.run(
        ["git", "rev-parse", "--verify", "--quiet", f"{ref}^{{commit}}"],
        cwd=cwd,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        check=False,
    )
    return result.returncode == 0


def git_repository_is_shallow(*, cwd: Path = REPO_ROOT) -> bool:
    output = subprocess.check_output(
        ["git", "rev-parse", "--is-shallow-repository"],
        cwd=cwd,
        text=True,
    ).strip()
    return output == "true"


def git_commit_is_ancestor(commit: str, ref: str, *, cwd: Path = REPO_ROOT) -> bool:
    """True iff ``commit`` is an ancestor of (or equal to) ``ref``.

    This is the only check schema v2 treats as proof of publication. Callers
    must have already confirmed the repository is non-shallow and that
    ``ref`` resolves; a shallow clone or an unresolved ref can make ancestry
    silently false-negative rather than raising.
    """
    result = subprocess.run(
        ["git", "merge-base", "--is-ancestor", commit, ref],
        cwd=cwd,
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
        if len(children) > MAX_EXPANSION_CHILDREN:
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
    expected_audit_ids = [f"A{index:02d}" for index in range(1, AUDIT_ID_COUNT + 1)]
    if (
        not isinstance(coverage, list)
        or [row.get("id") for row in coverage] != expected_audit_ids
    ):
        raise RailwayError(
            f"audit_coverage must contain ordered IDs A01 through A{AUDIT_ID_COUNT:02d}"
        )
    covered_nodes: set[str] = set()
    for row in coverage:
        references = row.get("nodes")
        if not isinstance(references, list) or not references:
            raise RailwayError(f"{row['id']}: audit coverage must cite graph nodes")
        for node_id in references:
            if node_id not in nodes:
                raise RailwayError(f"{row['id']}: unknown graph node {node_id}")
            if not node_id.startswith(AUDIT_COVERAGE_PREFIXES):
                raise RailwayError(
                    f"{row['id']}: coverage may cite only D/F/G executable nodes"
                )
            covered_nodes.add(node_id)
    executable_nodes = {node_id for node_id in nodes if node_id.startswith(("F", "G"))}
    missing = sorted(executable_nodes - covered_nodes)
    extra = sorted(
        node_id
        for node_id in covered_nodes - executable_nodes
        if not node_id.startswith("D")
    )
    if missing or extra:
        raise RailwayError(f"audit coverage mismatch; missing={missing}, extra={extra}")
    return nodes


def evidence_path(value: str) -> Path:
    path = Path(value)
    return path if path.is_absolute() else REPO_ROOT / path


def validate_state(
    state: dict[str, Any],
    nodes: dict[str, dict[str, Any]],
    *,
    published_ref: str = DEFAULT_PUBLISHED_REF,
) -> None:
    schema_version = state.get("schema_version")
    if schema_version == 1:
        _validate_state_v1(state, nodes)
        return
    if schema_version == 2:
        _validate_state_v2(state, nodes, published_ref=published_ref)
        return
    raise RailwayError("unsupported STATE.json schema_version")


def _validate_state_v1(state: dict[str, Any], nodes: dict[str, dict[str, Any]]) -> None:
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
            if not isinstance(commit, str) or not git_commit_object_exists(commit):
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


def _validate_state_v2(
    state: dict[str, Any],
    nodes: dict[str, dict[str, Any]],
    *,
    published_ref: str,
) -> None:
    records = state.get("nodes")
    if not isinstance(records, dict):
        raise RailwayError("STATE.json nodes must be an object")
    if set(records) != set(nodes):
        missing = sorted(set(nodes) - set(records))
        extra = sorted(set(records) - set(nodes))
        raise RailwayError(
            f"STATE.json node mismatch; missing={missing}, extra={extra}"
        )
    # Steps 3-4 of the design §8 sequence are preconditions of the whole
    # schema-v2 validator, not of any one record: an unresolved published ref
    # or a shallow clone makes every ancestry check below meaningless, so
    # fail once, up front, rather than once per accepted node. There is
    # deliberately no allow_shallow escape hatch (design §8).
    if not git_ref_resolves(published_ref):
        raise RailwayError(f"published ref does not resolve: {published_ref!r}")
    if git_repository_is_shallow():
        raise RailwayError(
            "repository is shallow; schema-v2 ancestry checks require full history"
        )
    for node_id, record in records.items():
        if not isinstance(record, dict):
            raise RailwayError(f"{node_id}: state record must be an object")
        disposition = record.get("state")
        if disposition not in ALLOWED_STATES:
            raise RailwayError(f"{node_id}: invalid state {disposition!r}")
        reviewed_commit = record.get("reviewed_commit")
        published_commit = record.get("published_commit")
        if disposition in DEPENDENCY_COMPLETE:
            evidence = record.get("evidence")
            for label, commit in (
                ("reviewed_commit", reviewed_commit),
                ("published_commit", published_commit),
            ):
                if not isinstance(commit, str) or not FULL_SHA.match(commit):
                    raise RailwayError(
                        f"{node_id}: completed state must cite a 40-hex {label}"
                    )
            # Step 1: reviewed object existence. This is existence, not
            # reachability from any ref, and must never be reported as proof
            # of publication (design §8).
            #
            # CI clones of jerudnik/jcode cannot satisfy this check: the
            # reviewed objects are not ancestors of main; they live in local
            # object stores and, after R07 barrier 1, in the private
            # recovery-archive repo. When the explicitly named lenient mode
            # is enabled (the fork-ci governance-contract job sets it), a
            # missing reviewed object degrades to a NOTE instead of an
            # error. Anti-fabrication is preserved because (a) every other
            # check stays strict, (b) local coordinator validation runs
            # without the lenient mode, and (c) barrier 1's fresh-fetch
            # verification proved the reviewed objects exist at these exact
            # SHAs on the archive remote.
            if not git_commit_object_exists(reviewed_commit):
                if allow_missing_reviewed_objects():
                    print(
                        f"NOTE: {node_id}: reviewed_commit object not present "
                        f"in this clone (allowed by "
                        f"{MISSING_REVIEWED_OBJECTS_ENV}): {reviewed_commit}"
                    )
                else:
                    raise RailwayError(
                        f"{node_id}: reviewed_commit object does not exist: "
                        f"{reviewed_commit}"
                    )
            # Step 2: published object existence.
            if not git_commit_object_exists(published_commit):
                raise RailwayError(
                    f"{node_id}: published_commit object does not exist: "
                    f"{published_commit}"
                )
            # Step 5: published_commit must actually be on the published ref.
            # This, not step 2's bare existence check, is what proves the
            # node was published rather than merely reviewed.
            if not git_commit_is_ancestor(published_commit, published_ref):
                raise RailwayError(
                    f"{node_id}: published_commit {published_commit} is not an "
                    f"ancestor of {published_ref}"
                )
            if not isinstance(evidence, list) or not evidence:
                raise RailwayError(f"{node_id}: completed state must cite evidence")
            for item in evidence:
                if not isinstance(item, str) or not evidence_path(item).exists():
                    raise RailwayError(f"{node_id}: missing evidence path {item!r}")
        else:
            # "Every record has both keys. Pending/in-progress records use
            # null for both." (design §8). A non-terminal node cannot carry a
            # commit identity: that would let a node be treated as published
            # by ancestry-scanning tools while the railway itself still
            # considers it incomplete.
            if reviewed_commit is not None or published_commit is not None:
                raise RailwayError(
                    f"{node_id}: {disposition!r} state must use null for both "
                    "reviewed_commit and published_commit"
                )
        if (
            disposition == "authorization_blocked"
            and nodes[node_id].get("class") != "gated"
        ):
            raise RailwayError(
                f"{node_id}: only gated nodes may be authorization_blocked"
            )


def expansion_violations(graph: dict[str, Any], state: dict[str, Any]) -> dict[str, str]:
    """Roots whose recorded state contradicts their children, by root id.

    ``validate_state`` only checks each record in isolation, so a root could sit
    at ``pending`` while every one of its children was ``accepted`` and ``check``
    would still report OK. That is not a harmless lag: ``ready_nodes`` treats a
    pending root as unexpanded and re-emits ``seed_and_expand``, which invites
    re-doing finished work. W3 reached exactly that state (nine accepted
    children under a pending root) with the gate green, which is why this
    exists.
    """
    records = state["nodes"]
    violations: dict[str, str] = {}
    for root_id, children in graph["expansions"].items():
        if not children:
            continue
        root_state = records[root_id]["state"]
        child_states = [records[child["id"]]["state"] for child in children]
        if root_state == "pending" and any(
            child != "pending" for child in child_states
        ):
            started = sorted(
                child["id"]
                for child in children
                if records[child["id"]]["state"] != "pending"
            )
            violations[root_id] = (
                f"{root_id}: root is pending but children have progressed "
                f"({', '.join(started)}); a pending root is re-seeded as "
                "unexpanded work"
            )
        elif root_state not in DEPENDENCY_COMPLETE and all(
            child in DEPENDENCY_COMPLETE for child in child_states
        ):
            violations[root_id] = (
                f"{root_id}: every child is complete but the root is "
                f"{root_state!r}; close the wave or record why it cannot close"
            )
    return violations


def validate_expansion_consistency(
    graph: dict[str, Any], state: dict[str, Any]
) -> None:
    violations = expansion_violations(graph, state)
    if violations:
        raise RailwayError("; ".join(violations[key] for key in sorted(violations)))


def validate_repository(
    *, published_ref: str = DEFAULT_PUBLISHED_REF
) -> tuple[dict[str, Any], dict[str, Any], dict[str, dict[str, Any]]]:
    graph = load_json(GRAPH_PATH)
    state = load_json(STATE_PATH)
    nodes = validate_graph(graph)
    validate_state(state, nodes, published_ref=published_ref)
    validate_expansion_consistency(graph, state)
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
    """Every node the railway can act on right now, with the action to take.

    A child is offered while it is ``pending`` (dispatch the work) and again
    while it is ``implemented`` or ``verifying`` (run the gates). The lifecycle
    in EXECUTION_PROTOCOL section 5 is
    ``pending -> in_progress -> implemented -> verifying -> accepted``, and
    "implementation is not acceptance": a node at ``implemented`` has outstanding
    work, namely the verification its own gates demand. Offering only ``pending``
    children made those two states invisible, so a wave whose remaining children
    were all awaiting verification projected NOTHING and the railway silently
    reported it had no next action. That is how D01-FIX-2 and G02, both
    correctly checkpointed, emptied the projection and failed the
    always-offer-some-action test.

    ``in_progress`` is deliberately NOT offered: it means an owner is already
    executing that node, and re-offering it invites duplicate work. ``blocked``
    is likewise withheld because it names a missing input by definition, so
    dispatching it would be dispatching something known to be unable to proceed.
    """
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
            child_state = records[child["id"]]["state"]
            if child_state in {"implemented", "verifying"}:
                # Awaiting its gates, not awaiting a dependency: a node only
                # reaches these states after its work exists, so dependency
                # completeness is not re-checked here.
                runnable_children.append({**child, "action": "verify"})
                continue
            if child_state != "pending":
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


def command_check(args: argparse.Namespace) -> int:
    graph, state, nodes = validate_repository(published_ref=args.published_ref)
    print(
        "ideal-base railway OK: "
        f"{len(graph['root_nodes'])} roots, {len(graph['all_nodes'])} child nodes, "
        f"{len(state['nodes'])} state records, protected hash intact"
    )
    return 0


def command_status(args: argparse.Namespace) -> int:
    graph, state, nodes = validate_repository(published_ref=args.published_ref)
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
    graph, state, nodes = validate_repository(published_ref=args.published_ref)
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
    # Deliberately not `validate_repository()`: that also enforces expansion
    # consistency, and a checkpoint is precisely how an inconsistent root is
    # repaired. Gating entry on it would deadlock the only tool that can fix
    # the problem. Consistency is enforced on the *resulting* state below, so
    # a checkpoint may repair an inconsistency but never introduce one.
    graph = load_json(GRAPH_PATH)
    state = load_json(STATE_PATH)
    nodes = validate_graph(graph)
    validate_state(state, nodes, published_ref=args.published_ref)
    schema_version = state.get("schema_version")
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

    if schema_version == 2:
        # Design §8: "The ambiguous `--commit` option is removed rather than
        # guessed." A schema-v2 checkpoint names the reviewed identity and the
        # merge/main-ancestral identity separately; there is no single commit
        # that could stand in for both without silently picking one.
        if args.commit is not None:
            raise RailwayError(
                "--commit is not valid against a schema-v2 STATE.json; use "
                "--reviewed-commit and --published-commit"
            )
        record_update = _build_checkpoint_record_v2(args, nodes)
    elif schema_version == 1:
        if args.reviewed_commit is not None or args.published_commit is not None:
            raise RailwayError(
                "--reviewed-commit/--published-commit are not valid against a "
                "schema-v1 STATE.json; use --commit"
            )
        record_update = _build_checkpoint_record_v1(args)
    else:
        raise RailwayError("unsupported STATE.json schema_version")

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
        record.update(record_update)
        if schema_version == 2:
            latest["last_checkpoint"] = {
                "node": args.node,
                "state": args.state,
                "reviewed_commit": record_update["reviewed_commit"],
                "published_commit": record_update["published_commit"],
                "updated_at": args.updated_at,
                "summary": args.summary,
            }
        else:
            latest["last_checkpoint"] = {
                "node": args.node,
                "state": args.state,
                "commit": record_update["commit"],
                "updated_at": args.updated_at,
                "summary": args.summary,
            }
        # Validate the prospective state before it reaches disk. Writing first
        # and validating after would leave a rejected state persisted, which is
        # worse than the inconsistency being prevented.
        #
        # Consistency is judged as a delta, not an absolute: a checkpoint must
        # not introduce or worsen a violation, but it must stay able to repair
        # one. Demanding a globally clean result would deadlock repair whenever
        # two roots drifted at once, since neither could be fixed first.
        validate_state(latest, nodes, published_ref=args.published_ref)
        before = expansion_violations(graph, state)
        after = expansion_violations(graph, latest)
        introduced = {
            root: message
            for root, message in after.items()
            if before.get(root) != message
        }
        if introduced:
            raise RailwayError(
                "; ".join(introduced[key] for key in sorted(introduced))
            )
        atomic_write_json(STATE_PATH, latest)
    print(f"checkpointed {args.node} -> {args.state}")
    remaining = expansion_violations(graph, latest)
    if remaining:
        # Surface, do not fail: the checkpoint was legitimate and is written.
        # These are pre-existing inconsistencies that this checkpoint did not
        # cause, and each needs its own repairing checkpoint.
        for key in sorted(remaining):
            print(f"warning: {remaining[key]}")
    return 0


def _build_checkpoint_record_v1(args: argparse.Namespace) -> dict[str, Any]:
    if args.state in DEPENDENCY_COMPLETE:
        if not args.commit or not git_commit_object_exists(args.commit):
            raise RailwayError("completed checkpoint requires a reachable --commit")
        if not args.evidence:
            raise RailwayError(
                "completed checkpoint requires at least one --evidence path"
            )
        for item in args.evidence:
            if not evidence_path(item).exists():
                raise RailwayError(f"evidence path does not exist: {item}")
    return {
        "state": args.state,
        "commit": args.commit,
        "evidence": args.evidence or [],
        "summary": args.summary,
        "updated_at": args.updated_at,
    }


def _build_checkpoint_record_v2(
    args: argparse.Namespace, nodes: dict[str, dict[str, Any]]
) -> dict[str, Any]:
    reviewed_commit = args.reviewed_commit
    published_commit = args.published_commit
    if args.state in DEPENDENCY_COMPLETE:
        for label, commit in (
            ("--reviewed-commit", reviewed_commit),
            ("--published-commit", published_commit),
        ):
            if not commit or not FULL_SHA.match(commit):
                raise RailwayError(
                    f"completed checkpoint requires a 40-hex {label}"
                )
        if not git_commit_object_exists(reviewed_commit):
            raise RailwayError(
                f"reviewed_commit object does not exist: {reviewed_commit}"
            )
        if not git_commit_object_exists(published_commit):
            raise RailwayError(
                f"published_commit object does not exist: {published_commit}"
            )
        if not git_commit_is_ancestor(published_commit, args.published_ref):
            raise RailwayError(
                f"published_commit {published_commit} is not an ancestor of "
                f"{args.published_ref}"
            )
        if not args.evidence:
            raise RailwayError(
                "completed checkpoint requires at least one --evidence path"
            )
        for item in args.evidence:
            if not evidence_path(item).exists():
                raise RailwayError(f"evidence path does not exist: {item}")
    else:
        # "Pending/in-progress records use null for both." (design §8).
        if reviewed_commit is not None or published_commit is not None:
            raise RailwayError(
                f"{args.state!r} checkpoint must leave reviewed_commit and "
                "published_commit null"
            )
    return {
        "state": args.state,
        "reviewed_commit": reviewed_commit,
        "published_commit": published_commit,
        "evidence": args.evidence or [],
        "summary": args.summary,
        "updated_at": args.updated_at,
    }


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    published_ref_help = (
        "ref that published_commit values must be an ancestor of "
        f"(schema v2 only; default {DEFAULT_PUBLISHED_REF!r}, CI passes "
        "refs/remotes/origin/main explicitly)"
    )
    check = subparsers.add_parser(
        "check", help="validate graph, state, links, evidence, and protected hash"
    )
    check.add_argument("--published-ref", default=DEFAULT_PUBLISHED_REF, help=published_ref_help)
    check.set_defaults(handler=command_check)
    status = subparsers.add_parser(
        "status", help="summarize durable node state and runnable work"
    )
    status.add_argument("--published-ref", default=DEFAULT_PUBLISHED_REF, help=published_ref_help)
    status.set_defaults(handler=command_status)
    next_parser = subparsers.add_parser(
        "next", help="print currently runnable graph nodes"
    )
    next_parser.add_argument(
        "--json", action="store_true", help="emit task-graph-ready JSON"
    )
    next_parser.add_argument(
        "--published-ref", default=DEFAULT_PUBLISHED_REF, help=published_ref_help
    )
    next_parser.set_defaults(handler=command_next)
    checkpoint = subparsers.add_parser(
        "checkpoint", help="atomically update one durable node record"
    )
    checkpoint.add_argument("node")
    checkpoint.add_argument("--state", required=True, choices=sorted(ALLOWED_STATES))
    checkpoint.add_argument(
        "--commit", help="schema v1 only; removed for schema v2 (design §8)"
    )
    checkpoint.add_argument(
        "--reviewed-commit", help="schema v2 only: the reviewed topic-branch identity"
    )
    checkpoint.add_argument(
        "--published-commit",
        help="schema v2 only: the merge/main-ancestral identity",
    )
    checkpoint.add_argument(
        "--published-ref", default=DEFAULT_PUBLISHED_REF, help=published_ref_help
    )
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
