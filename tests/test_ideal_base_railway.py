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


class SchemaV2ValidatorTests(unittest.TestCase):
    """R07 design §8: reviewed_commit/published_commit split and semantics.

    These tests exercise ``_validate_state_v2`` directly against the live
    graph's node set (so accepted/pending shapes match reality) rather than
    against the coordinator-owned ``STATE.json``, which stream S must not
    edit. The R07 proposal artifact itself (``STATE.proposed.json``) is
    checked separately in ``test_state_proposed_json_validates_as_schema_v2``.
    """

    R07_EVIDENCE = railway.CONTROL_ROOT / "evidence/R07"
    BASELINE_MAIN = "498249777c453c1d551aeb01fc45420d8ca0a585"

    @classmethod
    def setUpClass(cls) -> None:
        graph = railway.load_json(railway.GRAPH_PATH)
        cls.nodes = railway.validate_graph(graph)
        cls.graph = graph
        # A commit that certainly exists locally but is not on origin/main:
        # any reviewed-only commit works, e.g. the R07 design tip itself
        # relative to the pre-R07 baseline.
        cls.head = subprocess.check_output(
            ["git", "rev-parse", "HEAD"], cwd=railway.REPO_ROOT, text=True
        ).strip()

    def minimal_state(self) -> dict:
        records = {}
        for node_id in self.nodes:
            records[node_id] = {
                "state": "pending",
                "reviewed_commit": None,
                "published_commit": None,
                "evidence": [],
                "summary": "seed",
                "updated_at": "2026-01-01T00:00:00Z",
            }
        return {
            "schema_version": 2,
            "program": "test",
            "program_state": "railway_ready",
            "active_graph_id": "test",
            "last_checkpoint": {
                "node": "W0",
                "state": "pending",
                "reviewed_commit": None,
                "published_commit": None,
                "updated_at": "2026-01-01T00:00:00Z",
                "summary": "seed",
            },
            "nodes": records,
        }

    def test_pending_record_requires_null_commits(self) -> None:
        state = self.minimal_state()
        railway.validate_state(state, self.nodes, published_ref=self.BASELINE_MAIN)
        state["nodes"]["W0"]["reviewed_commit"] = self.head
        with self.assertRaisesRegex(railway.RailwayError, "must use null"):
            railway.validate_state(state, self.nodes, published_ref=self.BASELINE_MAIN)

    def test_accepted_record_requires_both_full_shas(self) -> None:
        state = self.minimal_state()
        state["nodes"]["W0"].update(
            {
                "state": "accepted",
                "reviewed_commit": self.head,
                "published_commit": self.BASELINE_MAIN,
                "evidence": ["docs/fork/ideal-base/evidence/README.md"],
            }
        )
        railway.validate_state(state, self.nodes, published_ref=self.BASELINE_MAIN)

        # Abbreviated SHA is rejected even though `git cat-file -e` would
        # happily resolve it: schema v2 requires the full 40-hex identity.
        state["nodes"]["W0"]["reviewed_commit"] = self.head[:10]
        with self.assertRaisesRegex(railway.RailwayError, "40-hex"):
            railway.validate_state(state, self.nodes, published_ref=self.BASELINE_MAIN)

    def test_object_existence_is_never_reachability(self) -> None:
        """Existence of the reviewed commit is not enough; ancestry is checked separately."""
        state = self.minimal_state()
        state["nodes"]["W0"].update(
            {
                "state": "accepted",
                # `self.head` exists but (per this repository's own R07
                # branch history) is not an ancestor of the pre-R07 baseline.
                "reviewed_commit": self.head,
                "published_commit": self.head,
                "evidence": ["docs/fork/ideal-base/evidence/README.md"],
            }
        )
        # published_commit must be an ancestor of published_ref: using a
        # commit that exists but is not on the baseline must fail even
        # though object existence alone would pass.
        self.assertTrue(railway.git_commit_object_exists(self.head))
        self.assertFalse(
            railway.git_commit_is_ancestor(self.head, self.BASELINE_MAIN)
        )
        with self.assertRaisesRegex(railway.RailwayError, "not an ancestor"):
            railway.validate_state(state, self.nodes, published_ref=self.BASELINE_MAIN)

    def test_published_commit_must_be_ancestor_of_published_ref(self) -> None:
        state = self.minimal_state()
        state["nodes"]["W0"].update(
            {
                "state": "accepted",
                "reviewed_commit": self.head,
                "published_commit": self.BASELINE_MAIN,
                "evidence": ["docs/fork/ideal-base/evidence/README.md"],
            }
        )
        railway.validate_state(state, self.nodes, published_ref=self.BASELINE_MAIN)
        # The baseline is not an ancestor of itself-minus-one; use a ref that
        # the baseline is not reachable from to force a real failure.
        with self.assertRaisesRegex(railway.RailwayError, "not an ancestor"):
            railway.validate_state(
                state, self.nodes, published_ref=f"{self.BASELINE_MAIN}~1"
            )

    def test_reviewed_commit_need_not_be_ancestor_of_published_ref(self) -> None:
        """The reviewed identity is a distinct topic commit, never main-ancestral."""
        state = self.minimal_state()
        state["nodes"]["W0"].update(
            {
                "state": "accepted",
                "reviewed_commit": self.head,  # not an ancestor of baseline
                "published_commit": self.BASELINE_MAIN,
                "evidence": ["docs/fork/ideal-base/evidence/README.md"],
            }
        )
        self.assertFalse(
            railway.git_commit_is_ancestor(self.head, self.BASELINE_MAIN)
        )
        railway.validate_state(state, self.nodes, published_ref=self.BASELINE_MAIN)

    def test_unresolved_published_ref_fails_closed(self) -> None:
        state = self.minimal_state()
        with self.assertRaisesRegex(railway.RailwayError, "does not resolve"):
            railway.validate_state(
                state, self.nodes, published_ref="refs/does/not/exist"
            )

    def test_shallow_repository_fails_closed_no_escape_hatch(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            subprocess.run(
                [
                    "git",
                    "clone",
                    "--quiet",
                    "--no-local",
                    "--depth",
                    "1",
                    f"file://{railway.REPO_ROOT}",
                    directory,
                ],
                check=True,
            )
            self.assertTrue(railway.git_repository_is_shallow(cwd=Path(directory)))
            # `_validate_state_v2` calls the shallow check unconditionally
            # against `REPO_ROOT`. Point it at the shallow clone for the
            # duration of this test to exercise the fail-closed path through
            # the real validator entry point, not just the bare helper.
            original_is_shallow = railway.git_repository_is_shallow
            railway.git_repository_is_shallow = (
                lambda *, cwd=Path(directory): original_is_shallow(cwd=cwd)
            )
            try:
                state = self.minimal_state()
                with self.assertRaisesRegex(railway.RailwayError, "shallow"):
                    railway.validate_state(
                        state, self.nodes, published_ref=self.BASELINE_MAIN
                    )
            finally:
                railway.git_repository_is_shallow = original_is_shallow

    def test_authorization_blocked_still_requires_gated_class(self) -> None:
        """authorization_blocked is itself dependency-complete (design §8: the
        DEPENDENCY_COMPLETE set is {accepted, authorization_blocked,
        superseded}), so it too needs both full commit identities; the class
        restriction is an orthogonal check on top of that.
        """
        gated_id = next(
            node_id
            for node_id, node in self.nodes.items()
            if node.get("class") == "gated"
        )
        non_gated_id = next(
            node_id
            for node_id, node in self.nodes.items()
            if node.get("class") != "gated"
        )
        state = self.minimal_state()
        blocked_shape = {
            "state": "authorization_blocked",
            "reviewed_commit": self.head,
            "published_commit": self.BASELINE_MAIN,
            "evidence": ["docs/fork/ideal-base/evidence/README.md"],
        }
        state["nodes"][gated_id].update(blocked_shape)
        railway.validate_state(state, self.nodes, published_ref=self.BASELINE_MAIN)
        state["nodes"][non_gated_id].update(blocked_shape)
        with self.assertRaisesRegex(
            railway.RailwayError, "only gated nodes may be authorization_blocked"
        ):
            railway.validate_state(state, self.nodes, published_ref=self.BASELINE_MAIN)

    def test_unsupported_schema_version_is_rejected(self) -> None:
        state = self.minimal_state()
        state["schema_version"] = 3
        with self.assertRaisesRegex(
            railway.RailwayError, "unsupported STATE.json schema_version"
        ):
            railway.validate_state(state, self.nodes)

    def test_state_proposed_json_validates_as_schema_v2(self) -> None:
        """The R07 coordinator hand-off artifact must validate clean end to end."""
        proposed = railway.load_json(self.R07_EVIDENCE / "STATE.proposed.json")
        self.assertEqual(proposed.get("schema_version"), 2)
        self.assertEqual(set(proposed["nodes"]), set(self.nodes))
        railway.validate_state(
            proposed, self.nodes, published_ref="refs/remotes/origin/main"
        )
        railway.validate_expansion_consistency(self.graph, proposed)
        accepted = {
            node_id: record
            for node_id, record in proposed["nodes"].items()
            if record["state"] == "accepted"
        }
        self.assertEqual(len(accepted), 35)
        for node_id, record in accepted.items():
            self.assertTrue(railway.FULL_SHA.match(record["reviewed_commit"]), node_id)
            self.assertTrue(railway.FULL_SHA.match(record["published_commit"]), node_id)
            self.assertTrue(
                railway.git_commit_is_ancestor(
                    record["published_commit"], self.BASELINE_MAIN
                ),
                f"{node_id}: published_commit must be an ancestor of baseline main",
            )

    def test_live_state_json_is_schema_v2_and_validates(self) -> None:
        """Post-migration, live STATE.json is schema v2 and must validate.

        The coordinator landed the schema-v2 migration in the same change as
        this validator (design §8, "Landing either half alone is invalid"), so
        the live file now carries reviewed_commit/published_commit pairs and
        must pass the v2 rules against the real published ref.
        """
        live = railway.load_json(railway.STATE_PATH)
        self.assertEqual(live.get("schema_version"), 2)
        railway.validate_state(
            live, self.nodes, published_ref=self.BASELINE_MAIN
        )
        for node_id, record in live["nodes"].items():
            if record["state"] in railway.DEPENDENCY_COMPLETE:
                self.assertNotIn("commit", record)
                self.assertIn("reviewed_commit", record)
                self.assertIn("published_commit", record)


class CheckpointSchemaV2Tests(unittest.TestCase):
    """`checkpoint` CLI behavior against a scratch schema-v2 STATE.json."""

    BASELINE_MAIN = "498249777c453c1d551aeb01fc45420d8ca0a585"

    def setUp(self) -> None:
        self.tempdir = tempfile.TemporaryDirectory()
        self.addCleanup(self.tempdir.cleanup)
        self.state_path = Path(self.tempdir.name) / "STATE.json"
        graph = railway.load_json(railway.GRAPH_PATH)
        nodes = railway.validate_graph(graph)
        self.nodes = nodes
        records = {
            node_id: {
                "state": "pending",
                "reviewed_commit": None,
                "published_commit": None,
                "evidence": [],
                "summary": "seed",
                "updated_at": "2026-01-01T00:00:00Z",
            }
            for node_id in nodes
        }
        state = {
            "schema_version": 2,
            "program": "test",
            "program_state": "railway_ready",
            "active_graph_id": "test",
            "last_checkpoint": {
                "node": "W0",
                "state": "pending",
                "reviewed_commit": None,
                "published_commit": None,
                "updated_at": "2026-01-01T00:00:00Z",
                "summary": "seed",
            },
            "nodes": records,
        }
        self.state_path.write_text(json.dumps(state, indent=2))
        self.head = subprocess.check_output(
            ["git", "rev-parse", "HEAD"], cwd=railway.REPO_ROOT, text=True
        ).strip()

    def run_checkpoint(self, argv: list[str]) -> int:
        original_state_path = railway.STATE_PATH
        railway.STATE_PATH = self.state_path
        try:
            parser = railway.build_parser()
            args = parser.parse_args(["checkpoint", *argv])
            return args.handler(args)
        finally:
            railway.STATE_PATH = original_state_path

    def test_bare_commit_flag_is_rejected_against_schema_v2(self) -> None:
        with self.assertRaisesRegex(railway.RailwayError, r"--commit is not valid"):
            self.run_checkpoint(
                [
                    "W0",
                    "--state",
                    "in_progress",
                    "--commit",
                    self.head,
                    "--summary",
                    "x",
                    "--updated-at",
                    "2026-01-01T00:00:00Z",
                ]
            )

    def test_accepted_checkpoint_requires_reviewed_and_published_commit(self) -> None:
        with self.assertRaisesRegex(railway.RailwayError, "40-hex"):
            self.run_checkpoint(
                [
                    "W0",
                    "--state",
                    "accepted",
                    "--reviewed-commit",
                    self.head,
                    "--evidence",
                    "docs/fork/ideal-base/evidence/README.md",
                    "--summary",
                    "x",
                    "--updated-at",
                    "2026-01-01T00:00:00Z",
                ]
            )

    def test_accepted_checkpoint_writes_both_commits_and_last_checkpoint(self) -> None:
        code = self.run_checkpoint(
            [
                "W0",
                "--state",
                "accepted",
                "--reviewed-commit",
                self.head,
                "--published-commit",
                self.BASELINE_MAIN,
                "--evidence",
                "docs/fork/ideal-base/evidence/README.md",
                "--published-ref",
                self.BASELINE_MAIN,
                "--summary",
                "checkpoint test",
                "--updated-at",
                "2026-01-01T00:00:00Z",
            ]
        )
        self.assertEqual(code, 0)
        written = json.loads(self.state_path.read_text())
        record = written["nodes"]["W0"]
        self.assertEqual(record["state"], "accepted")
        self.assertEqual(record["reviewed_commit"], self.head)
        self.assertEqual(record["published_commit"], self.BASELINE_MAIN)
        self.assertEqual(written["last_checkpoint"]["node"], "W0")
        self.assertEqual(written["last_checkpoint"]["reviewed_commit"], self.head)
        self.assertEqual(
            written["last_checkpoint"]["published_commit"], self.BASELINE_MAIN
        )
        self.assertNotIn("commit", record)

    def test_published_commit_not_ancestor_of_published_ref_is_rejected(self) -> None:
        with self.assertRaisesRegex(railway.RailwayError, "not an ancestor"):
            self.run_checkpoint(
                [
                    "W0",
                    "--state",
                    "accepted",
                    "--reviewed-commit",
                    self.head,
                    "--published-commit",
                    self.head,
                    "--evidence",
                    "docs/fork/ideal-base/evidence/README.md",
                    "--published-ref",
                    self.BASELINE_MAIN,
                    "--summary",
                    "x",
                    "--updated-at",
                    "2026-01-01T00:00:00Z",
                ]
            )

    def test_non_terminal_checkpoint_rejects_commit_identities(self) -> None:
        with self.assertRaisesRegex(railway.RailwayError, "must leave"):
            self.run_checkpoint(
                [
                    "W0",
                    "--state",
                    "in_progress",
                    "--reviewed-commit",
                    self.head,
                    "--summary",
                    "x",
                    "--updated-at",
                    "2026-01-01T00:00:00Z",
                ]
            )


if __name__ == "__main__":
    unittest.main()
