#!/usr/bin/env python3
"""Regression coverage for the executable modernization graph."""

from __future__ import annotations

import importlib.util
import sys
import unittest
from pathlib import Path
from types import ModuleType

REPO_ROOT = Path(__file__).resolve().parent.parent
VALIDATOR_PATH = REPO_ROOT / "docs" / "modernization" / "validate_graph.py"


def load_validator() -> ModuleType:
    spec = importlib.util.spec_from_file_location("modernization_validate_graph", VALIDATOR_PATH)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load {VALIDATOR_PATH}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


validator = load_validator()


class ModernizationGraphTest(unittest.TestCase):
    def test_wave_metrics_expose_ready_frontier_width(self) -> None:
        graph = {
            "default_concurrency": 2,
            "nodes": [
                {"id": "A", "priority": 10, "depends_on": []},
                {"id": "B", "priority": 9, "depends_on": []},
                {"id": "C", "priority": 8, "depends_on": ["A", "B"]},
            ],
        }

        waves = validator.topological_waves(graph)

        self.assertEqual(waves, [["A", "B"], ["C"]])
        self.assertEqual(
            validator.wave_metrics(graph, waves),
            {
                "wave_count": 2,
                "singleton_waves": 1,
                "max_width": 2,
                "equal_duration_average_parallelism": 1.5,
                "configured_concurrency": 2,
            },
        )

    def test_swarm_content_marks_atomic_and_composite_nodes(self) -> None:
        atomic = validator.swarm_content(
            {"id": "A", "content": "atomic work", "falsification": "stop"}
        )
        composite = validator.swarm_content(
            {
                "id": "B",
                "content": "composite work",
                "expandable": True,
                "required_children_min": 2,
                "required_children_max": 4,
                "falsification": "stop",
            }
        )

        self.assertIn("EXECUTION SHAPE: ATOMIC", atomic)
        self.assertIn("EXECUTION SHAPE: COMPOSITE", composite)
        self.assertIn("2 to 4", composite)

    def test_current_plan_can_reach_its_default_concurrency(self) -> None:
        graph = validator.load_graph()
        report = validator.validate(graph)
        self.assertTrue(report["ok"], report["errors"])

        composite_ids = {
            node["id"] for node in graph["nodes"] if node.get("expandable")
        }
        metrics = validator.wave_metrics(graph, validator.topological_waves(graph))

        self.assertEqual(composite_ids, {"M00", "A25"})
        self.assertGreaterEqual(metrics["max_width"], graph["default_concurrency"])


if __name__ == "__main__":
    unittest.main()
