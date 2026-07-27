#!/usr/bin/env python3
"""Tests for the ideal-base execution railway validator."""

from __future__ import annotations

import importlib.util
import json
import subprocess
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[1] / "scripts/ideal_base_railway.py"
SPEC = importlib.util.spec_from_file_location("ideal_base_railway", SCRIPT)
assert SPEC and SPEC.loader
railway = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(railway)


class IdealBaseRailwayTests(unittest.TestCase):
    def test_repository_control_plane_is_valid(self) -> None:
        # Counts are deliberately not asserted. This graph expands as nodes are
        # accepted (`seed_and_expand` adds children), so pinning totals froze a
        # moving number: the assertions said 38 nodes while the graph had grown
        # to 46, and failed on the growth they were supposed to permit. What
        # must hold is structural, so that is what is checked.
        graph, state, nodes = railway.validate_repository()
        self.assertTrue(graph["root_nodes"])
        self.assertTrue(graph["all_nodes"])
        self.assertLessEqual(len(graph["all_nodes"]), len(nodes))
        coordinator_paths = set(graph["coordinator_owned_paths"])
        self.assertTrue(coordinator_paths)
        self.assertTrue(
            all(
                not (set(node.get("owned_paths", [])) & coordinator_paths)
                for node in graph["all_nodes"]
            )
        )
        self.assertEqual(set(state["nodes"]), set(nodes))

    def test_runnable_projection_offers_only_genuinely_dispatchable_work(self) -> None:
        """Every runnable node must be pending with its dependencies complete.

        This deliberately does not assert a node count. It previously demanded
        exactly one ("the railway is sequential by construction"), which was
        never true: `validate_ownership` serializes only nodes with overlapping
        owned paths, so disjoint work is meant to be dispatchable in parallel.
        The assertion held by accident while waves happened to have one open
        node, then broke the moment W4 opened with five disjoint children.
        Assert the invariant that must always hold instead of a number that
        the design intends to move.
        """
        graph, state, nodes = railway.validate_repository()
        ready = railway.ready_nodes(graph, state, nodes)
        self.assertTrue(ready, "railway must always offer some next action")
        for node in ready:
            record = state["nodes"].get(node["id"], {})
            self.assertEqual(
                record.get("state"),
                "pending",
                f"{node['id']} is offered as runnable but is {record.get('state')!r}",
            )
            for dependency in node.get("depends_on", []):
                self.assertIn(
                    state["nodes"].get(dependency, {}).get("state"),
                    railway.DEPENDENCY_COMPLETE,
                    f"{node['id']} is runnable but depends on incomplete {dependency}",
                )
        self.assertEqual(
            len({node["id"] for node in ready}),
            len(ready),
            "the projection must not repeat a node",
        )

    def test_bootstrap_prompt_covers_the_full_execution_protocol(self) -> None:
        prompt = railway.validate_bootstrap_prompt()
        self.assertIn('mode: "deep"', prompt)
        self.assertIn("After each accepted node:", prompt)
        self.assertIn("Continue until every mandatory deterministic node", prompt)
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "bootstrap.md"
            path.write_text("````markdown\nDo not push.\n````\nOutside prompt.\n")
            with self.assertRaisesRegex(railway.RailwayError, "close at end of file"):
                railway.validate_bootstrap_prompt(path)

    def test_cycle_is_rejected(self) -> None:
        nodes = {
            "a": {"id": "a", "depends_on": ["b"]},
            "b": {"id": "b", "depends_on": ["a"]},
        }
        with self.assertRaisesRegex(railway.RailwayError, "dependency cycle"):
            railway.validate_dag(nodes)

    def test_unserialized_exact_path_overlap_is_rejected(self) -> None:
        nodes = {
            "W0": {"id": "W0", "depends_on": []},
            "a": {"id": "a", "parent": "W0", "depends_on": [], "owned_paths": ["same"]},
            "b": {"id": "b", "parent": "W0", "depends_on": [], "owned_paths": ["same"]},
        }
        with self.assertRaisesRegex(
            railway.RailwayError, "unserialized ownership overlap"
        ):
            railway.validate_ownership(nodes)
        nodes["b"]["depends_on"] = ["a"]
        railway.validate_ownership(nodes)

    def test_unserialized_glob_subsumption_is_rejected(self) -> None:
        nodes = {
            "W0": {"id": "W0", "depends_on": []},
            "a": {
                "id": "a",
                "parent": "W0",
                "depends_on": [],
                "owned_paths": ["src/server/**"],
            },
            "b": {
                "id": "b",
                "parent": "W0",
                "depends_on": [],
                "owned_paths": ["src/server/lifecycle.rs"],
            },
        }
        with self.assertRaisesRegex(
            railway.RailwayError, "unserialized ownership overlap"
        ):
            railway.validate_ownership(nodes)
        nodes["b"]["depends_on"] = ["a"]
        railway.validate_ownership(nodes)

    def test_completed_state_requires_reachable_commit_and_evidence(self) -> None:
        graph, state, nodes = railway.validate_repository()
        copied = json.loads(json.dumps(state))
        head = subprocess.check_output(
            ["git", "rev-parse", "HEAD"], cwd=railway.REPO_ROOT, text=True
        ).strip()
        copied["nodes"]["W0"]["state"] = "accepted"
        copied["nodes"]["W0"]["commit"] = head
        copied["nodes"]["W0"]["evidence"] = ["docs/fork/ideal-base/evidence/README.md"]
        railway.validate_state(copied, nodes)
        copied["nodes"]["W0"]["evidence"] = ["does/not/exist"]
        with self.assertRaisesRegex(railway.RailwayError, "missing evidence"):
            railway.validate_state(copied, nodes)

    def test_root_state_must_not_contradict_its_children(self) -> None:
        """A root's state and its children's states must tell the same story.

        Per-node validation cannot catch this: every individual record can be
        well-formed while the wave as a whole is incoherent. W3 really did sit
        at ``pending`` with all nine children ``accepted`` and ``check``
        reported OK, so ``next`` kept offering to seed an already-finished wave.
        """
        graph, state, _ = railway.validate_repository()
        root_id = next(
            root for root, children in graph["expansions"].items() if children
        )
        children = graph["expansions"][root_id]

        # A pending root with any progressed child is re-seeded as new work.
        copied = json.loads(json.dumps(state))
        copied["nodes"][root_id]["state"] = "pending"
        copied["nodes"][children[0]["id"]]["state"] = "in_progress"
        with self.assertRaisesRegex(railway.RailwayError, "root is pending"):
            railway.validate_expansion_consistency(graph, copied)

        # A wave whose children are all complete must be closed, not left open.
        copied = json.loads(json.dumps(state))
        copied["nodes"][root_id]["state"] = "in_progress"
        for child in children:
            copied["nodes"][child["id"]]["state"] = "accepted"
        with self.assertRaisesRegex(railway.RailwayError, "every child is complete"):
            railway.validate_expansion_consistency(graph, copied)

        # Coherent states pass: a fully accepted wave, and an untouched one.
        copied["nodes"][root_id]["state"] = "accepted"
        railway.validate_expansion_consistency(graph, copied)
        copied["nodes"][root_id]["state"] = "pending"
        for child in children:
            copied["nodes"][child["id"]]["state"] = "pending"
        railway.validate_expansion_consistency(graph, copied)

    def test_atomic_json_write_is_complete(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "state.json"
            value = {"node": "W0", "state": "accepted"}
            railway.atomic_write_json(path, value)
            self.assertEqual(json.loads(path.read_text()), value)
            self.assertEqual(list(path.parent.glob(f".{path.name}.*")), [])


if __name__ == "__main__":
    unittest.main()
