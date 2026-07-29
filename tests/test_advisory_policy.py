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
import json
import pathlib
import re
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

# The surface CI actually executes. `security_preflight.sh` carries its own
# hardcoded array, so a fixture that only writes audit.toml would prove
# nothing about the file that really runs.
VALID_PREFLIGHT = """#!/usr/bin/env bash
set -euo pipefail
audit_ignores=(
  --ignore RUSTSEC-2026-0141 # lettre, boring-tls backend unused
)
cargo audit "${audit_ignores[@]}"
"""


def preflight_with(ids: list[str]) -> str:
    lines = "\n".join(f"  --ignore {advisory_id}" for advisory_id in ids)
    return (
        "#!/usr/bin/env bash\nset -euo pipefail\naudit_ignores=(\n"
        + lines
        + '\n)\ncargo audit "${audit_ignores[@]}"\n'
    )


def yaml_needs(workflow: pathlib.Path, job_id: str) -> list[str]:
    """The `needs:` list of one job, read without a YAML dependency.

    The workflows write `needs: [a, b, c]` on one line, so a targeted regex is
    honest here and keeps the suite runnable on a bare `python3` (which is what
    the CI job uses).
    """
    text = workflow.read_text()
    block = re.search(rf"^  {re.escape(job_id)}:\n(.*?)(?=^  \w|\Z)", text, re.M | re.S)
    if block is None:
        raise AssertionError(f"job {job_id} not found in {workflow}")
    needs = re.search(r"^    needs:\s*\[(.*?)\]", block.group(1), re.M)
    if needs is None:
        raise AssertionError(f"job {job_id} has no inline needs: list")
    return [item.strip().strip("'\"") for item in needs.group(1).split(",") if item.strip()]


def run_checker(root: pathlib.Path, today: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, str(CHECKER), "--root", str(root), "--today", today],
        capture_output=True,
        text=True,
        check=False,
    )


class FixtureTree:
    """A minimal synthetic repo: the three files the checker reads."""

    def __init__(
        self, audit: str, record: str | None, preflight: str | None = None
    ) -> None:
        self._tmp = tempfile.TemporaryDirectory()
        self.path = pathlib.Path(self._tmp.name)
        (self.path / ".cargo").mkdir()
        (self.path / ".cargo/audit.toml").write_text(audit)
        (self.path / "scripts").mkdir()
        (self.path / "scripts/security_preflight.sh").write_text(
            VALID_PREFLIGHT if preflight is None else preflight
        )
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

    def test_preflight_array_agrees_with_audit_toml(self) -> None:
        """The surface CI actually executes must carry the same ignore set.

        `scripts/security_preflight.sh` has its own hardcoded array and is what
        ci.yml and security.yml --strict run. If it drifts from audit.toml,
        the governed list and the executed list are different lists.
        """
        audit = tomllib.loads((ROOT / ".cargo/audit.toml").read_text())
        ignored = {entry.split("/")[0] for entry in audit["advisories"]["ignore"]}
        preflight = (ROOT / "scripts/security_preflight.sh").read_text()
        executed = set(re.findall(r"--ignore\s+(RUSTSEC-\d{4}-\d{4})", preflight))
        self.assertEqual(executed, ignored)


class EverySuppressionSurfaceIsGoverned(unittest.TestCase):
    """A suppression is only as governed as its weakest surface.

    The first cut of this checker read only `.cargo/audit.toml`, so an ignore
    added straight to the preflight array -- the one CI executes -- passed
    silently. These fixtures pin that hole shut.
    """

    def test_undocumented_preflight_ignore_fails(self) -> None:
        preflight = preflight_with(["RUSTSEC-2026-0141", "RUSTSEC-2099-9999"])
        with FixtureTree(VALID_IGNORES, VALID_RECORD, preflight) as tree:
            result = run_checker(tree.path, TODAY)
        self.assertEqual(result.returncode, 1, "an undocumented preflight ignore was accepted")
        self.assertIn("RUSTSEC-2099-9999", result.stderr)
        self.assertIn("security_preflight.sh", result.stderr)

    def test_ignore_in_audit_but_not_preflight_fails(self) -> None:
        audit = '[advisories]\nignore = ["RUSTSEC-2026-0141", "RUSTSEC-2026-0190"]\n'
        record = VALID_RECORD + """
[[advisory]]
id = "RUSTSEC-2026-0190"
crate_name = "anyhow"
owner = "jerudnik"
accepted = "2026-07-29"
expires = "2027-01-29"
affected_surface = "workspace-wide error handling"
rationale = "downcast_mut unsoundness, no jcode call site casts through a shared reference"
retire_when = "a patched anyhow release exists"
"""
        with FixtureTree(audit, record, VALID_PREFLIGHT) as tree:
            result = run_checker(tree.path, TODAY)
        self.assertEqual(result.returncode, 1, "surface drift was accepted")
        self.assertIn("must agree", result.stderr)

    def test_ignore_in_preflight_but_not_audit_fails(self) -> None:
        preflight = preflight_with(["RUSTSEC-2026-0141", "RUSTSEC-2026-0190"])
        record = VALID_RECORD + """
[[advisory]]
id = "RUSTSEC-2026-0190"
crate_name = "anyhow"
owner = "jerudnik"
accepted = "2026-07-29"
expires = "2027-01-29"
affected_surface = "workspace-wide error handling"
rationale = "downcast_mut unsoundness, no jcode call site casts through a shared reference"
retire_when = "a patched anyhow release exists"
"""
        with FixtureTree(VALID_IGNORES, record, preflight) as tree:
            result = run_checker(tree.path, TODAY)
        self.assertEqual(result.returncode, 1, "surface drift was accepted")
        self.assertIn("must agree", result.stderr)

    def test_commented_out_preflight_ignore_is_not_a_suppression(self) -> None:
        preflight = """#!/usr/bin/env bash
audit_ignores=(
  --ignore RUSTSEC-2026-0141
  # --ignore RUSTSEC-2099-9999 retired, kept for history
)
"""
        with FixtureTree(VALID_IGNORES, VALID_RECORD, preflight) as tree:
            result = run_checker(tree.path, TODAY)
        self.assertEqual(result.returncode, 0, f"{result.stdout}\n{result.stderr}")

    def test_missing_preflight_file_fails(self) -> None:
        with FixtureTree(VALID_IGNORES, VALID_RECORD) as tree:
            (tree.path / "scripts/security_preflight.sh").unlink()
            result = run_checker(tree.path, TODAY)
        self.assertEqual(result.returncode, 1)
        self.assertIn("security_preflight.sh", result.stderr)


class SeverityThresholdIsGoverned(unittest.TestCase):
    """A threshold suppresses whole severity classes, including untriaged ones."""

    THRESHOLD_RECORD = """
[severity_threshold]
threshold = "critical"
owner = "jerudnik"
accepted = "2026-07-29"
expires = "2027-01-29"
rationale = "temporary while the backlog is triaged"
retire_when = "the low/medium backlog is cleared"
"""

    def test_threshold_without_record_fails(self) -> None:
        audit = '[advisories]\nignore = ["RUSTSEC-2026-0141"]\nseverity_threshold = "critical"\n'
        with FixtureTree(audit, VALID_RECORD) as tree:
            result = run_checker(tree.path, TODAY)
        self.assertEqual(result.returncode, 1, "a blanket severity threshold was accepted")
        self.assertIn("severity_threshold", result.stderr)

    def test_documented_threshold_passes(self) -> None:
        audit = '[advisories]\nignore = ["RUSTSEC-2026-0141"]\nseverity_threshold = "critical"\n'
        with FixtureTree(audit, VALID_RECORD + self.THRESHOLD_RECORD) as tree:
            result = run_checker(tree.path, TODAY)
        self.assertEqual(result.returncode, 0, f"{result.stdout}\n{result.stderr}")

    def test_threshold_record_expires(self) -> None:
        audit = '[advisories]\nignore = ["RUSTSEC-2026-0141"]\nseverity_threshold = "critical"\n'
        with FixtureTree(audit, VALID_RECORD + self.THRESHOLD_RECORD) as tree:
            result = run_checker(tree.path, AFTER_EXPIRY)
        self.assertEqual(result.returncode, 1)
        self.assertIn("severity_threshold", result.stderr)

    def test_threshold_record_must_match_configured_level(self) -> None:
        audit = '[advisories]\nignore = ["RUSTSEC-2026-0141"]\nseverity_threshold = "low"\n'
        with FixtureTree(audit, VALID_RECORD + self.THRESHOLD_RECORD) as tree:
            result = run_checker(tree.path, TODAY)
        self.assertEqual(result.returncode, 1, "record documented a different level than configured")
        self.assertIn("documents", result.stderr)

    def test_stale_threshold_record_fails(self) -> None:
        with FixtureTree(VALID_IGNORES, VALID_RECORD + self.THRESHOLD_RECORD) as tree:
            result = run_checker(tree.path, TODAY)
        self.assertEqual(result.returncode, 1, "a record for an unset threshold was accepted")
        self.assertIn("stale record", result.stderr)

    def test_real_tree_sets_no_ungoverned_threshold(self) -> None:
        audit = tomllib.loads((ROOT / ".cargo/audit.toml").read_text())
        threshold = audit.get("advisories", {}).get("severity_threshold")
        if threshold is not None:
            document = tomllib.loads((ROOT / "docs/security/advisories.toml").read_text())
            self.assertIn("severity_threshold", document)


class PostdatedAcceptanceFails(unittest.TestCase):
    """Expiry is an interval between two self-declared dates.

    Without this rule, `accepted = "2098-01-01"` with a 365-day window parks a
    suppression for 72 years and satisfies every other check.
    """

    def test_future_accepted_fails(self) -> None:
        record = VALID_RECORD.replace('accepted = "2026-07-29"', 'accepted = "2098-01-01"').replace(
            'expires = "2027-01-29"', 'expires = "2098-06-01"'
        )
        with FixtureTree(VALID_IGNORES, record) as tree:
            result = run_checker(tree.path, TODAY)
        self.assertEqual(result.returncode, 1, "a 72-year parked suppression was accepted")
        self.assertIn("future", result.stderr)

    def test_accepted_today_is_allowed(self) -> None:
        """The boundary is strict: accepting today is normal, tomorrow is not."""
        with FixtureTree(VALID_IGNORES, VALID_RECORD) as tree:
            self.assertEqual(run_checker(tree.path, "2026-07-29").returncode, 0)
            self.assertEqual(run_checker(tree.path, "2026-07-28").returncode, 1)


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
        """A record whose advisory is suppressed on *no* surface is dead weight.

        Both surfaces have to be cleared, otherwise this is surface drift
        rather than a stale record, which the checker reports separately.
        """
        audit = "[advisories]\nignore = []\n"
        empty_preflight = "#!/usr/bin/env bash\naudit_ignores=(\n)\n"
        with FixtureTree(audit, VALID_RECORD, empty_preflight) as tree:
            result = run_checker(tree.path, TODAY)
        self.assertEqual(result.returncode, 1)
        self.assertIn("stale record", result.stderr)

    def test_half_retired_advisory_fails(self) -> None:
        """Retiring on one surface only is caught as drift, not silently accepted."""
        audit = "[advisories]\nignore = []\n"
        with FixtureTree(audit, VALID_RECORD, VALID_PREFLIGHT) as tree:
            result = run_checker(tree.path, TODAY)
        self.assertEqual(result.returncode, 1)
        self.assertIn("must agree", result.stderr)


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

    def test_required_checks_manifest_lists_the_job(self) -> None:
        """security-gate's `needs:` and the governance manifest must agree.

        `governance_compare.py --live` asserts the manifest matches the real
        workflow. Adding a job to `needs:` without updating the manifest turns
        the daily Fork Health live run red, and the PR run will not catch it
        because fork-ci compares against an embedded fixture snapshot.
        """
        manifest = json.loads((ROOT / "scripts/required-checks.json").read_text())
        contract = next(
            c for c in manifest["workflow_contracts"] if c["job_id"] == "security-gate"
        )
        workflow = yaml_needs(ROOT / ".github/workflows/security.yml", "security-gate")
        self.assertEqual(sorted(contract["needs"]), sorted(workflow))
        self.assertIn("advisory-policy", contract["needs"])

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
