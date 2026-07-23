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
        graph, state, nodes = railway.validate_repository()
        self.assertEqual(len(graph["root_nodes"]), 6)
        self.assertEqual(len(graph["all_nodes"]), 38)
        self.assertEqual(len(graph["audit_coverage"]), 25)
        coordinator_paths = set(graph["coordinator_owned_paths"])
        self.assertTrue(coordinator_paths)
        self.assertTrue(
            all(
                not (set(node.get("owned_paths", [])) & coordinator_paths)
                for node in graph["all_nodes"]
            )
        )
        self.assertEqual(set(state["nodes"]), set(nodes))

    def test_initial_runnable_projection_contains_only_bootstrap_root(self) -> None:
        graph, state, nodes = railway.validate_repository()
        ready = railway.ready_nodes(graph, state, nodes)
        self.assertEqual(
            [(node["id"], node["action"]) for node in ready],
            [("W0", "seed_and_expand")],
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

    def test_atomic_json_write_is_complete(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "state.json"
            value = {"node": "W0", "state": "accepted"}
            railway.atomic_write_json(path, value)
            self.assertEqual(json.loads(path.read_text()), value)
            self.assertEqual(list(path.parent.glob(f".{path.name}.*")), [])


if __name__ == "__main__":
    unittest.main()
