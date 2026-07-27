#!/usr/bin/env python3
"""Executable guard for Jcode's Nix-only distribution contract."""

from __future__ import annotations

import pathlib
import tomllib
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[1]

RETIRED_PATHS = (
    "scripts/install.sh",
    "scripts/install.ps1",
    "scripts/uninstall.sh",
    "scripts/install_release.sh",
    "scripts/test_install_release.sh",
    "scripts/update_packages.sh",
    "scripts/quick-release.sh",
    "scripts/phone-server/testflight-setup.py",
    ".github/workflows/ios-testflight.yml",
)

ACTIVE_DISTRIBUTION_DOCS = (
    "README.md",
    "RELEASING.md",
    "docs/BRANCHING.md",
    "docs/IOS_APP.md",
    "docs/NIX.md",
    "docs/WINDOWS.md",
    "docs/WRAPPERS.md",
)

FORBIDDEN_ACTIVE_DOC_TEXT = (
    "scripts/install.sh",
    "scripts/install.ps1",
    "scripts/uninstall.sh",
    "scripts/update_packages.sh",
    "scripts/quick-release.sh",
    "brew install jcode",
    "brew tap 1jehuang/jcode",
    "jcode-bin",
    "jcode update installs",
    "--option eval-cores",
)


class NixOnlyDistributionPolicy(unittest.TestCase):
    def test_retired_distribution_entrypoints_are_absent(self) -> None:
        for relative in RETIRED_PATHS:
            with self.subTest(path=relative):
                self.assertFalse((ROOT / relative).exists(), f"retired distribution path returned: {relative}")

    def test_release_workflow_is_metadata_only(self) -> None:
        workflow = (ROOT / ".github/workflows/release.yml").read_text()
        self.assertIn("metadata-only GitHub release", workflow)
        self.assertIn("must not contain binary assets", workflow)
        self.assertIn("workflow_call", workflow)
        self.assertNotIn("tags:", workflow)
        for banned in (
            "actions/upload-artifact",
            "actions/download-artifact",
            "gh release upload",
            "SHA256SUMS",
            "Homebrew",
            "AUR",
            "cargo build",
            ".tar.gz",
            "TestFlight",
        ):
            with self.subTest(token=banned):
                self.assertNotIn(banned, workflow)

    def test_no_workflow_publishes_non_nix_binaries(self) -> None:
        banned_tokens = (
            "gh release upload",
            "xcodebuild archive",
            "-exportArchive",
            "APPSTORE_API_KEY",
            "notarytool submit",
            "cargo publish",
            "DeterminateSystems/nix-installer-action",
            "flakehub",
            "eval-cores",
        )
        workflow_dir = ROOT / ".github/workflows"
        workflows = list(workflow_dir.glob("*.yml")) + list(workflow_dir.glob("*.yaml"))
        for workflow in workflows:
            text = workflow.read_text()
            for banned in banned_tokens:
                with self.subTest(workflow=workflow.name, token=banned):
                    self.assertNotIn(banned, text)

    def test_nix_and_cachix_are_the_binary_authority(self) -> None:
        flake = (ROOT / "flake.nix").read_text()
        nix_workflow = (ROOT / ".github/workflows/nix.yml").read_text()
        self.assertIn("jerudnik-jcode.cachix.org", flake)
        self.assertIn("packages", flake)
        self.assertIn('tags: ["v*"]', nix_workflow)
        self.assertIn("cachix/cachix-action", nix_workflow)
        self.assertIn("cachix/install-nix-action@v31", nix_workflow)
        self.assertIn("Require Cachix publication for release tags", nix_workflow)
        self.assertIn("nix build .#packages.${{ matrix.system }}.jcode", nix_workflow)
        self.assertIn("needs: [validate, build]", nix_workflow)
        self.assertIn("uses: ./.github/workflows/release.yml", nix_workflow)

        ios_workflow = (ROOT / ".github/workflows/ios.yml").read_text()
        self.assertIn("CODE_SIGNING_ALLOWED=NO", ios_workflow)
        for banned in ("xcodebuild archive", "-exportArchive", "APPSTORE_API_KEY"):
            with self.subTest(token=banned):
                self.assertNotIn(banned, ios_workflow)

    def test_runtime_update_commands_cannot_acquire_or_replace_jcode(self) -> None:
        update = (ROOT / "crates/jcode-app-core/src/update.rs").read_text()
        hot_exec = (ROOT / "src/cli/hot_exec.rs").read_text()
        tui_maintenance = (ROOT / "crates/jcode-tui/src/tui/app/state_ui_maintenance.rs").read_text()

        self.assertIn("NIX_UPDATE_GUIDANCE", update)
        self.assertIn("NIX_UPDATE_GUIDANCE", hot_exec)
        self.assertIn("NIX_UPDATE_GUIDANCE", tui_maintenance)
        for banned in (
            "download_and_install",
            "verify_asset_checksum",
            "SHA256SUMS",
            "releases/download",
            "api.github.com",
            "reqwest",
        ):
            with self.subTest(token=banned):
                self.assertNotIn(banned, update)

        runtime_sources = list((ROOT / "src").rglob("*.rs")) + list(
            (ROOT / "crates").rglob("*.rs")
        )
        for source in runtime_sources:
            text = source.read_text()
            for banned in (
                "api.github.com/repos/1jehuang/jcode/releases",
                "github.com/1jehuang/jcode/releases/download",
                "github.com/jerudnik/jcode/releases/download",
            ):
                with self.subTest(source=source.relative_to(ROOT), token=banned):
                    self.assertNotIn(banned, text)

    def test_retired_update_flags_have_no_active_callers(self) -> None:
        retired_flags = ("--no" + "-update", "--auto" + "-update")
        roots = ("scripts", "tests", "src", "crates", ".github/workflows")
        suffixes = {".py", ".sh", ".rs", ".yml", ".yaml"}
        for root in roots:
            for path in (ROOT / root).rglob("*"):
                if not path.is_file() or path.suffix not in suffixes:
                    continue
                text = path.read_text(errors="ignore")
                for flag in retired_flags:
                    with self.subTest(path=path.relative_to(ROOT), flag=flag):
                        self.assertNotIn(flag, text)

    def test_workspace_crates_cannot_publish_to_a_registry(self) -> None:
        root_manifest = tomllib.loads((ROOT / "Cargo.toml").read_text())
        members = root_manifest["workspace"]["members"]
        missing: list[str] = []
        for member in members:
            manifest = ROOT / member / "Cargo.toml" if member != "." else ROOT / "Cargo.toml"
            package = tomllib.loads(manifest.read_text()).get("package", {})
            if package.get("publish") is not False:
                missing.append(str(manifest.relative_to(ROOT)))
        self.assertEqual([], missing, f"workspace crates missing `publish = false`: {missing}")

    def test_active_distribution_docs_do_not_advertise_retired_channels(self) -> None:
        for relative in ACTIVE_DISTRIBUTION_DOCS:
            path = ROOT / relative
            self.assertTrue(path.exists(), f"active distribution document missing: {relative}")
            text = path.read_text()
            for banned in FORBIDDEN_ACTIVE_DOC_TEXT:
                with self.subTest(path=relative, token=banned):
                    self.assertNotIn(banned, text)


if __name__ == "__main__":
    unittest.main(verbosity=2)
