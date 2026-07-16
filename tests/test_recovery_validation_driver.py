import importlib.util
import json
import tempfile
import unittest
from pathlib import Path
from unittest import mock


SCRIPT = Path(__file__).resolve().parents[1] / "scripts" / "recovery_validation_driver.py"
SPEC = importlib.util.spec_from_file_location("recovery_validation_driver", SCRIPT)
assert SPEC and SPEC.loader
DRIVER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(DRIVER)


def valid_plan():
    return {
        "schema_version": 1,
        "name": "test",
        "branch": "test",
        "prompt_diff_sha256": "prompt",
        "expected_stash_count": 0,
        "vendor_upstream_head": "vendor",
        "minimum_free_gib": 0,
        "forbidden_output_substrings": [],
        "steps": [
            {
                "name": "ok",
                "command": "python3 -c 'print(1)'",
                "expected_exit": 0,
                "timeout_seconds": 10,
            }
        ],
    }


class RecoveryValidationDriverTests(unittest.TestCase):
    def test_vendor_upstream_head_resolves_authoritative_local_branch(self):
        with mock.patch.object(DRIVER, "git_output", return_value="abc123\n") as git_output:
            self.assertEqual(DRIVER.vendor_upstream_head(), "abc123")
        git_output.assert_called_once_with(
            "rev-parse", "--verify", "refs/heads/vendor/upstream^{commit}"
        )

    def test_checked_in_pilot_plan_is_valid(self):
        plan_path = (
            Path(__file__).resolve().parents[1]
            / "docs"
            / "fork"
            / "recovery"
            / "pilot"
            / "2026-07-15-g4-validation-plan.json"
        )
        DRIVER.validate_plan(json.loads(plan_path.read_text()))

    def test_validate_plan_rejects_update(self):
        plan = valid_plan()
        plan["steps"][0]["command"] = "python3 gate.py --update"
        with self.assertRaisesRegex(ValueError, "forbidden"):
            DRIVER.validate_plan(plan)

    def test_validate_plan_rejects_duplicate_step_names(self):
        plan = valid_plan()
        plan["steps"].append(dict(plan["steps"][0]))
        with self.assertRaisesRegex(ValueError, "duplicate"):
            DRIVER.validate_plan(plan)

    def test_validate_plan_rejects_network_command(self):
        plan = valid_plan()
        plan["steps"][0]["command"] = "curl https://example.com"
        with self.assertRaisesRegex(ValueError, "forbidden"):
            DRIVER.validate_plan(plan)

    def test_validate_plan_rejects_invalid_forbidden_output_list(self):
        plan = valid_plan()
        plan["forbidden_output_substrings"] = [""]
        with self.assertRaisesRegex(ValueError, "forbidden_output_substrings"):
            DRIVER.validate_plan(plan)

    def test_preservation_projection_ignores_runtime_measurements(self):
        base = {
            "head": "abc",
            "branch": "recovery/2026-07-15",
            "dirty_paths": [DRIVER.PROMPT_PATH],
            "prompt_diff_sha256": "hash",
            "stash_count": 4,
            "stash_list_sha256": "stash",
            "worktrees_sha256": "worktrees",
            "branches_sha256": "branches",
            "vendor_upstream_head": "vendor",
            "disk_free_bytes": 1,
            "active_build_processes": [],
        }
        later = dict(base, disk_free_bytes=2, active_build_processes=[{"pid": 1}])
        self.assertEqual(
            DRIVER.preservation_projection(base),
            DRIVER.preservation_projection(later),
        )

    def test_sha256sums_are_sorted_and_exclude_themselves(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "b.log").write_text("b")
            (root / "a.log").write_text("a")
            DRIVER.write_sha256sums(root)
            lines = (root / "SHA256SUMS").read_text().splitlines()
            self.assertEqual([line.split("  ", 1)[1] for line in lines], ["a.log", "b.log"])

    def test_run_step_requires_exactly_one_pilot_observation(self):
        step = {
            "name": "pilot_fixture",
            "command": "printf 'PILOT_OBSERVATION {\"ok\":true}\\n'",
            "expected_exit": 0,
            "timeout_seconds": 10,
        }
        with tempfile.TemporaryDirectory() as directory:
            row = DRIVER.run_step(1, 1, step, Path(directory), dict(DRIVER.os.environ), [])
        self.assertEqual(row["actual_exit"], 0)
        self.assertEqual(row["pilot_observation_count"], 1)

    def test_run_step_fails_on_forbidden_output(self):
        step = {
            "name": "check",
            "command": "printf 'credential-value\\n'",
            "expected_exit": 0,
            "timeout_seconds": 10,
        }
        with tempfile.TemporaryDirectory() as directory:
            row = DRIVER.run_step(
                1,
                1,
                step,
                Path(directory),
                dict(DRIVER.os.environ),
                ["credential-value"],
            )
        self.assertEqual(row["actual_exit"], 126)
        self.assertEqual(row["forbidden_output_hits"], 1)


if __name__ == "__main__":
    unittest.main()
