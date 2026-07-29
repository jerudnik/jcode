#!/usr/bin/env python3
"""Deterministic tests for the critical-path budget checker.

These cover the parts that a planted-defect run cannot observe cheaply: the
domain attribution rules, the digest's sensitivity to every field it claims to
pin, the repository-trend comparison, and the coherence of the recorded
ceilings and targets. The end-to-end red/green behaviour is proved separately by
the planted defects recorded in docs/fork/ideal-base/evidence/F23/README.md.
"""

from __future__ import annotations

import json
import sys
import unittest
from pathlib import Path

SCRIPTS_DIR = Path(__file__).resolve().parent
REPO_ROOT = SCRIPTS_DIR.parent
sys.path.insert(0, str(SCRIPTS_DIR))

import check_critical_path_budget as budget  # noqa: E402


class DomainAttributionTests(unittest.TestCase):
    def test_directory_prefixes_match_descendants(self) -> None:
        self.assertEqual(
            budget.domain_for("crates/jcode-app-core/src/server/shutdown.rs"), "lifecycle"
        )
        self.assertEqual(budget.domain_for("crates/jcode-tui/src/tui/app/mod.rs"), "tui")

    def test_file_prefixes_match_exactly_not_by_stem(self) -> None:
        self.assertEqual(budget.domain_for("crates/jcode-app-core/src/update.rs"), "updater")
        # A sibling that merely shares the prefix string must not be captured.
        self.assertIsNone(budget.domain_for("crates/jcode-app-core/src/update_helpers.rs"))

    def test_out_of_scope_paths_are_unattributed(self) -> None:
        for path in (
            "crates/jcode-fuzzy/src/lib.rs",
            "crates/jcode-provider-openai/src/lib.rs",
            "crates/jcode-desktop/src/main.rs",
        ):
            self.assertIsNone(budget.domain_for(path), path)

    def test_vendor_adapters_are_excluded_from_provider_infrastructure(self) -> None:
        # The exclusion is the documented scope boundary, so pin it: a prefix
        # typo like "crates/jcode-provider-" would silently swallow every vendor
        # adapter and make the ceiling meaningless.
        self.assertIsNone(budget.domain_for("crates/jcode-provider-anthropic/src/lib.rs"))
        self.assertEqual(budget.domain_for("crates/jcode-provider-core/src/transport.rs"), "provider_infrastructure")

    def test_each_domain_prefix_resolves_to_its_own_domain(self) -> None:
        # Prefixes are matched in declaration order, so an overlap would make an
        # earlier domain shadow a later one and silently move debt.
        for domain, prefixes in budget.CRITICAL_PATHS.items():
            for prefix in prefixes:
                probe = prefix + "probe.rs" if prefix.endswith("/") else prefix
                self.assertEqual(budget.domain_for(probe), domain, probe)

    def test_every_declared_prefix_exists_in_the_tree(self) -> None:
        for domain, prefixes in budget.CRITICAL_PATHS.items():
            for prefix in prefixes:
                self.assertTrue(
                    (REPO_ROOT / prefix.rstrip("/")).exists(),
                    f"{domain} names a path that does not exist: {prefix}",
                )


class PinnedBlockTests(unittest.TestCase):
    def test_digest_is_stable_across_calls(self) -> None:
        self.assertEqual(budget.scope_digest(), budget.scope_digest())

    def test_digest_covers_every_field_it_claims_to_pin(self) -> None:
        pinned = budget.pinned_data()
        self.assertEqual(set(pinned), set(budget.DIGEST_FIELDS))

    def test_digest_changes_when_any_pinned_field_changes(self) -> None:
        original = budget.scope_digest()
        mutations = {
            "oversize_threshold_loc": lambda: setattr(
                budget, "OVERSIZE_THRESHOLD_LOC", budget.OVERSIZE_THRESHOLD_LOC + 1
            ),
            "critical_paths": lambda: budget.CRITICAL_PATHS["tui"].append("crates/jcode-fuzzy/"),
            "ceilings": lambda: budget.CEILINGS["tui"].__setitem__("panic", 999),
            "targets": lambda: budget.TARGETS["tui"].__setitem__("panic", 999),
            "repository_ceilings": lambda: budget.REPOSITORY_CEILINGS.__setitem__("panic", 999),
        }
        for field, mutate in mutations.items():
            snapshot = json.dumps(budget.pinned_data(), sort_keys=True)
            mutate()
            try:
                self.assertNotEqual(
                    original, budget.scope_digest(), f"digest ignores changes to {field}"
                )
            finally:
                self._restore(snapshot)
            self.assertEqual(original, budget.scope_digest())

    @staticmethod
    def _restore(snapshot: str) -> None:
        data = json.loads(snapshot)
        budget.OVERSIZE_THRESHOLD_LOC = data["oversize_threshold_loc"]
        budget.CRITICAL_PATHS.clear()
        budget.CRITICAL_PATHS.update(data["critical_paths"])
        budget.CEILINGS.clear()
        budget.CEILINGS.update(data["ceilings"])
        budget.TARGETS.clear()
        budget.TARGETS.update(data["targets"])
        budget.REPOSITORY_CEILINGS.clear()
        budget.REPOSITORY_CEILINGS.update(data["repository_ceilings"])

    def test_workflow_pin_matches_the_current_digest(self) -> None:
        workflow = (REPO_ROOT / ".github" / "workflows" / "fork-ci.yml").read_text(encoding="utf-8")
        self.assertIn(
            f"--expect-digest {budget.scope_digest()}",
            workflow,
            "fork-ci.yml pin is stale; refresh with --print-digest inside a maintenance window",
        )


class CeilingAndTargetCoherenceTests(unittest.TestCase):
    def test_every_domain_has_a_ceiling_and_a_target(self) -> None:
        self.assertEqual(set(budget.CRITICAL_PATHS), set(budget.CEILINGS))
        self.assertEqual(set(budget.CRITICAL_PATHS), set(budget.TARGETS))

    def test_targets_are_strictly_downward(self) -> None:
        for domain in budget.CRITICAL_PATHS:
            for dim in budget.DIMENSIONS:
                self.assertLessEqual(
                    budget.TARGETS[domain][dim],
                    budget.CEILINGS[domain][dim],
                    f"{domain}/{dim} target is above its ceiling, which is not a downward target",
                )

    def test_at_least_one_target_is_strictly_below_its_ceiling(self) -> None:
        # A target set equal to the ceiling everywhere would satisfy the check
        # above while demanding nothing, so require real downward intent.
        self.assertTrue(
            any(
                budget.TARGETS[d][dim] < budget.CEILINGS[d][dim]
                for d in budget.CRITICAL_PATHS
                for dim in budget.DIMENSIONS
            )
        )

    def test_every_target_has_a_rationale(self) -> None:
        for domain, target in budget.TARGETS.items():
            self.assertTrue(target.get("rationale", "").strip(), f"{domain} target lacks rationale")

    def test_ceilings_are_non_negative(self) -> None:
        for domain in budget.CEILINGS:
            for dim in budget.DIMENSIONS:
                self.assertGreaterEqual(budget.CEILINGS[domain][dim], 0)


class RepositoryTrendTests(unittest.TestCase):
    def test_recorded_baselines_are_at_or_below_their_marks(self) -> None:
        self.assertEqual(budget.repository_trend_regressions(budget.repository_totals()), [])

    def test_a_raised_baseline_is_reported_as_a_regression(self) -> None:
        raised = dict(budget.repository_totals())
        raised["panic"] += 1
        regressions = budget.repository_trend_regressions(raised)
        self.assertEqual(len(regressions), 1)
        self.assertIn("panic", regressions[0])

    def test_a_lowered_baseline_is_not_a_regression(self) -> None:
        lowered = {key: max(0, value - 1) for key, value in budget.repository_totals().items()}
        self.assertEqual(budget.repository_trend_regressions(lowered), [])

    def test_marks_cover_every_measured_repository_key(self) -> None:
        self.assertEqual(set(budget.repository_totals()), set(budget.REPOSITORY_CEILINGS))


class OversizeThresholdTests(unittest.TestCase):
    def test_pinned_threshold_matches_the_unprotected_baseline(self) -> None:
        baseline = json.loads(
            (REPO_ROOT / "scripts" / "code_size_budget.json").read_text(encoding="utf-8")
        )
        self.assertEqual(baseline["threshold_loc"], budget.OVERSIZE_THRESHOLD_LOC)


class ScopeShrinkTests(unittest.TestCase):
    """A scope shrink must not be indistinguishable from a cleanup.

    Moving a file out of a critical directory removes its debt from the domain.
    A count-only gate reads that as progress: an independent review moved
    server/shutdown.rs out of the critical set, lifecycle/panic fell 11 -> 3,
    and the gate passed while praising the drop as headroom.
    """

    def test_matching_counts_are_not_a_regression(self) -> None:
        self.assertEqual(
            budget.scope_shrink_regressions(dict(budget.EXPECTED_FILE_COUNTS)), []
        )

    def test_a_lost_file_is_a_regression(self) -> None:
        counts = dict(budget.EXPECTED_FILE_COUNTS)
        counts["lifecycle"] -= 1
        regressions = budget.scope_shrink_regressions(counts)
        self.assertEqual(len(regressions), 1)
        self.assertIn("lifecycle", regressions[0])
        self.assertIn("lost in-scope production files", regressions[0])

    def test_added_files_are_not_a_regression(self) -> None:
        # Growth is normal work; the debt it brings is still bounded by the
        # ceilings, so only a decrease is gated.
        counts = dict(budget.EXPECTED_FILE_COUNTS)
        counts["tui"] += 25
        self.assertEqual(budget.scope_shrink_regressions(counts), [])

    def test_every_domain_is_covered(self) -> None:
        self.assertEqual(
            set(budget.EXPECTED_FILE_COUNTS), set(budget.CRITICAL_PATHS)
        )

    def test_expected_counts_match_the_current_tree(self) -> None:
        measurement = budget.measure()
        self.assertEqual(
            dict(measurement.file_counts), dict(budget.EXPECTED_FILE_COUNTS)
        )

    def test_expected_counts_sum_to_the_scanned_total(self) -> None:
        measurement = budget.measure()
        self.assertEqual(sum(budget.EXPECTED_FILE_COUNTS.values()), measurement.scanned)

    def test_counts_are_pinned_by_the_digest(self) -> None:
        # Without this the counts could be edited freely, since the workflow
        # only pins the digest.
        self.assertIn("expected_file_counts", budget.DIGEST_FIELDS)
        before = budget.scope_digest()
        original = budget.EXPECTED_FILE_COUNTS["updater"]
        budget.EXPECTED_FILE_COUNTS["updater"] = original + 1
        try:
            self.assertNotEqual(before, budget.scope_digest())
        finally:
            budget.EXPECTED_FILE_COUNTS["updater"] = original


class SelfProtectionTests(unittest.TestCase):
    """This checker must be a protected path, like every checker it sits beside.

    A digest pin cannot substitute for protection: the digest lives in the
    workflow, but the code that verifies it lives in this script, so an edit
    that both raises a ceiling and disables the comparison is self-consistent.
    An independent review did exactly that in two lines and CI stayed green.
    """

    SCRIPTS = (
        "scripts/check_critical_path_budget.py",
        "scripts/test_critical_path_budget.py",
    )

    def test_manifest_protects_this_checker_and_its_tests(self) -> None:
        manifest = json.loads(
            (REPO_ROOT / "scripts" / "required-checks.json").read_text(encoding="utf-8")
        )
        required = set(manifest["protected_paths"]["required"])
        for path in self.SCRIPTS:
            self.assertIn(path, required, f"{path} is not a protected path")

    def test_governance_root_workflow_names_this_checker(self) -> None:
        text = (
            REPO_ROOT / ".github" / "workflows" / "governance-root.yml"
        ).read_text(encoding="utf-8")
        for path in self.SCRIPTS:
            self.assertIn(path, text, f"governance-root.yml does not name {path}")

    def test_repository_marks_are_not_derived_from_the_baselines(self) -> None:
        """The pinned marks must stay pinned; deriving them makes the gate vacuous.

        `repository_totals()` reads the live ratchet baselines. If
        REPOSITORY_CEILINGS were derived from the same source - a change that
        looks like a cleanup, and that the module comment once implied was
        already the case - every comparison becomes `value > value`. This test
        demonstrates that failure mode directly rather than describing it: under
        derived marks, even doubled debt reports no breach.
        """

        live = {"panic": 10, "swallowed_error": 100, "oversize_files": 5,
                "oversize_total_loc": 1000, "test_oversize_files": 3, "warnings": 0}
        doubled = {key: value * 2 for key, value in live.items()}

        original = budget.REPOSITORY_CEILINGS
        try:
            budget.REPOSITORY_CEILINGS = dict(live)
            self.assertEqual(
                budget.repository_trend_regressions(doubled) != [],
                True,
                "pinned marks must catch a doubling of recorded debt",
            )
            # Now the vacuous form: marks derived from the same live numbers.
            budget.REPOSITORY_CEILINGS = dict(doubled)
            self.assertEqual(
                budget.repository_trend_regressions(doubled),
                [],
                "derived marks report no breach even at doubled debt, which is "
                "exactly why the marks are pinned literals",
            )
        finally:
            budget.REPOSITORY_CEILINGS = original

    def test_recorded_marks_are_literal_ints_not_computed(self) -> None:
        # Guards the property structurally: a future edit that swaps the literal
        # dict for a call to repository_totals() fails here, not silently.
        source = (REPO_ROOT / "scripts" / "check_critical_path_budget.py").read_text(
            encoding="utf-8"
        )
        marker = "REPOSITORY_CEILINGS: dict[str, int] = {"
        self.assertIn(marker, source)
        block = source.split(marker, 1)[1].split("}", 1)[0]
        self.assertNotIn("repository_totals", block)
        self.assertNotIn("load_json", block)
        for key in budget.REPOSITORY_CEILINGS:
            self.assertIn(f'"{key}"', block)

    def test_protection_matches_a_sibling_checker(self) -> None:
        # Anchors the assertion to how the repo already protects an equivalent
        # gate, so this cannot pass by protecting nothing.
        manifest = json.loads(
            (REPO_ROOT / "scripts" / "required-checks.json").read_text(encoding="utf-8")
        )
        required = set(manifest["protected_paths"]["required"])
        self.assertIn("scripts/check_panic_budget.py", required)


if __name__ == "__main__":
    unittest.main()
