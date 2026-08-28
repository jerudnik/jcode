#!/usr/bin/env python3
"""Deterministic tests for the critical-path budget checker.

These cover the parts that a planted-defect run cannot observe cheaply: the
domain attribution rules, the digest's sensitivity to every field it claims to
pin, the repository-trend comparison, and the coherence of the recorded
ceilings and targets. The end-to-end red/green behaviour is proved separately
by planted-defect runs.
"""

from __future__ import annotations

import json
import sys
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent

# Borrowed, not donated. Leaving scripts/ on sys.path re-creates the shadowing
# hazard the guards are hardened against, and it leaks into every module that
# runs after this one. Append rather than insert so the standard library keeps
# precedence even inside the window.
_SCRIPTS_DIR = str(REPO_ROOT / "scripts")
_BORROWED_PATH_ENTRY = _SCRIPTS_DIR not in sys.path
if _BORROWED_PATH_ENTRY:
    sys.path.append(_SCRIPTS_DIR)
try:
    import check_critical_path_budget as budget
finally:
    if _BORROWED_PATH_ENTRY:
        sys.path.remove(_SCRIPTS_DIR)


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

    def test_check_recipe_pin_matches_the_current_digest(self) -> None:
        recipe = (REPO_ROOT / "justfile").read_text(encoding="utf-8")
        self.assertIn(
            f"--expect-digest {budget.scope_digest()}",
            recipe,
            "just check critical-path pin is stale; refresh with --print-digest after reviewing the scope change",
        )

    def test_pr_gate_runs_the_pinned_check_recipe(self) -> None:
        pr_workflow = (REPO_ROOT / ".github" / "workflows" / "pr.yml").read_text(encoding="utf-8")
        ci_workflow = (REPO_ROOT / ".github" / "workflows" / "ci.yml").read_text(encoding="utf-8")
        fork_workflow = (REPO_ROOT / ".github" / "workflows" / "fork-ci.yml").read_text(encoding="utf-8")
        self.assertIn("name: PR Gate", pr_workflow)
        self.assertIn("uses: ./.github/workflows/ci.yml", pr_workflow)
        self.assertIn("uses: ./.github/workflows/fork-ci.yml", ci_workflow)
        self.assertIn("nix shell nixpkgs#just nixpkgs#python3 -c just check", fork_workflow)


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
    def test_measured_tree_is_at_or_below_the_pinned_marks(self) -> None:
        # The one full-scan test in this module: the actual tree must sit at or
        # below every pinned high-water mark, which is what the wired gate
        # enforces on every run.
        totals = budget.repository_totals(budget.measure())
        self.assertEqual(budget.repository_trend_regressions(totals), [])

    def test_a_total_above_its_mark_is_reported_as_a_regression(self) -> None:
        raised = {key: 0 for key in budget.REPOSITORY_CEILINGS}
        raised["panic"] = budget.REPOSITORY_CEILINGS["panic"] + 1
        regressions = budget.repository_trend_regressions(raised)
        self.assertEqual(len(regressions), 1)
        self.assertIn("panic", regressions[0])

    def test_a_total_below_its_mark_is_not_a_regression(self) -> None:
        lowered = {key: 0 for key in budget.REPOSITORY_CEILINGS}
        self.assertEqual(budget.repository_trend_regressions(lowered), [])

    def test_marks_cover_every_measured_repository_key(self) -> None:
        # An unmeasured Measurement still has the full key shape, plus the
        # warnings key repository_totals() adds from the recorded budget.
        totals = budget.repository_totals(budget.Measurement())
        self.assertEqual(set(totals), set(budget.REPOSITORY_CEILINGS))


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


class CriticalPathGateContractTests(unittest.TestCase):
    """The critical-path checker must remain part of the accepted PR Gate route."""

    def test_required_manifest_names_single_pr_gate_context(self) -> None:
        manifest = json.loads(
            (REPO_ROOT / "scripts" / "required-checks.json").read_text(encoding="utf-8")
        )
        contexts = [entry["context"] for entry in manifest["required_checks"]]
        self.assertEqual(contexts, ["PR Gate"])

    def test_ci_contract_keeps_one_pr_gate_entrypoint(self) -> None:
        pr_workflow = (REPO_ROOT / ".github" / "workflows" / "pr.yml").read_text(encoding="utf-8")
        self.assertIn("pr-gate:", pr_workflow)
        self.assertIn("name: PR Gate", pr_workflow)
        self.assertIn("uses: ./.github/workflows/ci.yml", pr_workflow)

    def test_repository_marks_are_not_derived_from_the_baselines(self) -> None:
        """The pinned marks must stay pinned; deriving them makes the gate vacuous.

        `repository_totals()` measures the working tree. If
        REPOSITORY_CEILINGS were derived from the same measurement - a change
        that looks like a cleanup - every comparison becomes `value > value`.
        This test demonstrates that failure mode directly rather than
        describing it: under derived marks, even doubled debt reports no
        breach.
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

    def test_critical_path_checker_runs_with_sibling_ratchets(self) -> None:
        # Anchors the assertion to the maintained local recipe PR Gate executes,
        # not to retired branch-protection path lists.
        recipe = (REPO_ROOT / "justfile").read_text(encoding="utf-8")
        self.assertIn("scripts/check_critical_path_budget.py", recipe)
        self.assertIn("scripts/cargo_exec.sh check", recipe)


if __name__ == "__main__":
    unittest.main()
