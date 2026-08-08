#!/usr/bin/env python3
"""Planted-failure tests for the governance comparator (R07 design.md section 7).

The comparator is a drift detector, and D029's standing rule is that a detector
is not trusted until it has been observed red. So almost every test here takes
the valid fixture, mutates exactly one property, and asserts a specific non-zero
exit with a diagnostic that names the thing that changed. A test that only
asserted "nonzero" would pass against a comparator that failed for an unrelated
reason, which is the failure mode these tests exist to rule out.

Live-mode tests use a generated `gh` shim rather than the real API. The shim
serves a table of path -> response, so a test can fail one endpoint, omit one
key, or return a mutated body while everything else stays valid. This is what
makes "insufficient authorization is not an empty bypass list" and "a mutated
live surface is observed red" testable without network access or credentials.

Run:  python3 -m unittest tests.test_governance_compare
"""

from __future__ import annotations

import json
import os
import re
import stat
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from scripts.generate_governance_fixture import build as build_fixture

REPO_ROOT = Path(__file__).resolve().parent.parent
SCRIPTS = REPO_ROOT / "scripts"
MANIFEST = SCRIPTS / "required-checks.json"
COMPARATOR = SCRIPTS / "governance_compare.py"
FORK_HEALTH = SCRIPTS / "fork-health.sh"

EXIT_OK = 0
EXIT_MISMATCH = 1
EXIT_ACQUISITION = 2
# Same exit code: section 6 folds "could not read" and "manifest is malformed"
# into one unclassifiable outcome, which never reads as a pass.
EXIT_SCHEMA = 2


def load_fixture() -> dict:
    return build_fixture(load_manifest(), REPO_ROOT / ".github" / "workflows")


def load_manifest() -> dict:
    return json.loads(MANIFEST.read_text(encoding="utf-8"))


def ruleset(snapshot: dict, name: str) -> dict:
    for body in snapshot["rulesets"]:
        if body["name"] == name:
            return body
    raise AssertionError(f"fixture has no ruleset named {name!r}")


def rule(snapshot: dict, ruleset_name: str, rule_type: str) -> dict:
    for entry in ruleset(snapshot, ruleset_name)["rules"]:
        if entry["type"] == rule_type:
            return entry
    raise AssertionError(f"ruleset {ruleset_name!r} has no rule {rule_type!r}")


def drop_rule(snapshot: dict, ruleset_name: str, rule_type: str) -> None:
    body = ruleset(snapshot, ruleset_name)
    body["rules"] = [r for r in body["rules"] if r["type"] != rule_type]
    if ruleset_name == "protect-fork-rails":
        snapshot["effective_main_rules"] = [
            r for r in snapshot["effective_main_rules"] if r["type"] != rule_type
        ]


class ComparatorCase(unittest.TestCase):
    """Runs the comparator against a snapshot written to a temporary file."""

    maxDiff = None

    def run_snapshot(self, snapshot: dict, *, manifest: dict | None = None) -> subprocess.CompletedProcess:
        with tempfile.TemporaryDirectory() as tmp:
            snapshot_path = Path(tmp) / "snapshot.json"
            snapshot_path.write_text(json.dumps(snapshot), encoding="utf-8")
            manifest_path = MANIFEST
            if manifest is not None:
                manifest_path = Path(tmp) / "manifest.json"
                manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
            return subprocess.run(
                [
                    sys.executable,
                    str(COMPARATOR),
                    "--manifest",
                    str(manifest_path),
                    "--snapshot",
                    str(snapshot_path),
                ],
                capture_output=True,
                text=True,
                check=False,
            )

    def assert_rejected(self, snapshot: dict, *needles: str, manifest: dict | None = None) -> str:
        result = self.run_snapshot(snapshot, manifest=manifest)
        output = result.stdout + result.stderr
        self.assertEqual(
            result.returncode,
            EXIT_MISMATCH,
            f"expected a governance mismatch (exit 1), got {result.returncode}\n{output}",
        )
        for needle in needles:
            self.assertIn(needle, output, f"diagnostic did not mention {needle!r}\n{output}")
        return output

    def assert_schema_failure(self, snapshot: dict, *needles: str) -> str:
        result = self.run_snapshot(snapshot)
        output = result.stdout + result.stderr
        self.assertEqual(
            result.returncode,
            EXIT_ACQUISITION,
            f"expected a schema failure (exit 2), got {result.returncode}\n{output}",
        )
        for needle in needles:
            self.assertIn(needle, output, f"diagnostic did not mention {needle!r}\n{output}")
        return output


class ValidFixtureTests(ComparatorCase):
    def test_the_valid_fixture_passes(self) -> None:
        result = self.run_snapshot(load_fixture())
        self.assertEqual(
            result.returncode, EXIT_OK, f"valid fixture was rejected:\n{result.stdout}{result.stderr}"
        )
        self.assertIn("matches the manifest", result.stdout)

    def test_fixture_is_regenerable_from_the_manifest(self) -> None:
        # The expected state is generated from the manifest and live workflow
        # text, so the planted-failure tests only mutate the object the
        # comparator actually compares.
        fixture = load_fixture()
        manifest = load_manifest()
        self.assertEqual(fixture["repository"]["id"], manifest["repository_id"])
        for name, body in manifest["rulesets"].items():
            self.assertEqual(ruleset(fixture, name)["rules"], body["rules"])
            self.assertEqual(ruleset(fixture, name)["conditions"], body["conditions"])
        for context in (c["context"] for c in manifest["required_checks"]):
            required = rule(fixture, "protect-fork-rails", "required_status_checks")
            contexts = [
                c["context"] for c in required["parameters"]["required_status_checks"]
            ]
            self.assertIn(context, contexts)


class RulesetMutationTests(ComparatorCase):
    def test_missing_pull_request_enforcement(self) -> None:
        snapshot = load_fixture()
        drop_rule(snapshot, "protect-fork-rails", "pull_request")
        self.assert_rejected(snapshot, "missing required rule 'pull_request'")

    def test_wrong_required_approvals(self) -> None:
        snapshot = load_fixture()
        rule(snapshot, "protect-fork-rails", "pull_request")["parameters"][
            "required_approving_review_count"
        ] = 1
        self.assert_rejected(snapshot, "required_approving_review_count")

    def test_squash_added_to_allowed_merge_methods(self) -> None:
        snapshot = load_fixture()
        rule(snapshot, "protect-fork-rails", "pull_request")["parameters"][
            "allowed_merge_methods"
        ] = ["merge", "squash"]
        self.assert_rejected(snapshot, "allowed_merge_methods")

    def test_merge_removed_from_allowed_merge_methods(self) -> None:
        snapshot = load_fixture()
        rule(snapshot, "protect-fork-rails", "pull_request")["parameters"][
            "allowed_merge_methods"
        ] = ["rebase"]
        self.assert_rejected(snapshot, "allowed_merge_methods")

    def test_force_push_permitted(self) -> None:
        snapshot = load_fixture()
        drop_rule(snapshot, "protect-fork-rails", "non_fast_forward")
        self.assert_rejected(snapshot, "missing required rule 'non_fast_forward'")

    def test_deletion_permitted(self) -> None:
        snapshot = load_fixture()
        drop_rule(snapshot, "protect-fork-rails", "deletion")
        self.assert_rejected(snapshot, "missing required rule 'deletion'")

    def test_unresolved_review_threads_allowed(self) -> None:
        snapshot = load_fixture()
        rule(snapshot, "protect-fork-rails", "pull_request")["parameters"][
            "required_review_thread_resolution"
        ] = False
        self.assert_rejected(snapshot, "required_review_thread_resolution")

    def test_status_checks_not_strict(self) -> None:
        snapshot = load_fixture()
        rule(snapshot, "protect-fork-rails", "required_status_checks")["parameters"][
            "strict_required_status_checks_policy"
        ] = False
        self.assert_rejected(snapshot, "strict_required_status_checks_policy")

    def test_unexpected_extra_rule(self) -> None:
        snapshot = load_fixture()
        ruleset(snapshot, "protect-fork-rails")["rules"].append({"type": "required_signatures"})
        snapshot["effective_main_rules"].append({"type": "required_signatures"})
        self.assert_rejected(snapshot, "unexpected rule 'required_signatures'")

    def test_enforcement_downgraded_to_evaluate(self) -> None:
        snapshot = load_fixture()
        ruleset(snapshot, "protect-fork-rails")["enforcement"] = "evaluate"
        self.assert_rejected(snapshot, "enforcement is 'evaluate'")

    def test_enforcement_disabled(self) -> None:
        snapshot = load_fixture()
        ruleset(snapshot, "protect-fork-rails")["enforcement"] = "disabled"
        self.assert_rejected(snapshot, "enforcement is 'disabled'")

    def test_unexpected_bypass_actor_on_main_ruleset(self) -> None:
        snapshot = load_fixture()
        ruleset(snapshot, "protect-fork-rails")["bypass_actors"] = [
            {"actor_id": 5, "actor_type": "RepositoryRole", "bypass_mode": "always"}
        ]
        self.assert_rejected(snapshot, "bypass_actors")

    def test_unexpected_bypass_actor_on_non_main_ruleset(self) -> None:
        # The stale-rail ruleset is the one that actually carried a bypass actor
        # before R07, so a comparator that only inspected the main ruleset would
        # have reported green on the exact state recon found.
        snapshot = load_fixture()
        ruleset(snapshot, "no-stray-branches")["bypass_actors"] = [
            {"actor_id": 5, "actor_type": "RepositoryRole", "bypass_mode": "always"}
        ]
        self.assert_rejected(snapshot, "no-stray-branches", "bypass_actors")

    def test_stale_rail_in_ruleset_conditions(self) -> None:
        snapshot = load_fixture()
        ruleset(snapshot, "no-stray-branches")["conditions"]["ref_name"]["exclude"].append(
            "refs/heads/vendor/upstream"
        )
        self.assert_rejected(snapshot, "vendor/upstream")

    def test_distro_nix_rail_in_ruleset_conditions(self) -> None:
        snapshot = load_fixture()
        ruleset(snapshot, "no-stray-branches")["conditions"]["ref_name"]["exclude"].append(
            "refs/heads/distro/nix"
        )
        self.assert_rejected(snapshot, "distro/nix")

    def test_automation_carveout_removed(self) -> None:
        snapshot = load_fixture()
        body = ruleset(snapshot, "no-stray-branches")
        body["conditions"]["ref_name"]["exclude"] = ["refs/heads/main"]
        self.assert_rejected(snapshot, "automation/**")

    def test_automation_carveout_widened(self) -> None:
        snapshot = load_fixture()
        body = ruleset(snapshot, "no-stray-branches")
        body["conditions"]["ref_name"]["exclude"] = ["refs/heads/main", "refs/heads/**"]
        self.assert_rejected(snapshot, "no-stray-branches")

    def test_unknown_active_ruleset(self) -> None:
        snapshot = load_fixture()
        snapshot["rulesets"].append(
            {
                "name": "shadow-rules",
                "target": "branch",
                "enforcement": "active",
                "bypass_actors": [],
                "conditions": {"ref_name": {"include": ["refs/heads/main"], "exclude": []}},
                "rules": [{"type": "creation"}],
            }
        )
        self.assert_rejected(snapshot, "unknown active ruleset 'shadow-rules'")

    def test_required_ruleset_absent(self) -> None:
        snapshot = load_fixture()
        snapshot["rulesets"] = [b for b in snapshot["rulesets"] if b["name"] != "no-stray-branches"]
        self.assert_rejected(snapshot, "required ruleset 'no-stray-branches' is absent")


class RequiredContextTests(ComparatorCase):
    def test_each_required_context_removal_is_detected(self) -> None:
        for context in ("Fork CI Gate", "Security Gate", "Nix Gate"):
            with self.subTest(context=context):
                snapshot = load_fixture()
                params = rule(snapshot, "protect-fork-rails", "required_status_checks")["parameters"]
                params["required_status_checks"] = [
                    c for c in params["required_status_checks"] if c["context"] != context
                ]
                self.assert_rejected(snapshot, f"required context {context!r} is not required")

    def test_stale_context_added(self) -> None:
        snapshot = load_fixture()
        rule(snapshot, "protect-fork-rails", "required_status_checks")["parameters"][
            "required_status_checks"
        ].append({"context": "Detect changes", "integration_id": 15368})
        self.assert_rejected(snapshot, "unexpected context 'Detect changes'")

    def test_integration_id_nulled(self) -> None:
        snapshot = load_fixture()
        for entry in rule(snapshot, "protect-fork-rails", "required_status_checks")["parameters"][
            "required_status_checks"
        ]:
            if entry["context"] == "Nix Gate":
                entry["integration_id"] = None
        self.assert_rejected(snapshot, "spoofable")

    def test_integration_id_changed_to_another_app(self) -> None:
        snapshot = load_fixture()
        for entry in rule(snapshot, "protect-fork-rails", "required_status_checks")["parameters"][
            "required_status_checks"
        ]:
            if entry["context"] == "Fork CI Gate":
                entry["integration_id"] = 99999
        self.assert_rejected(snapshot, "integration_id")


class RepositoryTests(ComparatorCase):
    def test_wrong_repository_id(self) -> None:
        snapshot = load_fixture()
        snapshot["repository"]["id"] = 999
        self.assert_rejected(snapshot, "different repository")

    def test_wrong_repository_name(self) -> None:
        snapshot = load_fixture()
        snapshot["repository"]["full_name"] = "someone/else"
        self.assert_rejected(snapshot, "someone/else")

    def test_squash_merge_enabled(self) -> None:
        snapshot = load_fixture()
        snapshot["repository"]["allow_squash_merge"] = True
        self.assert_rejected(snapshot, "allow_squash_merge")

    def test_rebase_merge_enabled(self) -> None:
        snapshot = load_fixture()
        snapshot["repository"]["allow_rebase_merge"] = True
        self.assert_rejected(snapshot, "allow_rebase_merge")

    def test_merge_commit_disabled(self) -> None:
        snapshot = load_fixture()
        snapshot["repository"]["allow_merge_commit"] = False
        self.assert_rejected(snapshot, "allow_merge_commit")

    def test_classic_protection_still_present(self) -> None:
        # The recon section 2.2 shape: a weak classic layer contradicting the
        # ruleset. Deleting it is the last step of the apply, so a comparator
        # that ignored it would report green on a half-applied state.
        snapshot = load_fixture()
        snapshot["classic_branch_protection"] = {
            "url": "https://api.github.com/repos/jerudnik/jcode/branches/main/protection",
            "required_status_checks": {
                "url": "https://api.github.com/x",
                "strict": False,
                "contexts": ["Detect changes"],
                "contexts_url": "https://api.github.com/y",
            },
            "enforce_admins": {"url": "https://api.github.com/z", "enabled": False},
        }
        self.assert_rejected(snapshot, "classic branch protection still exists")

    def test_missing_maintained_rail(self) -> None:
        snapshot = load_fixture()
        snapshot["branches"] = ["automation/topic"]
        self.assert_rejected(snapshot, "missing the maintained rail 'main'")

    def test_retired_rail_returned_as_a_branch(self) -> None:
        snapshot = load_fixture()
        snapshot["branches"] = ["main", "vendor/upstream"]
        self.assert_rejected(snapshot, "retired rail 'vendor/upstream' has returned")

    def test_effective_main_rules_do_not_reflect_the_ruleset(self) -> None:
        # A ruleset body can look right while the rule is not actually in effect
        # on the branch, which is precisely what "configured but not enforcing"
        # looks like from the API.
        snapshot = load_fixture()
        snapshot["effective_main_rules"] = [{"type": "deletion"}]
        self.assert_rejected(snapshot, "effective rules", "non_fast_forward")


class WorkflowContractTests(ComparatorCase):
    def test_duplicate_context_definition(self) -> None:
        # Two jobs named "Nix Gate" in different workflows: branch protection
        # matches by name, so the wrong one could satisfy the requirement, and
        # integration_id cannot separate them because both are the same app.
        snapshot = load_fixture()
        snapshot["workflows"][".github/workflows/decoy.yml"] = (
            "name: Decoy\n"
            "on:\n"
            "  pull_request:\n"
            "    branches: [main]\n"
            "jobs:\n"
            "  decoy:\n"
            "    name: Nix Gate\n"
            "    runs-on: ubuntu-latest\n"
            "    steps:\n"
            "      - run: echo ok\n"
        )
        self.assert_rejected(snapshot, "defined by more than one job")

    def test_summary_dependency_removed(self) -> None:
        snapshot = load_fixture()
        snapshot["workflows"][".github/workflows/fork-ci.yml"] = snapshot["workflows"][
            ".github/workflows/fork-ci.yml"
        ].replace(
            "needs: [changes, governance-contract, quality, macos, linux-tests]",
            "needs: [changes, governance-contract, quality, macos]",
        )
        self.assert_rejected(snapshot, "summary dependencies")

    def test_summary_dependency_added(self) -> None:
        snapshot = load_fixture()
        workflow = snapshot["workflows"][".github/workflows/security.yml"]
        original = "needs: [detect-dependency-changes, advisory-policy, secret-scan, dependency-audit]"
        # A no-op replace would make this test vacuous, so pin the anchor.
        self.assertIn(original, workflow, "security-gate needs: line moved; update this fixture mutation")
        snapshot["workflows"][".github/workflows/security.yml"] = workflow.replace(
            original,
            original[:-1] + ", weekly-report]",
        )
        self.assert_rejected(snapshot, "summary dependencies")

    def test_declared_dependency_never_consulted(self) -> None:
        # The subtle version of dependency drift: `needs` is intact, so the job
        # waits for the dependency, but the gate script never reads its result,
        # making the gate green regardless of that job's conclusion.
        snapshot = load_fixture()
        snapshot["workflows"][".github/workflows/nix.yml"] = snapshot["workflows"][
            ".github/workflows/nix.yml"
        ].replace("${{ needs.build.result }}", "success")
        self.assert_rejected(snapshot, "never reads", "needs.build.result")

    def test_routing_drift_on_a_conditional_job(self) -> None:
        snapshot = load_fixture()
        snapshot["workflows"][".github/workflows/fork-ci.yml"] = snapshot["workflows"][
            ".github/workflows/fork-ci.yml"
        ].replace(
            "if: needs.changes.outputs.rust == 'true' || needs.changes.outputs.scripts == 'true' || github.event_name != 'pull_request'",
            "if: needs.changes.outputs.rust == 'true' || github.event_name != 'pull_request'",
        )
        self.assert_rejected(snapshot, "routed job 'quality'")

    def test_workflow_level_pull_request_paths_filter(self) -> None:
        # The lockout case: a required context whose workflow is path-filtered
        # never runs on an unrelated PR, so that PR can never satisfy the
        # requirement and the branch becomes permanently unmergeable.
        snapshot = load_fixture()
        snapshot["workflows"][".github/workflows/nix.yml"] = snapshot["workflows"][
            ".github/workflows/nix.yml"
        ].replace(
            "  pull_request:\n    branches: [main]\n",
            '  pull_request:\n    branches: [main]\n    paths:\n      - "flake.nix"\n',
            1,
        )
        self.assert_rejected(snapshot, "paths", "unmergeable")

    def test_pull_request_trigger_removed(self) -> None:
        snapshot = load_fixture()
        snapshot["workflows"][".github/workflows/governance-root.yml"] = snapshot["workflows"][
            ".github/workflows/governance-root.yml"
        ].replace("  pull_request:\n    branches: [main]\n", "  workflow_dispatch:\n")
        self.assert_rejected(snapshot, "no pull_request trigger")

    def test_required_context_job_renamed(self) -> None:
        snapshot = load_fixture()
        snapshot["workflows"][".github/workflows/security.yml"] = snapshot["workflows"][
            ".github/workflows/security.yml"
        ].replace("    name: Security Gate", "    name: Security Summary")
        self.assert_rejected(snapshot, "'Security Gate' has no job definition")

    def test_always_if_weakened(self) -> None:
        # `if: always()` is what makes the summary run when a dependency was
        # skipped. Without it the summary is skipped too, and a skipped required
        # context blocks forever rather than failing informatively.
        snapshot = load_fixture()
        snapshot["workflows"][".github/workflows/fork-ci.yml"] = snapshot["workflows"][
            ".github/workflows/fork-ci.yml"
        ].replace(
            "    if: always() && github.event_name == 'pull_request'\n    runs-on: ubuntu-latest\n    timeout-minutes: 5\n    env:\n      CHANGES_RESULT:",
            "    if: github.event_name == 'pull_request'\n    runs-on: ubuntu-latest\n    timeout-minutes: 5\n    env:\n      CHANGES_RESULT:",
        )
        self.assert_rejected(snapshot, "`if:` is")

    def test_governance_root_stops_naming_a_protected_path(self) -> None:
        # A vacuous audit gate is worse than none: it reports green on exactly
        # the change it exists to flag. design.md section 13 makes this a stop.
        snapshot = load_fixture()
        snapshot["workflows"][".github/workflows/governance-root.yml"] = snapshot["workflows"][
            ".github/workflows/governance-root.yml"
        ].replace("            scripts/fork-health.sh\n", "")
        self.assert_rejected(snapshot, "does not name protected path", "scripts/fork-health.sh")

    def test_unparseable_workflow_is_exit_two_not_a_pass(self) -> None:
        snapshot = load_fixture()
        snapshot["workflows"][".github/workflows/nix.yml"] = (
            "name: Nix\n"
            "on:\n"
            "  pull_request:\n"
            "    branches: [main]\n"
            "jobs:\n"
            "  base: &anchor\n"
            "    runs-on: ubuntu-latest\n"
        )
        self.assert_schema_failure(snapshot, "anchor")

    def test_tab_indentation_is_exit_two(self) -> None:
        snapshot = load_fixture()
        snapshot["workflows"][".github/workflows/nix.yml"] = "name: Nix\non:\n\tpull_request:\n"
        self.assert_schema_failure(snapshot, "tab indentation")


class SchemaFailureTests(ComparatorCase):
    def test_missing_bypass_actors_is_authorization_failure(self) -> None:
        # This is the single most important negative: a credential without
        # ruleset write access gets a body with no bypass_actors at all. Reading
        # that as "no bypass actors" would convert an unauthorized read into a
        # green governance result.
        snapshot = load_fixture()
        del ruleset(snapshot, "protect-fork-rails")["bypass_actors"]
        output = self.assert_schema_failure(snapshot, "bypass_actors")
        self.assertIn("unauthorized", output)

    def test_missing_rules_key(self) -> None:
        snapshot = load_fixture()
        del ruleset(snapshot, "protect-fork-rails")["rules"]
        self.assert_schema_failure(snapshot, "no rules key")

    def test_missing_conditions_key(self) -> None:
        snapshot = load_fixture()
        del ruleset(snapshot, "no-stray-branches")["conditions"]
        self.assert_schema_failure(snapshot, "no conditions key")

    def test_missing_snapshot_section(self) -> None:
        for key in ("repository", "rulesets", "effective_main_rules", "branches"):
            with self.subTest(key=key):
                snapshot = load_fixture()
                del snapshot[key]
                self.assert_schema_failure(snapshot, f"missing required key {key!r}")

    def test_missing_repository_merge_setting(self) -> None:
        snapshot = load_fixture()
        del snapshot["repository"]["allow_squash_merge"]
        self.assert_schema_failure(snapshot, "allow_squash_merge")

    def test_malformed_required_status_check_entry(self) -> None:
        snapshot = load_fixture()
        rule(snapshot, "protect-fork-rails", "required_status_checks")["parameters"][
            "required_status_checks"
        ] = ["Nix Gate"]
        self.assert_schema_failure(snapshot, "malformed required status check")

    def test_unsupported_manifest_schema_version(self) -> None:
        manifest = load_manifest()
        manifest["schema_version"] = 99
        result = self.run_snapshot(load_fixture(), manifest=manifest)
        self.assertEqual(result.returncode, EXIT_ACQUISITION)
        self.assertIn("schema_version", result.stderr)


class SanitizationTests(ComparatorCase):
    def test_server_generated_keys_do_not_affect_the_result(self) -> None:
        # A live response carries ids, timestamps, and links that change on
        # every read. If those reached the comparison, live mode would be
        # permanently red and nobody would look at it.
        snapshot = load_fixture()
        body = ruleset(snapshot, "protect-fork-rails")
        body.update(
            {
                "id": 18509013,
                "node_id": "RRS_lACqUmVwb3NpdG9yec5J06N6zgEabNU",
                "source": "jerudnik/jcode",
                "source_type": "Repository",
                "created_at": "2026-07-04T10:22:12.044-04:00",
                "updated_at": "2026-07-27T00:40:10.812-04:00",
                "current_user_can_bypass": "never",
                "_links": {"self": {"href": "https://api.github.com/x"}},
            }
        )
        result = self.run_snapshot(snapshot)
        self.assertEqual(
            result.returncode, EXIT_OK, f"sanitization failed:\n{result.stdout}{result.stderr}"
        )


class CrossArtifactCoherenceTests(unittest.TestCase):
    """The enforced protected-path set must be identical in every artifact.

    The R07 integration gate caught the apply document lagging the manifest and
    workflow after adjudication; this test makes a future one-sided edit fail
    loudly instead of silently weakening the bootstrap-to-apply assertion.
    """

    APPLY_DOC = (
        REPO_ROOT
        / "docs"
        / "fork"
        / "ideal-base"
        / "evidence"
        / "R07"
        / "github-governance.proposed.json"
    )
    RATCHET_BASELINES = {
        "scripts/code_size_budget.json",
        "scripts/panic_budget.json",
        "scripts/swallowed_error_budget.json",
        "scripts/test_size_budget.json",
        "scripts/warning_budget.txt",
    }

    @staticmethod
    def _norm(paths: list[str]) -> set[str]:
        return {p.rstrip("/") for p in paths}

    @staticmethod
    def _protected_array(text: str) -> set[str]:
        """Parse the audit gate's inline `protected=( ... )` array.

        Deliberately re-derived here rather than imported: this test's job is
        to be an independent reader of the artifacts, so it must not inherit
        the comparator's parse. A zero-pattern parse is an artifact, never an
        answer, so an empty result fails loudly instead of comparing equal to
        an empty manifest.
        """
        matches = re.findall(r"protected=\(\s*(.*?)\s*\)", text, re.DOTALL)
        if len(matches) != 1:
            raise AssertionError(
                f"expected exactly one `protected=( ... )` array, found {len(matches)}"
            )
        paths = {token for token in matches[0].split() if token}
        if not paths:
            raise AssertionError("`protected=( ... )` array parsed empty")
        return paths

    def test_protected_set_is_coherent_across_artifacts(self) -> None:
        manifest = load_manifest()
        required = self._norm(manifest["protected_paths"]["required"])
        self.assertEqual(
            manifest["protected_paths"]["proposed_additions"],
            [],
            "unadjudicated proposed additions must not linger post-integration",
        )
        self.assertTrue(manifest["protected_paths"]["additions_adjudicated"])

        # Compare only the artifacts this change actually owns. The workflow
        # text and the checked-in fixture must both name exactly the same
        # long-lived governance paths as the manifest.
        workflow_text = load_fixture()["workflows"][
            ".github/workflows/governance-root.yml"
        ]
        fixture_paths = self._norm(
            sorted(self._protected_array(workflow_text))
        )
        self.assertEqual(
            required,
            fixture_paths,
            f"manifest/governance-root.yml fixture mismatch: "
            f"{sorted(required ^ fixture_paths)}",
        )

        # The fixture is a copy. The set the gate actually runs is the one in
        # the live workflow on disk, so hold that to the same equality: a
        # stale fixture must not be able to certify a drifted gate.
        live_workflow = (
            REPO_ROOT / ".github" / "workflows" / "governance-root.yml"
        ).read_text(encoding="utf-8")
        live_paths = self._norm(
            sorted(self._protected_array(live_workflow))
        )
        self.assertEqual(
            required,
            live_paths,
            f"manifest/live governance-root.yml mismatch: "
            f"{sorted(required ^ live_paths)}",
        )

        # The ratchet baselines are deliberately unprotected everywhere; if a
        # future edit adds one back it must happen in all artifacts at once,
        # which this test forces by pinning their absence.
        for baseline in sorted(self.RATCHET_BASELINES):
            self.assertNotIn(baseline, required)
            self.assertNotIn(baseline, workflow_text)


class ProtectedPathAdjudicationTests(ComparatorCase):
    def test_pending_additions_are_reported_but_not_enforced(self) -> None:
        # The repo manifest has no pending additions (the R07 integration
        # adjudication resolved them), so construct the pending state
        # explicitly: a real path the fixture workflow does not name, behind
        # additions_adjudicated: false, must report but stay green.
        manifest = load_manifest()
        manifest["protected_paths"]["proposed_additions"] = [
            "scripts/governance_compare.py"
        ]
        manifest["protected_paths"]["additions_adjudicated"] = False
        result = self.run_snapshot(load_fixture(), manifest=manifest)
        self.assertEqual(result.returncode, EXIT_OK, result.stdout + result.stderr)
        self.assertIn("pending adjudication", result.stdout)

    def test_adjudicated_additions_are_enforced(self) -> None:
        # Flipping the flag on a pending addition must turn the same fixture
        # red, which proves the flag is load-bearing rather than decorative.
        # Use a deliberately unprotected ratchet baseline: it exists in the
        # tree (so the schema check passes) but the fixture workflow does not
        # name it, so enforcement must fail with a mismatch.
        manifest = load_manifest()
        manifest["protected_paths"]["proposed_additions"] = [
            "scripts/panic_budget.json"
        ]
        manifest["protected_paths"]["additions_adjudicated"] = True
        snapshot = load_fixture()
        result = self.run_snapshot(snapshot, manifest=manifest)
        output = result.stdout + result.stderr
        self.assertEqual(result.returncode, EXIT_MISMATCH, output)
        self.assertIn("does not name protected path", output)
        self.assertIn("scripts/panic_budget.json", output)

    def test_protected_path_that_does_not_exist_is_a_schema_error(self) -> None:
        # A protected path with a typo protects nothing while reading as
        # coverage, so it must fail loudly. Exit 2, not 1: the manifest is
        # wrong, which is unclassifiable, rather than the remote having drifted.
        manifest = load_manifest()
        manifest["protected_paths"]["required"].append("scripts/check_not_a_real_gate.py")
        result = self.run_snapshot(load_fixture(), manifest=manifest)
        output = result.stdout + result.stderr
        self.assertEqual(result.returncode, EXIT_SCHEMA, output)
        self.assertIn("do not exist in the working tree", output)
        self.assertIn("scripts/check_not_a_real_gate.py", output)

    def test_every_declared_protected_path_exists(self) -> None:
        manifest = load_manifest()
        protected = manifest["protected_paths"]
        for path in protected["required"] + protected["proposed_additions"]:
            self.assertTrue(
                (REPO_ROOT / path).exists(), f"protected path does not exist: {path}"
            )


GH_SHIM = '''#!/usr/bin/env python3
import json, os, sys

table = json.loads(open(os.environ["GH_SHIM_TABLE"]).read())
argv = sys.argv[1:]
if argv[:1] == ["auth"]:
    if table.get("__auth_fails__"):
        sys.stderr.write("not logged in\\n")
        sys.exit(1)
    sys.exit(0)
if argv[:1] != ["api"] or len(argv) < 2:
    sys.stderr.write("shim: unsupported invocation %r\\n" % (argv,))
    sys.exit(1)
path = argv[1]
if path in table.get("__fail__", {}):
    sys.stderr.write(table["__fail__"][path] + "\\n")
    sys.exit(1)
if path not in table:
    sys.stderr.write("shim: HTTP 404: no such path %s\\n" % path)
    sys.exit(1)
sys.stdout.write(json.dumps(table[path]))
'''


class LiveModeTests(unittest.TestCase):
    """Live mode through a `gh` shim: the only way to observe it red offline."""

    maxDiff = None

    def build_table(self, snapshot: dict | None = None) -> dict:
        snapshot = snapshot or load_fixture()
        repo = load_manifest()["repository"]
        table = {
            f"repos/{repo}": snapshot["repository"],
            f"repos/{repo}/rulesets": [
                {"id": 1000 + i, "name": b["name"]} for i, b in enumerate(snapshot["rulesets"])
            ],
            f"repos/{repo}/rules/branches/main": snapshot["effective_main_rules"],
            f"repos/{repo}/branches?per_page=100": [
                {"name": name} for name in snapshot["branches"]
            ],
        }
        for i, body in enumerate(snapshot["rulesets"]):
            table[f"repos/{repo}/rulesets/{1000 + i}"] = body
        classic = snapshot["classic_branch_protection"]
        if classic is not None:
            table[f"repos/{repo}/branches/main/protection"] = classic
        return table

    def run_live(
        self,
        table: dict,
        *,
        on_path: bool = True,
        workflows: dict[str, str] | None = None,
    ) -> subprocess.CompletedProcess:
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            table_path = tmp_path / "table.json"
            table_path.write_text(json.dumps(table), encoding="utf-8")
            shim = tmp_path / "gh"
            shim.write_text(GH_SHIM, encoding="utf-8")
            shim.chmod(shim.stat().st_mode | stat.S_IEXEC | stat.S_IXGRP | stat.S_IXOTH)

        # Live mode reads workflow text from the working tree. Tests that
        # need the three required-context jobs to exist must supply them,
        # because workflow-contexts.proposed.patch is coordinator-owned and
        # has not been applied to this repository's own .github/workflows.
            if workflows is None:
                workflows_dir = REPO_ROOT / ".github" / "workflows"
            else:
                workflows_dir = tmp_path / "workflows"
                workflows_dir.mkdir()
                for path, text in workflows.items():
                    (workflows_dir / Path(path).name).write_text(text, encoding="utf-8")

            env = dict(os.environ)
            env["GH_SHIM_TABLE"] = str(table_path)
            env["FORK_HEALTH_GH"] = str(shim) if on_path else "definitely-not-gh"
            return subprocess.run(
                [
                    sys.executable,
                    str(COMPARATOR),
                    "--manifest",
                    str(MANIFEST),
                    "--live",
                    "--workflows-dir",
                    str(workflows_dir),
                ],
                capture_output=True,
                text=True,
                check=False,
                env=env,
            )

    def test_valid_live_surface_passes(self) -> None:
        snapshot = load_fixture()
        result = self.run_live(self.build_table(snapshot), workflows=snapshot["workflows"])
        output = result.stdout + result.stderr
        self.assertEqual(result.returncode, EXIT_OK, f"valid live surface was rejected:\n{output}")
        self.assertIn("matches the manifest", result.stdout)

    def test_live_mode_accepts_the_repositorys_patched_workflows(self) -> None:
        # Post-bootstrap counterpart of the pre-bootstrap sanity check (which
        # asserted the unpatched workflows went red): now that the authorized
        # workflow diff is applied, this repository's actual workflow
        # directory carries the three required-context jobs and live mode must
        # go green. If a future edit removes a required-context job, this goes
        # red again, so the workflow contract check stays load-bearing.
        result = self.run_live(self.build_table())
        output = result.stdout + result.stderr
        self.assertEqual(result.returncode, EXIT_OK, output)
        self.assertIn("matches the manifest", result.stdout)

    def test_missing_gh_is_exit_two(self) -> None:
        result = self.run_live(self.build_table(), on_path=False)
        self.assertEqual(result.returncode, EXIT_ACQUISITION, result.stdout + result.stderr)
        self.assertIn("not on PATH", result.stderr)

    def test_unauthenticated_gh_is_exit_two(self) -> None:
        table = self.build_table()
        table["__auth_fails__"] = True
        result = self.run_live(table)
        self.assertEqual(result.returncode, EXIT_ACQUISITION, result.stdout + result.stderr)
        self.assertIn("gh auth status", result.stderr)

    def test_each_endpoint_failure_is_exit_two_and_names_the_endpoint(self) -> None:
        repo = load_manifest()["repository"]
        endpoints = [
            f"repos/{repo}",
            f"repos/{repo}/rulesets",
            f"repos/{repo}/rulesets/1000",
            f"repos/{repo}/rules/branches/main",
            f"repos/{repo}/branches?per_page=100",
        ]
        for endpoint in endpoints:
            with self.subTest(endpoint=endpoint):
                table = self.build_table()
                table["__fail__"] = {endpoint: "HTTP 503: upstream unavailable"}
                result = self.run_live(table)
                self.assertEqual(
                    result.returncode,
                    EXIT_ACQUISITION,
                    result.stdout + result.stderr,
                )
                self.assertIn(endpoint, result.stderr)

    def test_insufficient_authorization_hides_bypass_actors(self) -> None:
        snapshot = load_fixture()
        del ruleset(snapshot, "protect-fork-rails")["bypass_actors"]
        result = self.run_live(self.build_table(snapshot))
        self.assertEqual(result.returncode, EXIT_ACQUISITION, result.stdout + result.stderr)
        self.assertIn("unauthorized", result.stderr)

    def test_mutated_live_surface_is_observed_red(self) -> None:
        # D029: the drift detector must be seen failing against a deliberately
        # wrong live surface, not only against a fixture.
        snapshot = load_fixture()
        ruleset(snapshot, "protect-fork-rails")["bypass_actors"] = [
            {"actor_id": 5, "actor_type": "RepositoryRole", "bypass_mode": "always"}
        ]
        result = self.run_live(self.build_table(snapshot))
        self.assertEqual(result.returncode, EXIT_MISMATCH, result.stdout + result.stderr)
        self.assertIn("bypass_actors", result.stderr)

    def test_absent_classic_protection_is_a_404_not_a_failure(self) -> None:
        table = self.build_table()
        repo = load_manifest()["repository"]
        self.assertNotIn(f"repos/{repo}/branches/main/protection", table)
        result = self.run_live(table)
        self.assertNotEqual(result.returncode, EXIT_ACQUISITION, result.stdout + result.stderr)
        self.assertNotIn("classic branch protection still exists", result.stdout + result.stderr)


class ForkHealthModeTests(unittest.TestCase):
    """Mode selection is a contract: design.md section 6 forbids warn-and-skip."""

    @classmethod
    def fork_remote(cls) -> str:
        """The canonical fork remote available in this checkout.

        CI (actions/checkout) names it `origin`; the canonical local clone names
        it `github`. Discovering it instead of hardcoding `origin` keeps these
        tests runnable on any checkout, matching the same fix made to the
        railway validator and its tests.
        """
        configured = subprocess.run(
            ["git", "remote"],
            cwd=REPO_ROOT,
            capture_output=True,
            text=True,
            check=False,
        ).stdout.split()
        for name in ("origin", "github"):
            if name in configured:
                return name
        return configured[0] if configured else "origin"

    def run_fork_health(self, *args: str) -> subprocess.CompletedProcess:
        return subprocess.run(
            [str(FORK_HEALTH), *args],
            capture_output=True,
            text=True,
            check=False,
            cwd=REPO_ROOT,
        )

    def test_no_source_is_usage_error(self) -> None:
        result = self.run_fork_health("--fork-remote", self.fork_remote())
        self.assertEqual(result.returncode, EXIT_ACQUISITION, result.stdout + result.stderr)
        self.assertIn("--fixture", result.stderr)

    def test_both_sources_is_usage_error(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "fixture.json"
            path.write_text(json.dumps(load_fixture()), encoding="utf-8")
            result = self.run_fork_health("--fixture", str(path), "--live")
        self.assertEqual(result.returncode, EXIT_ACQUISITION, result.stdout + result.stderr)
        self.assertIn("mutually exclusive", result.stderr)

    def test_unknown_option_is_usage_error(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "fixture.json"
            path.write_text(json.dumps(load_fixture()), encoding="utf-8")
            result = self.run_fork_health("--fixture", str(path), "--nope")
        self.assertEqual(result.returncode, EXIT_ACQUISITION)
        self.assertIn("unknown option", result.stderr)

    def test_missing_fixture_is_usage_error(self) -> None:
        result = self.run_fork_health("--fixture", "/nonexistent/fixture.json", "--fork-remote", self.fork_remote())
        self.assertEqual(result.returncode, EXIT_ACQUISITION)
        self.assertIn("fixture not found", result.stderr)

    def test_repo_disagreeing_with_the_manifest_is_usage_error(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "fixture.json"
            path.write_text(json.dumps(load_fixture()), encoding="utf-8")
            result = self.run_fork_health(
                "--fixture", str(path), "--repo", "someone/else", "--fork-remote", self.fork_remote()
            )
        self.assertEqual(result.returncode, EXIT_ACQUISITION)
        self.assertIn("disagrees with the manifest", result.stderr)

    def test_valid_fixture_run_is_green_end_to_end(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "fixture.json"
            path.write_text(json.dumps(load_fixture()), encoding="utf-8")
            result = self.run_fork_health("--fixture", str(path), "--fork-remote", self.fork_remote())
        self.assertEqual(result.returncode, EXIT_OK, result.stdout + result.stderr)
        self.assertIn("all invariants hold", result.stdout)

    def test_a_mismatched_fixture_makes_the_whole_script_red(self) -> None:
        # The comparator's exit 1 must propagate. A script that printed the
        # mismatch and still exited 0 would be the warn-and-skip regression in a
        # different costume.
        snapshot = load_fixture()
        ruleset(snapshot, "protect-fork-rails")["enforcement"] = "disabled"
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "mutated.json"
            path.write_text(json.dumps(snapshot), encoding="utf-8")
            result = self.run_fork_health("--fixture", str(path), "--fork-remote", self.fork_remote())
        self.assertEqual(result.returncode, EXIT_MISMATCH, result.stdout + result.stderr)
        self.assertIn("invariant violation", result.stderr)
        self.assertNotIn("all invariants hold", result.stdout)


if __name__ == "__main__":
    unittest.main()
