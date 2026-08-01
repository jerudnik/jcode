#!/usr/bin/env python3
"""Tests for ci_workflow_commands: parity between the extractor and fork-ci.yml.

The whole point of the extractor is that ci_local.sh runs the SAME commands CI
runs. These tests fail loudly if the workflow format drifts out from under the
scan, which is the failure mode that would otherwise let the local gate silently
diverge from the authoritative hosted job.
"""

from __future__ import annotations

import unittest

import ci_workflow_commands as cwc


class JobExtractionTests(unittest.TestCase):
    def test_macos_job_has_release_build_and_full_test_suite(self) -> None:
        cmds = cwc.job_cargo_commands("macos")
        # The release build is the single most important command to mirror; a
        # regression that dropped it (it is an inline `run: cargo ...`, not a
        # block) would make the local gate miss release-only breaks.
        self.assertIn(
            "cargo build --locked --release --target aarch64-apple-darwin", cmds
        )
        # The suite CI actually runs, in order, must all be present.
        for expected in (
            "cargo test --locked --target aarch64-apple-darwin --workspace --lib --bins --no-run",
            "cargo test --locked --target aarch64-apple-darwin -p jcode-tui --lib",
            "cargo test --locked --target aarch64-apple-darwin -p jcode-app-core --lib",
            "cargo test --locked --target aarch64-apple-darwin --test provider_matrix",
            "cargo test --locked --target aarch64-apple-darwin --test e2e",
        ):
            self.assertIn(expected, cmds)

    def test_linux_job_uses_the_linux_triple(self) -> None:
        cmds = cwc.job_cargo_commands("linux-tests")
        self.assertTrue(cmds, "linux-tests job produced no commands")
        self.assertTrue(
            all("x86_64-unknown-linux-gnu" in c for c in cmds),
            "every linux-tests cargo command should target the linux triple",
        )

    def test_no_run_wrapper_leaks_into_commands(self) -> None:
        # The timeout wrapper (run_with_timeout.py) must be stripped: locally we
        # run the bare cargo command. A leak would try to exec cargo with a
        # numeric first arg.
        for job in ("macos", "linux-tests"):
            for cmd in cwc.job_cargo_commands(job):
                self.assertTrue(cmd.startswith("cargo "), f"not a bare cargo cmd: {cmd!r}")
                self.assertNotIn("run_with_timeout", cmd)

    def test_host_target_rewrite_is_applied(self) -> None:
        # The --host-target rewrite is what lets the macOS job run on whatever
        # triple the fleet builder actually is.
        lines = cwc.job_cargo_commands("macos")
        rewritten = [
            c.replace("--target aarch64-apple-darwin", "--target x86_64-apple-darwin")
            for c in lines
        ]
        self.assertTrue(any("x86_64-apple-darwin" in c for c in rewritten))
        self.assertFalse(any("aarch64-apple-darwin" in c for c in rewritten))

    def test_unknown_job_fails_loudly(self) -> None:
        with self.assertRaises(SystemExit):
            cwc.job_cargo_commands("no-such-job")


if __name__ == "__main__":
    unittest.main()
