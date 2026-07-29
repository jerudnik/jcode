#!/usr/bin/env python3
"""Executable guard for the fork's structured advisory-ownership policy.

Two kinds of test here, deliberately:

  * against the real tree - the checked-in `.cargo/audit.toml` and
    `docs/security/advisories.toml` must pass, and must actually contain
    ownership metadata rather than an empty shell;
  * against synthetic fixture trees - each failure mode the checker claims to
    catch is planted and observed failing, so a gate that has never been seen
    red is never trusted.

Every fixture injects its own current date through `--today`, so expiry
behavior does not depend on when the suite runs. `test_expiry_is_deterministic`
pins that property directly.
"""

from __future__ import annotations

import datetime as dt
import pathlib
import subprocess
import sys
import tempfile
import tomllib
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[1]
CHECKER = ROOT / "scripts/check_advisory_policy.py"

# Fixed reference dates. Nothing in this file reads the wall clock except
# test_expiry_is_deterministic, which asserts the wall clock does not matter.
TODAY = "2026-07-29"
BEFORE_EXPIRY = "2026-12-31"
AFTER_EXPIRY = "2027-03-01"

VALID_RECORD = """
[policy]
max_expiry_days = 365

[[advisory]]
id = "RUSTSEC-2026-0141"
crate_name = "lettre"
owner = "jerudnik"
accepted = "2026-07-29"
expires = "2027-01-29"
affected_surface = "jcode-notify-email outbound SMTP"
rationale = "boring-tls backend is not compiled into any jcode target"
retire_when = "lettre ships a patched release"
"""

VALID_IGNORES = """
[advisories]
ignore = ["RUSTSEC-2026-0141"]
"""


def run_checker(root: pathlib.Path, today: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, str(CHECKER), "--root", str(root), "--today", today],
        capture_output=True,
        text=True,
        check=False,
    )


class FixtureTree:
    """A minimal synthetic repo: just the two files the checker reads."""

    def __init__(self, audit: str, record: str | None) -> None:
        self._tmp = tempfile.TemporaryDirectory()
        self.path = pathlib.Path(self._tmp.name)
        (self.path / ".cargo").mkdir()
        (self.path / ".cargo/audit.toml").write_text(audit)
        if record is not None:
            (self.path / "docs/security").mkdir(parents=True)
            (self.path / "docs/security/advisories.toml").write_text(record)

    def __enter__(self) -> FixtureTree:
        return self

    def __exit__(self, *exc: object) -> None:
        self._tmp.cleanup()


class RealTreePolicy(unittest.TestCase):
    """The tree as committed must satisfy its own policy."""

    def test_real_tree_passes(self) -> None:
        result = run_checker(ROOT, TODAY)
        self.assertEqual(result.returncode, 0, f"{result.stdout}\n{result.stderr}")

    def test_record_file_is_machine_readable_and_complete(self) -> None:
        document = tomllib.loads((ROOT / "docs/security/advisories.toml").read_text())
        records = document["advisory"]
        self.assertGreater(len(records), 0, "policy must describe the advisories actually ignored")
        for record in records:
            with self.subTest(advisory=record.get("id")):
                for field in (
                    "id",
                    "crate_name",
                    "owner",
                    "accepted",
                    "expires",
                    "affected_surface",
                    "rationale",
                    "retire_when",
                ):
                    self.assertTrue(str(record.get(field, "")).strip(), f"blank {field}")
                self.assertIsInstance(dt.date.fromisoformat(str(record["expires"])), dt.date)

    def test_every_ignore_has_a_record(self) -> None:
        audit = tomllib.loads((ROOT / ".cargo/audit.toml").read_text())
        ignored = {entry.split("/")[0] for entry in audit["advisories"]["ignore"]}
        document = tomllib.loads((ROOT / "docs/security/advisories.toml").read_text())
        documented = {str(record["id"]) for record in document["advisory"]}
        self.assertEqual(ignored, documented)


class UndocumentedIgnoreFails(unittest.TestCase):
    def test_ignore_without_record_fails(self) -> None:
        audit = '[advisories]\nignore = ["RUSTSEC-2026-0141", "RUSTSEC-2099-0001"]\n'
        with FixtureTree(audit, VALID_RECORD) as tree:
            result = run_checker(tree.path, TODAY)
        self.assertEqual(result.returncode, 1)
        self.assertIn("RUSTSEC-2099-0001", result.stderr)
        self.assertIn("no record", result.stderr)

    def test_missing_record_file_fails(self) -> None:
        with FixtureTree(VALID_IGNORES, None) as tree:
            result = run_checker(tree.path, TODAY)
        self.assertEqual(result.returncode, 1)
        self.assertIn("docs/security/advisories.toml", result.stderr)

    def test_stale_record_without_ignore_fails(self) -> None:
        audit = "[advisories]\nignore = []\n"
        with FixtureTree(audit, VALID_RECORD) as tree:
            result = run_checker(tree.path, TODAY)
        self.assertEqual(result.returncode, 1)
        self.assertIn("stale record", result.stderr)


class IncompleteRecordFails(unittest.TestCase):
    def test_each_required_field_is_enforced(self) -> None:
        for field in (
            "crate_name",
            "owner",
            "accepted",
            "expires",
            "affected_surface",
            "rationale",
            "retire_when",
        ):
            record = "\n".join(
                line for line in VALID_RECORD.splitlines() if not line.startswith(f"{field} = ")
            )
            with self.subTest(missing=field), FixtureTree(VALID_IGNORES, record) as tree:
                result = run_checker(tree.path, TODAY)
                self.assertEqual(result.returncode, 1, f"dropping {field} was accepted")
                self.assertIn(field, result.stderr)

    def test_blank_field_is_not_documentation(self) -> None:
        record = VALID_RECORD.replace('rationale = "boring-tls backend is not compiled into any jcode target"', 'rationale = "   "')
        with FixtureTree(VALID_IGNORES, record) as tree:
            result = run_checker(tree.path, TODAY)
        self.assertEqual(result.returncode, 1)
        self.assertIn("rationale", result.stderr)

    def test_malformed_expiry_fails(self) -> None:
        record = VALID_RECORD.replace('expires = "2027-01-29"', 'expires = "sometime in 2027"')
        with FixtureTree(VALID_IGNORES, record) as tree:
            result = run_checker(tree.path, TODAY)
        self.assertEqual(result.returncode, 1)
        self.assertIn("ISO", result.stderr)


class ExpiryFails(unittest.TestCase):
    def test_expired_ignore_fails(self) -> None:
        with FixtureTree(VALID_IGNORES, VALID_RECORD) as tree:
            result = run_checker(tree.path, AFTER_EXPIRY)
        self.assertEqual(result.returncode, 1)
        self.assertIn("expired", result.stderr)
        self.assertIn("RUSTSEC-2026-0141", result.stderr)

    def test_unexpired_ignore_passes(self) -> None:
        with FixtureTree(VALID_IGNORES, VALID_RECORD) as tree:
            result = run_checker(tree.path, BEFORE_EXPIRY)
        self.assertEqual(result.returncode, 0, f"{result.stdout}\n{result.stderr}")

    def test_expiry_boundary_is_inclusive(self) -> None:
        """The expiry date itself is already too late; no silent last-day grace."""
        with FixtureTree(VALID_IGNORES, VALID_RECORD) as tree:
            self.assertEqual(run_checker(tree.path, "2027-01-28").returncode, 0)
            self.assertEqual(run_checker(tree.path, "2027-01-29").returncode, 1)

    def test_expiry_window_is_capped(self) -> None:
        record = VALID_RECORD.replace('expires = "2027-01-29"', 'expires = "2036-01-29"')
        with FixtureTree(VALID_IGNORES, record) as tree:
            result = run_checker(tree.path, TODAY)
        self.assertEqual(result.returncode, 1)
        self.assertIn("max_expiry_days", result.stderr)

    def test_expiry_is_deterministic(self) -> None:
        """Same tree, same injected date, same verdict, whatever the wall clock says.

        This is the property that makes the expiry gate testable: the checker
        must never consult `date` when a date is injected. Two dates that
        straddle the expiry are evaluated, and the verdicts must differ purely
        because of the injected value.
        """
        with FixtureTree(VALID_IGNORES, VALID_RECORD) as tree:
            before = [run_checker(tree.path, BEFORE_EXPIRY).returncode for _ in range(3)]
            after = [run_checker(tree.path, AFTER_EXPIRY).returncode for _ in range(3)]
        self.assertEqual(before, [0, 0, 0])
        self.assertEqual(after, [1, 1, 1])

    def test_injected_date_overrides_environment(self) -> None:
        import os

        env = dict(os.environ, ADVISORY_POLICY_TODAY=BEFORE_EXPIRY)
        with FixtureTree(VALID_IGNORES, VALID_RECORD) as tree:
            explicit = subprocess.run(
                [sys.executable, str(CHECKER), "--root", str(tree.path), "--today", AFTER_EXPIRY],
                capture_output=True,
                text=True,
                env=env,
                check=False,
            )
            from_env = subprocess.run(
                [sys.executable, str(CHECKER), "--root", str(tree.path)],
                capture_output=True,
                text=True,
                env=env,
                check=False,
            )
        self.assertEqual(explicit.returncode, 1, "--today must win over the environment")
        self.assertEqual(from_env.returncode, 0, "environment date must be honored when --today is absent")


class DuplicateRecordFails(unittest.TestCase):
    def test_duplicate_id_fails(self) -> None:
        _, body = VALID_RECORD.split("[[advisory]]", 1)
        record = VALID_RECORD + "[[advisory]]" + body
        with FixtureTree(VALID_IGNORES, record) as tree:
            result = run_checker(tree.path, TODAY)
        self.assertEqual(result.returncode, 1)
        self.assertIn("duplicate", result.stderr)


class WiringIsReal(unittest.TestCase):
    """A checker nothing runs is not a gate."""

    def test_security_workflow_runs_the_checker(self) -> None:
        workflow = (ROOT / ".github/workflows/security.yml").read_text()
        self.assertIn("scripts/check_advisory_policy.py", workflow)

    def test_preflight_runs_the_checker(self) -> None:
        preflight = (ROOT / "scripts/preflight.sh").read_text()
        self.assertIn("check_advisory_policy.py", preflight)

    def test_retired_homebrew_host_verification_is_gone(self) -> None:
        """F22's original Homebrew host-identity clause retired with the path itself."""
        release = (ROOT / ".github/workflows/release.yml").read_text()
        self.assertNotIn("StrictHostKeyChecking=no", release)
        self.assertNotIn("homebrew", release.lower())
        for name in ("docs/SECURITY_DEPENDENCIES.md", "docs/security/advisories.toml"):
            text = (ROOT / name).read_text().lower()
            with self.subTest(doc=name):
                self.assertNotIn("homebrew", text)


if __name__ == "__main__":
    unittest.main()
