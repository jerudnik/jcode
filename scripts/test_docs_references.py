#!/usr/bin/env python3
"""Tests for check_docs_references.py.

Every rule gets a planted failure. A checker that cannot fail is worse than no
checker, because it reports OK and nobody looks again. Two of these tests exist
specifically because the rule they cover is easy to get backwards:

  test_prohibition_is_not_a_finding   the retired-rail rule must not fire on
                                      the policy sentence that retires the
                                      rail, or it would flag its own contract
  test_exclusions_do_not_hide_live_docs
                                      the path exclusions are load-bearing (25
                                      findings instead of 143), so they must be
                                      proven to exclude only frozen material
"""

from __future__ import annotations

import sys
import subprocess
import tempfile
import unittest
from pathlib import Path

SCRIPTS_DIR = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPTS_DIR))

import check_docs_references as mod  # noqa: E402


class DocsReferencesTest(unittest.TestCase):
    def run_on(self, files: dict[str, str]) -> list[mod.Finding]:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            for rel, text in files.items():
                path = root / rel
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(text, encoding="utf-8")
            return mod.run(root)

    def rules(self, findings: list[mod.Finding]) -> set[str]:
        return {f.rule for f in findings}

    # --- broken-link -----------------------------------------------------

    def test_broken_link_is_flagged(self):
        findings = self.run_on({"docs/a.md": "see [thing](./gone.md)\n"})
        self.assertIn("broken-link", self.rules(findings))

    def run_on_git_tree(self, files: dict[str, str], untracked: dict[str, str]):
        """Like run_on, but the tree is a real git repo and `untracked` is not added.

        The untracked-target rule can only be exercised against real git state,
        which is exactly why the bug it now catches survived every other test
        here: a tempdir with no repo makes the checker fall back to filesystem
        truth, so a generated-but-ignored file looks fine.
        """
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            for rel, text in {**files, **untracked}.items():
                path = root / rel
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(text, encoding="utf-8")
            subprocess.run(["git", "init", "-q"], cwd=root, check=True)
            for rel in files:
                subprocess.run(["git", "add", "--", rel], cwd=root, check=True)
            mod._tracked_files.cache_clear()
            try:
                return mod.run(root)
            finally:
                mod._tracked_files.cache_clear()

    def test_link_to_generated_untracked_file_is_flagged(self):
        """CI caught this on a clean checkout when no local test could.

        docs/README.md linked to docs/AGENTS.md, which `apm compile` generates
        and .gitignore excludes. It resolved on the author's machine and 404s
        for every reader, so `exists()` alone was the wrong question.
        """
        findings = self.run_on_git_tree(
            {"docs/a.md": "see [contract](./AGENTS.md)\n"},
            {"docs/AGENTS.md": "generated\n"},
        )
        self.assertIn("broken-link", self.rules(findings))
        self.assertIn("not committed", " ".join(f.detail for f in findings))

    def test_link_to_tracked_file_is_not_flagged_in_a_git_tree(self):
        """The control: the rule must not fire on ordinary committed files."""
        findings = self.run_on_git_tree(
            {"docs/a.md": "see [thing](./b.md)\n", "docs/b.md": "hi\n"}, {}
        )
        self.assertEqual(self.rules(findings), set())

    # --- document discovery scope -----------------------------------------

    def test_untracked_document_is_not_scanned(self):
        """Local reported 146 documents, CI reported 137, same tree. The 9 were
        apm-generated AGENTS.md/CLAUDE.md/GEMINI.md, gitignored and absent from
        a clean clone. A gate that scans the checkout rather than the commit
        gives different verdicts to different people."""
        # Needs one tracked file: an empty `git ls-files` is indistinguishable
        # from "no git here" and correctly falls back to a filesystem scan.
        findings = self.run_on_git_tree(
            {"docs/real.md": "fine\n"}, {"docs/generated.md": "[x](./gone.md)\n"}
        )
        self.assertEqual(self.rules(findings), set())

    def test_tracked_document_is_still_scanned(self):
        """The control: narrowing scope to git must not narrow it to nothing."""
        findings = self.run_on_git_tree({"docs/real.md": "[x](./gone.md)\n"}, {})
        self.assertIn("broken-link", self.rules(findings))

    # --- stale-code-path (D01-F12) ---------------------------------------

    def test_citation_of_a_missing_source_file_is_flagged(self):
        """The F12 class: modularization moved files and the docs still cite
        the old path, so the reader is sent somewhere that does not exist."""
        findings = self.run_on_git_tree(
            {"docs/a.md": "see `src/platform.rs` for detail\n"}, {}
        )
        self.assertIn("stale-code-path", self.rules(findings))

    def test_citation_of_a_tracked_source_file_is_not_flagged(self):
        """The control. Without this the rule could fire on everything."""
        findings = self.run_on_git_tree(
            {"docs/a.md": "see `src/real.rs`\n", "src/real.rs": "fn main() {}\n"}, {}
        )
        self.assertEqual(self.rules(findings), set())

    def test_dated_audit_snapshots_are_exempt(self):
        """A document titled with its date is a record of a tree that has since
        moved. Its stale paths are accurate history, and flagging them would
        mean editing evidence to make a counter fall."""
        findings = self.run_on_git_tree(
            {"docs/CODE_QUALITY_AUDIT_2026-04-18.md": "see `src/gone.rs`\n"}, {}
        )
        self.assertEqual(self.rules(findings), set())

    def test_prose_mentioning_a_path_without_backticks_is_not_flagged(self):
        """The rule keys on a backticked citation. Bare prose is too noisy to
        gate on, and widening it was measured at 847 and 3451 hits."""
        findings = self.run_on_git_tree({"docs/a.md": "the old src/gone.rs file\n"}, {})
        self.assertEqual(self.rules(findings), set())

    def test_existing_link_is_not_flagged(self):
        findings = self.run_on({"docs/a.md": "see [thing](./b.md)\n", "docs/b.md": "hi\n"})
        self.assertEqual(self.rules(findings), set())

    def test_external_and_anchor_links_are_ignored(self):
        findings = self.run_on(
            {"docs/a.md": "[x](https://example.com) [y](#section) [z](mailto:a@b.c)\n"}
        )
        self.assertEqual(self.rules(findings), set())

    def test_link_with_anchor_resolves_to_the_file(self):
        findings = self.run_on({"docs/a.md": "[x](./b.md#part)\n", "docs/b.md": "hi\n"})
        self.assertEqual(self.rules(findings), set())

    def test_image_links_are_not_treated_as_links(self):
        # ![...](...) is an embed, and a missing image is a different defect
        # class than a broken prose reference.
        findings = self.run_on({"docs/a.md": "![alt](./missing.png)\n"})
        self.assertEqual(self.rules(findings), set())

    # --- machine-local ---------------------------------------------------

    def test_machine_local_link_is_flagged(self):
        findings = self.run_on({"docs/a.md": "[plan](~/notes/projects/p.md)\n"})
        self.assertIn("machine-local", self.rules(findings))

    def test_machine_local_prose_is_flagged(self):
        # This is the D01-F08 case that a link-only rule misses. Counting only
        # links gave 10; counting any mention gives 14, and the extra 4 are
        # prose or backticked references that are equally unreachable.
        findings = self.run_on({"docs/a.md": "See ~/notes/projects/jcode/plan.md for detail.\n"})
        self.assertIn("machine-local", self.rules(findings))

    def test_machine_local_absolute_home_is_flagged(self):
        findings = self.run_on({"docs/a.md": "run it in /Users/someone/labs/jcode\n"})
        self.assertIn("machine-local", self.rules(findings))

    def test_machine_local_counts_every_occurrence(self):
        findings = self.run_on(
            {"docs/a.md": "~/notes/one.md\n~/notes/two.md\nunrelated\n~/notes/three.md\n"}
        )
        self.assertEqual(len([f for f in findings if f.rule == "machine-local"]), 3)

    # --- retired-rail ----------------------------------------------------

    def test_retired_rail_instruction_is_flagged(self):
        for line in (
            "Install with `brew install jcode`.",
            "Run `yay -S jcode` on Arch.",
            "Bootstrap: curl -fsSL https://example.com/i.sh | sh",
            "You can cargo install jcode from crates.io.",
            "Download the build via TestFlight to try it.",
        ):
            with self.subTest(line=line):
                findings = self.run_on({"docs/a.md": line + "\n"})
                self.assertIn("retired-rail", self.rules(findings), line)

    def test_prohibition_is_not_a_finding(self):
        # The exact sentences shipped in README.md and RELEASING.md. A rule
        # that flags these would fire on the contract it exists to enforce.
        for line in (
            "Homebrew, AUR, GitHub executable assets, checksum manifests for those assets,",
            "curl installers, release archives, Homebrew, AUR, Cargo registry packages, or a",
            "The native iOS application and App Store/TestFlight delivery are retired.",
            "Do not add brew install instructions.",
            "We never ship via cargo install jcode.",
        ):
            with self.subTest(line=line):
                findings = self.run_on({"docs/a.md": line + "\n"})
                self.assertEqual(self.rules(findings), set(), line)

    # --- scope -----------------------------------------------------------

    def test_archives_are_excluded(self):
        findings = self.run_on({"docs/archive/old.md": "[gone](./nope.md)\n~/notes/x.md\n"})
        self.assertEqual(self.rules(findings), set())

    def test_frozen_fork_records_are_excluded(self):
        findings = self.run_on(
            {
                "docs/fork/recovery/r.md": "/Users/someone/labs/jcode\n",
                "docs/fork/normalization/n.md": "/Users/someone/labs/jcode\n",
            }
        )
        self.assertEqual(self.rules(findings), set())

    def test_exclusions_do_not_hide_live_docs(self):
        # The load-bearing counter-check: a path that merely *resembles* an
        # excluded one must still be checked, so the exclusion cannot be
        # widened by accident into "skip everything under docs/fork".
        findings = self.run_on({"docs/fork/ideal-base/STATE_NOTES.md": "[x](./gone.md)\n"})
        self.assertIn("broken-link", self.rules(findings))

    def test_audit_register_may_name_machine_local_paths(self):
        findings = self.run_on(
            {"docs/fork/ideal-base/D01_DOCUMENTATION_AUDIT.md": "counts ~/notes/x.md references\n"}
        )
        self.assertEqual(self.rules(findings), set())

    def test_audit_register_is_still_link_checked(self):
        # Exempt from machine-local, NOT exempt from broken links.
        findings = self.run_on(
            {"docs/fork/ideal-base/D01_DOCUMENTATION_AUDIT.md": "[x](./gone.md)\n"}
        )
        self.assertIn("broken-link", self.rules(findings))

    # --- exit behavior ---------------------------------------------------

    def test_clean_tree_reports_no_findings(self):
        findings = self.run_on({"docs/a.md": "clean [link](./b.md)\n", "docs/b.md": "hi\n"})
        self.assertEqual(findings, [])


class RatchetTest(unittest.TestCase):
    """The machine-local rule is budgeted, because D01-F08 is still open.

    A ratchet is only honest if it can go down and cannot go up. These tests
    pin both directions; without the second, the baseline would be a way to
    launder new debt into the tree.
    """

    def test_new_reference_exceeds_baseline(self):
        problems = mod.ratchet_violations({"docs/a.md": 3}, {"docs/a.md": 2})
        self.assertEqual(len(problems), 1)
        self.assertIn("baseline allows 2", problems[0])

    def test_at_baseline_is_clean(self):
        self.assertEqual(mod.ratchet_violations({"docs/a.md": 2}, {"docs/a.md": 2}), [])

    def test_below_baseline_is_clean(self):
        self.assertEqual(mod.ratchet_violations({"docs/a.md": 1}, {"docs/a.md": 2}), [])

    def test_file_absent_from_baseline_is_a_violation(self):
        # A brand new file carrying a machine-local path must not slip through
        # just because it has no baseline entry.
        problems = mod.ratchet_violations({"docs/new.md": 1}, {})
        self.assertEqual(len(problems), 1)
        self.assertIn("baseline allows 0", problems[0])

    def test_update_refuses_to_raise_an_existing_baseline(self):
        # write_baseline targets the real baseline path, so redirect it. Without
        # this the test only avoids clobbering the repository because the guard
        # under test happens to fire first, which means a mutation that removes
        # the guard silently rewrites the shipped baseline.
        with tempfile.TemporaryDirectory() as tmp:
            target = Path(tmp) / "budget.json"
            original = mod.BASELINE_FILE
            mod.BASELINE_FILE = target
            try:
                with self.assertRaises(SystemExit) as caught:
                    mod.write_baseline({"docs/a.md": 3}, {"docs/a.md": 2})
                self.assertIn("refuses to raise", str(caught.exception))
                self.assertFalse(target.exists(), "refusal must not write a baseline")
            finally:
                mod.BASELINE_FILE = original

    def test_shipped_baseline_matches_the_tree(self):
        # If this fails, someone changed the docs without running --update, or
        # the baseline was hand-edited. Either way the ratchet is no longer
        # describing reality.
        root = Path(__file__).resolve().parent.parent
        counts = mod.machine_local_counts(mod.run(root))
        self.assertEqual(counts, mod.load_baseline())

    def test_update_refuses_to_raise_a_rule_that_reached_zero(self):
        # Regression: a rule driven to zero has an EMPTY per-file dict, which the
        # "first measurement establishes the ceiling" branch read as "never
        # measured". Reaching zero therefore DISARMED the ratchet and the next
        # --update accepted any number of new stale citations. Finishing the
        # cleanup must not be what unlocks the regression.
        with tempfile.TemporaryDirectory() as tmp:
            target = Path(tmp) / "budget.json"
            original = mod.BASELINE_FILE
            mod.BASELINE_FILE = target
            try:
                # A rule that has been measured records a <key>_total, even at 0.
                mod.write_baselines(
                    {"machine-local": {}, "stale-code-path": {}},
                    {"machine-local": {}, "stale-code-path": {}},
                )
                self.assertIn("stale_code_paths_by_file_total", target.read_text())
                with self.assertRaises(SystemExit) as caught:
                    mod.write_baselines(
                        {"machine-local": {}, "stale-code-path": {"docs/a.md": 1}},
                        {"machine-local": {}, "stale-code-path": {}},
                    )
                self.assertIn("refuses to raise", str(caught.exception))
                self.assertIn("0 -> 1", str(caught.exception))
            finally:
                mod.BASELINE_FILE = original

    def test_first_ever_measurement_is_still_allowed(self):
        # Counter-check to the above: with no baseline file at all, a rule's
        # first measurement must still establish its ceiling rather than being
        # refused, otherwise a new rule could never be adopted.
        with tempfile.TemporaryDirectory() as tmp:
            target = Path(tmp) / "budget.json"
            original = mod.BASELINE_FILE
            mod.BASELINE_FILE = target
            try:
                mod.write_baselines(
                    {"machine-local": {}, "stale-code-path": {"docs/a.md": 4}},
                    {"machine-local": {}, "stale-code-path": {}},
                )
                self.assertTrue(target.exists())
            finally:
                mod.BASELINE_FILE = original


if __name__ == "__main__":
    unittest.main()
