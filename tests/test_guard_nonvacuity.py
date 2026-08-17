#!/usr/bin/env python3
"""Planted-failure tests for the guard non-vacuity harness.

D029's standing rule is that a detector is not trusted until it has been
observed red, and `scripts/check_guard_nonvacuity.py` is itself a detector. So
this file does to the harness what the harness does to the guards: every test
takes a working claim, breaks exactly one thing, and asserts the harness reports
that specific breakage.

The load-bearing test is `test_the_d034_weakening_is_caught`. It reads the real
`check_critical_path_budget.py`, applies the exact edit D034's reproduction
used -- relaxing `value > REPOSITORY_CEILINGS[key]` to `... * 2` -- imports the
mutated copy, and asserts the real plant stops tripping. That is the end-to-end
proof that the control closes the hole it was built for, rather than a proof
that some mock returns False.

The mutation is applied to a copy in a temporary directory. Nothing in the
working tree is modified.

Run:  python3 -m unittest tests.test_guard_nonvacuity
"""

from __future__ import annotations

import contextlib
import importlib.util
import io
import sys
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(REPO_ROOT / "scripts"))

import check_guard_nonvacuity as harness  # noqa: E402


def _load_from(path: Path, name: str):
    spec = importlib.util.spec_from_file_location(name, path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class TheHarnessPassesOnTheRealTree(unittest.TestCase):
    def test_every_registered_claim_holds(self) -> None:
        failures, passes = harness.run()
        self.assertEqual(failures, [], "\n".join(failures))
        self.assertGreater(len(passes), 0)

    def test_the_registry_names_every_guard_on_disk(self) -> None:
        self.assertEqual(harness._check_registry_covers_every_guard(), [])

    def test_every_gating_guard_has_a_plant_or_a_recorded_reason(self) -> None:
        for guard in harness.GUARDS:
            if guard.status != harness.GATING:
                continue
            with self.subTest(guard=guard.script):
                self.assertTrue(
                    guard.plant is not None or guard.reason,
                    "a gating guard with neither a plant nor a reason is an "
                    "unproved claim",
                )

    def test_every_dormant_guard_records_why(self) -> None:
        for guard in harness.GUARDS:
            if guard.status == harness.DORMANT:
                with self.subTest(guard=guard.script):
                    self.assertTrue(guard.reason.strip())


class TheControlIsItselfWiredIn(unittest.TestCase):
    """The one assertion the harness cannot make about itself.

    A guard that nothing invokes is the exact defect this work found in
    `scripts/test_critical_path_budget.py`: a test file that had been failing
    three of its own tests for as long as anyone could measure, because no
    recipe and no workflow ever ran it. Shipping the harness without wiring it
    in would reproduce that defect one directory over.

    The harness cannot check its own invocation -- if the line is deleted, the
    harness does not run to complain. So the check lives here, in the file the
    same recipe runs immediately before it.
    """

    @staticmethod
    def _check_recipe() -> str:
        body = harness._justfile_recipe_body(Path("justfile").read_text(), "check")
        assert body is not None, "justfile has no `check` recipe"
        return body

    def test_the_harness_runs_in_just_check(self) -> None:
        self.assertIn("scripts/check_guard_nonvacuity.py", self._check_recipe())

    def test_this_test_file_runs_in_just_check(self) -> None:
        """Otherwise the assertion above is itself never evaluated."""

        self.assertIn("tests.test_guard_nonvacuity", self._check_recipe())

    def test_the_harness_does_not_claim_to_be_a_protected_path(self) -> None:
        """It is not one, and a control must not overstate its own standing.

        `scripts/check_guard_nonvacuity.py` is not in the ruleset's protected
        paths, so editing it needs no maintenance window. An earlier draft of
        the failure message said otherwise. A guard that misdescribes the
        process protecting it teaches the reader to trust a step that is not
        there, which is how D034's claim outlived D034's behaviour.
        """

        source = Path("scripts/check_guard_nonvacuity.py").read_text()
        offenders = [
            line.strip()
            for line in source.splitlines()
            if "protected path" in line or "protected file" in line
        ]
        self.assertEqual(offenders, [], "\n".join(offenders))


class TheD034WeakeningIsCaught(unittest.TestCase):
    """The whole point of the control, proved against the real guard source."""

    WEAKENING = (
        "if value > REPOSITORY_CEILINGS[key]",
        "if value > REPOSITORY_CEILINGS[key] * 2",
    )

    def _mutated_guard(self, tmp: str):
        source_path = REPO_ROOT / "scripts" / "check_critical_path_budget.py"
        source = source_path.read_text(encoding="utf-8")
        original, weakened = self.WEAKENING
        self.assertIn(
            original,
            source,
            "the guard no longer contains the comparison D034 weakened; this "
            "test is pinned to that line and must be updated deliberately",
        )
        target = Path(tmp) / "weakened_budget.py"
        target.write_text(source.replace(original, weakened), encoding="utf-8")
        return _load_from(target, "weakened_budget")

    def test_the_weakened_comparison_stops_rejecting_the_plant(self) -> None:
        """For every non-zero ceiling, the weakened guard waves the plant through."""

        with tempfile.TemporaryDirectory() as tmp:
            weakened = self._mutated_guard(tmp)
            nonzero = {
                k: v + 1
                for k, v in weakened.REPOSITORY_CEILINGS.items()
                if v > 0
            }
            self.assertTrue(nonzero, "no non-zero ceilings left to plant against")
            regressions = weakened.repository_trend_regressions(nonzero)

        self.assertEqual(
            regressions,
            [],
            "the weakened guard still rejected the plant, so this test is not "
            "exercising the weakening it claims to",
        )

    def test_a_zero_ceiling_survives_the_weakening_by_accident(self) -> None:
        """The hole has a shape, and it is worth stating precisely.

        `* 2` is not a uniform bypass: a ceiling recorded as 0 doubles to 0, so
        the first regression past it still trips. The weakening hides growth
        only above ceilings that are already non-zero. This is luck, not
        design -- it depends entirely on which ceilings happen to sit at zero
        today, and it disappears the moment one of them is raised. It is
        recorded here so nobody reads the partial protection as intentional.
        """

        guard = harness._load("scripts/check_critical_path_budget.py")
        zeros = {k for k, v in guard.REPOSITORY_CEILINGS.items() if v == 0}
        self.assertTrue(zeros, "no zero-valued ceilings; this test is vacuous")

        with tempfile.TemporaryDirectory() as tmp:
            weakened = self._mutated_guard(tmp)
            planted = {k: 1 for k in zeros}
            self.assertEqual(
                len(weakened.repository_trend_regressions(planted)), len(zeros)
            )

    def test_the_unweakened_comparison_does_reject_the_plant(self) -> None:
        guard = harness._load("scripts/check_critical_path_budget.py")
        over = {k: v + 1 for k, v in guard.REPOSITORY_CEILINGS.items()}
        self.assertEqual(len(guard.repository_trend_regressions(over)), len(over))

    def test_the_digest_pin_does_not_move_under_the_weakening(self) -> None:
        """Why the digest alone cannot catch this.

        `--expect-digest` hashes `pinned_data()`, which covers ceilings and
        scope, not comparison logic. The weakening leaves the digest identical,
        which is exactly why D034's scenario slips through `just check`.
        """

        guard = harness._load("scripts/check_critical_path_budget.py")
        with tempfile.TemporaryDirectory() as tmp:
            weakened = self._mutated_guard(tmp)
            self.assertEqual(guard.pinned_data(), weakened.pinned_data())

    def test_the_harness_goes_red_against_the_weakened_guard(self) -> None:
        """End to end: the exact D034 edit, and the harness reports it.

        This is the assertion the whole control exists to support. Everything
        else is scaffolding. The real plant runs against the real guard source
        with the real weakening applied, and the verdict must flip from held to
        failed. The plant survives partially -- zero-valued ceilings still trip
        the doubled comparison -- so `rejected` rests on the count matching,
        not on any regression at all being reported.
        """

        with tempfile.TemporaryDirectory() as tmp:
            weakened = self._mutated_guard(tmp)
            original = harness._load
            harness._load = lambda script: (
                weakened
                if script.endswith("check_critical_path_budget.py")
                else original(script)
            )
            try:
                verdict = harness.plant_critical_path_budget()
                failures = harness._run_plant(
                    harness.Guard(
                        "scripts/check_critical_path_budget.py",
                        harness.GATING,
                        plant=harness.plant_critical_path_budget,
                    )
                )
            finally:
                harness._load = original

        self.assertFalse(
            verdict.rejected,
            "the weakened guard was still judged to reject its plant; the "
            "control would stay green through the D034 edit",
        )
        self.assertEqual(len(failures), 1)
        self.assertIn("ACCEPTED a planted defect", failures[0])

    def test_the_harness_is_green_against_the_unweakened_guard(self) -> None:
        """The other direction, so the red above is not merely a broken plant."""

        self.assertEqual(
            harness._run_plant(
                harness.Guard(
                    "scripts/check_critical_path_budget.py",
                    harness.GATING,
                    plant=harness.plant_critical_path_budget,
                )
            ),
            [],
        )


class TheProductionFilterClaim(unittest.TestCase):
    """The classifier that decides what the budget guard measures.

    Registered because weakening it is invisible to the guard: measured, making
    every file containing `#[cfg(test)]` yield no production lines drops
    lifecycle panics 11 -> 0 and swallowed errors 440 -> 91 while the digest,
    the in-scope file counts and the exit status all stay put.
    """

    def test_the_sample_separates_production_from_test_panics(self) -> None:
        module = harness._load("scripts/rust_production_filter.py")
        lines = module.production_lines_from_text(harness._RUST_SAMPLE)
        self.assertEqual(sum(1 for line in lines if "panic!" in line), 2)

    def test_the_claim_holds_against_the_current_filter(self) -> None:
        outcome = harness.plant_production_filter()
        self.assertTrue(outcome.accepted, outcome.detail)
        self.assertTrue(outcome.rejected, outcome.detail)

    def test_a_widened_exclusion_is_reported(self) -> None:
        """The real weakening, applied to a copy, must break the claim."""

        source = (
            harness.REPO_ROOT / "scripts/rust_production_filter.py"
        ).read_text(encoding="utf-8")
        weakened = source.replace(
            "def production_lines_from_text(source: str) -> list[str]:\n"
            "    masked_code = _mask_rust_non_code(source)",
            "def production_lines_from_text(source: str) -> list[str]:\n"
            "    masked_code = _mask_rust_non_code(source)\n"
            '    if "#[cfg(test)]" in masked_code:\n'
            "        return []",
            1,
        )
        self.assertNotEqual(weakened, source, "the weakening no longer applies")
        module = harness._exec_module(weakened, "_weakened_under_test")
        lines = module.production_lines_from_text(harness._RUST_SAMPLE)
        self.assertEqual(sum(1 for line in lines if "panic!" in line), 0)

    def test_an_inapplicable_weakening_fails_rather_than_passes(self) -> None:
        """If the plant cannot bite, it must not read as a held claim."""

        outcome = harness.plant_production_filter(
            source="def production_lines_from_text(source):\n    return []\n"
        )
        self.assertFalse(outcome.rejected)
        self.assertFalse(outcome.accepted)
        self.assertIn("could not apply the weakening", outcome.detail)


class TheImportShadowingClaim(unittest.TestCase):
    """Adding `scripts/<name>.py` rebinds `import <name>` for every guard.

    Measured: a `scripts/hashlib.py` returning a fixed `sha256` freezes
    `scope_digest()`, so lifecycle/swallowed_error can go 441 -> 9999 and
    `--expect-digest 5ed12e31...` still passes. It defeats the pin itself,
    which is the only mechanism that otherwise forces a ceiling change through
    review.
    """

    def test_the_tree_is_clean_today(self) -> None:
        self.assertEqual(harness._check_no_import_shadowing(), [])

    def test_a_shadow_of_a_real_import_is_reported(self) -> None:
        with _shadow("hashlib"):
            failures = harness._check_no_import_shadowing()
        self.assertEqual(len(failures), 1, failures)
        self.assertIn("scripts/hashlib.py", failures[0])
        self.assertIn("check_critical_path_budget.py", failures[0])

    def test_an_intended_local_import_is_not_reported(self) -> None:
        """rust_production_filter is a real local module, not a shadow."""

        self.assertIn(
            "rust_production_filter", harness.INTENDED_LOCAL_IMPORTS
        )
        self.assertEqual(harness._check_no_import_shadowing(), [])

    def test_a_shadow_of_an_unimported_name_is_not_reported(self) -> None:
        """The claim is about names guards actually import, not every file."""

        with _shadow("zoneinfo"):
            self.assertEqual(harness._check_no_import_shadowing(), [])

    def test_one_file_is_reported_once(self) -> None:
        """Two registry entries share the budget guard's file."""

        scripts = [
            g.script
            for g in harness.GUARDS
            if g.script.startswith("scripts/check_critical_path_budget.py")
        ]
        self.assertGreater(len(scripts), 1, "precondition: the file has 2 entries")
        with _shadow("hashlib"):
            self.assertEqual(len(harness._check_no_import_shadowing()), 1)


@contextlib.contextmanager
def _shadow(name: str):
    """Create scripts/<name>.py for the duration of the block."""

    path = harness.REPO_ROOT / "scripts" / f"{name}.py"
    if path.exists():
        raise AssertionError(f"{path} already exists; refusing to clobber it")
    path.write_text("# planted shadow\n", encoding="utf-8")
    try:
        yield
    finally:
        path.unlink()


class TheHarnessGoesRed(unittest.TestCase):
    """Each failure mode the harness claims to detect, observed red."""

    def _guard(self, **kwargs) -> harness.Guard:
        defaults = dict(script="fake", status=harness.GATING)
        defaults.update(kwargs)
        return harness.Guard(**defaults)

    def test_a_guard_that_accepts_its_plant_is_reported(self) -> None:
        guard = self._guard(
            plant=lambda: harness.Outcome(
                rejected=False, accepted=True, evidence="", detail="d"
            )
        )
        failures = harness._run_plant(guard)
        self.assertEqual(len(failures), 1)
        self.assertIn("ACCEPTED a planted defect", failures[0])

    def test_a_guard_that_rejects_the_clean_control_is_reported(self) -> None:
        guard = self._guard(
            plant=lambda: harness.Outcome(
                rejected=True, accepted=False, evidence="e", detail="d"
            )
        )
        failures = harness._run_plant(guard)
        self.assertEqual(len(failures), 1)
        self.assertIn("REJECTED the clean control", failures[0])

    def test_a_rejection_carrying_no_evidence_is_not_proof(self) -> None:
        """The F23 lesson: a bare non-zero outcome can be a crash."""

        guard = self._guard(
            plant=lambda: harness.Outcome(
                rejected=True, accepted=True, evidence="   ", detail="d"
            )
        )
        failures = harness._run_plant(guard)
        self.assertEqual(len(failures), 1)
        self.assertIn("no message", failures[0])

    def test_a_plant_that_raises_fails_rather_than_passes(self) -> None:
        def boom() -> harness.Outcome:
            raise RuntimeError("planted crash")

        failures = harness._run_plant(self._guard(plant=boom))
        self.assertEqual(len(failures), 1)
        self.assertIn("raised instead of returning a verdict", failures[0])
        self.assertIn("planted crash", failures[0])

    def test_unplugging_a_guard_from_its_justfile_recipe_is_reported(self) -> None:
        guard = self._guard(
            wiring=(
                harness.Wiring(
                    where="justfile",
                    recipe="check",
                    must_contain="scripts/this_is_not_invoked_anywhere.py",
                ),
            )
        )
        failures = harness._check_wiring(guard)
        self.assertEqual(len(failures), 1)
        self.assertIn("unplugged from CI", failures[0])

    def test_moving_an_invocation_out_of_the_named_recipe_is_reported(self) -> None:
        """A whole-file grep would miss this; the recipe-scoped check does not."""

        guard = self._guard(
            wiring=(
                harness.Wiring(
                    where="justfile",
                    recipe="lint-docs",
                    must_contain="scripts/check_critical_path_budget.py",
                ),
            )
        )
        failures = harness._check_wiring(guard)
        self.assertEqual(len(failures), 1)
        self.assertIn("no longer invokes", failures[0])

    def test_a_missing_recipe_is_reported(self) -> None:
        guard = self._guard(
            wiring=(
                harness.Wiring(
                    where="justfile", recipe="no-such-recipe", must_contain="x"
                ),
            )
        )
        failures = harness._check_wiring(guard)
        self.assertEqual(len(failures), 1)
        self.assertIn("no `no-such-recipe` recipe", failures[0])

    def test_a_missing_wiring_file_is_reported(self) -> None:
        guard = self._guard(
            wiring=(harness.Wiring(where=".github/workflows/gone.yml", must_contain="x"),)
        )
        failures = harness._check_wiring(guard)
        self.assertEqual(len(failures), 1)
        self.assertIn("is missing", failures[0])

    def test_an_unregistered_guard_script_is_reported(self) -> None:
        planted = REPO_ROOT / "scripts" / "check_planted_unregistered_guard.py"
        self.assertFalse(planted.exists(), "leftover from a previous run")
        planted.write_text("#!/usr/bin/env python3\n", encoding="utf-8")
        try:
            failures = harness._check_registry_covers_every_guard()
        finally:
            planted.unlink()
        self.assertEqual(len(failures), 1)
        self.assertIn("not registered", failures[0])

    def test_a_registered_guard_that_vanished_is_reported(self) -> None:
        original = harness.GUARDS
        harness.GUARDS = original + (
            harness.Guard("scripts/check_deleted_guard.py", harness.DORMANT, reason="r"),
        )
        try:
            failures = harness._check_registry_covers_every_guard()
        finally:
            harness.GUARDS = original
        self.assertEqual(len(failures), 1)
        self.assertIn("no longer exists", failures[0])

    def test_an_unknown_status_is_reported(self) -> None:
        original = harness.GUARDS
        harness.GUARDS = original + (
            harness.Guard("scripts/check_panic_budget.py", "advisory", reason="r"),
        )
        try:
            failures, _ = harness.run()
        finally:
            harness.GUARDS = original
        self.assertTrue(any("unknown status" in f for f in failures), failures)


class TheHarnessExitCodesAreDistinguishable(unittest.TestCase):
    """`main` must never let an internal error read as a passing run."""

    @staticmethod
    def _quietly(argv: list[str]) -> int:
        """Run `main` with its report suppressed; only the exit code is under test."""

        buffer = io.StringIO()
        with contextlib.redirect_stdout(buffer), contextlib.redirect_stderr(buffer):
            return harness.main(argv)

    def test_a_held_claim_exits_zero(self) -> None:
        self.assertEqual(self._quietly([]), 0)

    def test_a_failed_claim_exits_one(self) -> None:
        original = harness.run
        harness.run = lambda: (["planted failure"], [])
        try:
            self.assertEqual(self._quietly([]), 1)
        finally:
            harness.run = original

    def test_an_internal_error_exits_two_not_zero(self) -> None:
        original = harness.run

        def boom():
            raise RuntimeError("planted harness error")

        harness.run = boom
        try:
            self.assertEqual(self._quietly([]), 2)
        finally:
            harness.run = original

    def test_list_mode_does_not_report_success_for_unrun_claims(self) -> None:
        self.assertEqual(self._quietly(["--list"]), 0)


if __name__ == "__main__":
    unittest.main()
