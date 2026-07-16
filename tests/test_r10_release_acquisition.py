#!/usr/bin/env python3
"""Hermetic R10 release-acquisition tests.

These tests exercise scripts/install.sh through a fake curl and disposable HOME /
JCODE_INSTALL_DIR fixtures. They never contact the network, mutate a real shell
profile, or talk to a live daemon.
"""

from __future__ import annotations

import hashlib
import os
import shutil
import stat
import subprocess
import textwrap
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory


REPO = Path(__file__).resolve().parents[1]
INSTALL_SH = REPO / "scripts" / "install.sh"
UPDATE_RS = REPO / "crates" / "jcode-app-core" / "src" / "update.rs"
RELEASE_YML = REPO / ".github" / "workflows" / "release.yml"
QUICK_RELEASE_SH = REPO / "scripts" / "quick-release.sh"


class InstallFixture:
    def __init__(self, case: unittest.TestCase, checksum_mode: str) -> None:
        self.case = case
        self.tmp = TemporaryDirectory()
        self.root = Path(self.tmp.name)
        self.home = self.root / "home"
        self.install_dir = self.root / "install-bin"
        self.fakebin = self.root / "fakebin"
        self.fixture = self.root / "fixture"
        self.reload_log = self.root / "reload.log"
        self.version = "v9.8.7"
        self.artifact = self._artifact_name()
        self.asset_path = self.fixture / "asset"
        self.checksum_mode = checksum_mode

        self.home.mkdir()
        self.install_dir.mkdir()
        self.fakebin.mkdir()
        self.fixture.mkdir()
        self._write_asset()
        self._write_fake_curl()
        self._seed_existing_install()

    def cleanup(self) -> None:
        self.tmp.cleanup()

    def _artifact_name(self) -> str:
        uname = subprocess.check_output(["uname", "-s"], text=True).strip()
        arch = subprocess.check_output(["uname", "-m"], text=True).strip()
        if uname == "Darwin":
            if arch == "arm64":
                return "jcode-macos-aarch64"
            if arch == "x86_64":
                return "jcode-macos-x86_64"
        if uname == "Linux":
            if arch == "x86_64":
                return "jcode-linux-x86_64"
            if arch in {"aarch64", "arm64"}:
                return "jcode-linux-aarch64"
        raise AssertionError(f"unsupported test platform: {uname} {arch}")

    def _write_asset(self) -> None:
        script = textwrap.dedent(
            """\
            #!/usr/bin/env bash
            set -euo pipefail
            if [ "${1:-}" = "--version" ]; then
              echo "jcode 9.8.7"
              exit 0
            fi
            if [ "${1:-}" = "setup-hotkey" ]; then
              exit 1
            fi
            if [ "${1:-}" = "server" ] && [ "${2:-}" = "reload" ]; then
              printf 'reload %s\\n' "$(pwd)" >> "${JCODE_RELOAD_LOG:?}"
              exit 0
            fi
            exit 0
            """
        )
        self.asset_path.write_text(script)
        self.asset_path.chmod(self.asset_path.stat().st_mode | stat.S_IXUSR)

    def _write_fake_curl(self) -> None:
        curl = self.fakebin / "curl"
        curl.write_text(
            textwrap.dedent(
                """\
                #!/usr/bin/env bash
                set -euo pipefail
                out=""
                url=""
                while [ "$#" -gt 0 ]; do
                  case "$1" in
                    -o)
                      out="$2"
                      shift 2
                      ;;
                    -*)
                      shift
                      ;;
                    *)
                      url="$1"
                      shift
                      ;;
                  esac
                done
                [ -n "$url" ] || exit 2
                case "$url" in
                  */releases/latest)
                    printf '{"tag_name":"%s"}\n' "$JCODE_FAKE_VERSION"
                    ;;
                  */"$JCODE_FAKE_VERSION"/"$JCODE_FAKE_ARTIFACT".tar.gz)
                    exit 22
                    ;;
                  */"$JCODE_FAKE_VERSION"/"$JCODE_FAKE_ARTIFACT")
                    [ -n "$out" ] || exit 2
                    cp "$JCODE_FAKE_ASSET" "$out"
                    ;;
                  */"$JCODE_FAKE_VERSION"/SHA256SUMS)
                    [ -n "$out" ] || exit 2
                    case "$JCODE_FAKE_CHECKSUM_MODE" in
                      missing)
                        exit 22
                        ;;
                      mismatch)
                        printf '%064d  %s\n' 0 "$JCODE_FAKE_ARTIFACT" > "$out"
                        ;;
                      ok)
                        printf '%s  %s\n' "$JCODE_FAKE_DIGEST" "$JCODE_FAKE_ARTIFACT" > "$out"
                        ;;
                      *)
                        exit 2
                        ;;
                    esac
                    ;;
                  *)
                    echo "unexpected curl url: $url" >&2
                    exit 22
                    ;;
                esac
                """
            )
        )
        curl.chmod(curl.stat().st_mode | stat.S_IXUSR)

    def _seed_existing_install(self) -> None:
        builds = self.home / ".jcode" / "builds"
        old_version = builds / "versions" / "old" / "jcode"
        stable = builds / "stable"
        current = builds / "current"
        stable.mkdir(parents=True)
        current.mkdir(parents=True)
        old_version.parent.mkdir(parents=True)
        old_version.write_text("old binary\n")
        (stable / "jcode").symlink_to(old_version)
        (current / "jcode").symlink_to(old_version)
        (builds / "stable-version").write_text("old\n")
        (builds / "current-version").write_text("old\n")
        launcher = self.install_dir / "jcode"
        launcher.write_text("old launcher\n")
        launcher.chmod(0o755)

    def env(self, *, reload_server: bool = False) -> dict[str, str]:
        digest = hashlib.sha256(self.asset_path.read_bytes()).hexdigest()
        env = os.environ.copy()
        env.update(
            {
                "PATH": f"{self.fakebin}:/usr/bin:/bin:/usr/sbin:/sbin",
                "HOME": str(self.home),
                "XDG_CONFIG_HOME": str(self.root / "xdg-config"),
                "JCODE_INSTALL_DIR": str(self.install_dir),
                "JCODE_RELOAD_LOG": str(self.reload_log),
                "JCODE_FAKE_VERSION": self.version,
                "JCODE_FAKE_ARTIFACT": self.artifact,
                "JCODE_FAKE_ASSET": str(self.asset_path),
                "JCODE_FAKE_DIGEST": digest,
                "JCODE_FAKE_CHECKSUM_MODE": self.checksum_mode,
                "JCODE_NO_TELEMETRY": "1",
                "FORK_NUDGE_MAX_AGE": "2147483647",
                "FORK_NUDGE_AUTOSYNC": "0",
            }
        )
        if reload_server:
            env["JCODE_RELOAD_SERVER"] = "1"
        else:
            env.pop("JCODE_RELOAD_SERVER", None)
        return env

    def run_install(self, *, reload_server: bool = False) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            ["bash", str(INSTALL_SH)],
            cwd=REPO,
            env=self.env(reload_server=reload_server),
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=30,
        )

    def state(self) -> dict[str, object]:
        builds = self.home / ".jcode" / "builds"
        return {
            "stable_target": os.readlink(builds / "stable" / "jcode"),
            "current_target": os.readlink(builds / "current" / "jcode"),
            "stable_version": (builds / "stable-version").read_text(),
            "current_version": (builds / "current-version").read_text(),
            "launcher": (self.install_dir / "jcode").read_text(),
            "versions": sorted(p.name for p in (builds / "versions").iterdir()),
            "reload_log_exists": self.reload_log.exists(),
        }


class R10ReleaseAcquisitionTests(unittest.TestCase):
    def make_fixture(self, checksum_mode: str) -> InstallFixture:
        fixture = InstallFixture(self, checksum_mode)
        self.addCleanup(fixture.cleanup)
        return fixture

    def assert_failed_install_preserves_markers(self, mode: str, expected_message: str) -> None:
        fixture = self.make_fixture(mode)
        before = fixture.state()
        result = fixture.run_install()
        self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn(expected_message, result.stderr)
        self.assertEqual(fixture.state(), before)

    def test_missing_sha256sums_leaves_existing_install_unchanged(self) -> None:
        self.assert_failed_install_preserves_markers(
            "missing",
            "does not include SHA256SUMS",
        )

    def test_mismatched_sha256sum_leaves_existing_install_unchanged(self) -> None:
        self.assert_failed_install_preserves_markers(
            "mismatch",
            "Checksum mismatch",
        )

    def test_verified_asset_promotes_exactly_one_version_without_default_reload(self) -> None:
        fixture = self.make_fixture("ok")
        shutil.rmtree(fixture.home / ".jcode")
        (fixture.install_dir / "jcode").unlink()

        result = fixture.run_install()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

        builds = fixture.home / ".jcode" / "builds"
        versions = sorted(p.name for p in (builds / "versions").iterdir())
        self.assertEqual(versions, ["9.8.7"])
        self.assertEqual((builds / "stable-version").read_text(), "9.8.7\n")
        self.assertEqual(os.readlink(builds / "stable" / "jcode"), str(builds / "versions" / "9.8.7" / "jcode"))
        self.assertEqual(os.readlink(fixture.install_dir / "jcode"), str(builds / "stable" / "jcode"))
        self.assertFalse(fixture.reload_log.exists(), "server reload must be default-off")

    def test_reload_is_explicit_opt_in_only(self) -> None:
        default_fixture = self.make_fixture("ok")
        result = default_fixture.run_install()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertFalse(default_fixture.reload_log.exists(), "default install unexpectedly reloaded daemon")

        opt_in_fixture = self.make_fixture("ok")
        result = opt_in_fixture.run_install(reload_server=True)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(opt_in_fixture.reload_log.read_text().count("reload "), 1)

    def test_rust_updater_requires_sha256sums_asset(self) -> None:
        source = UPDATE_RS.read_text()
        self.assertIn("fn verify_asset_checksum_required", source)
        self.assertIn("does not include SHA256SUMS; refusing to install unchecked asset", source)
        self.assertIn("verify_asset_checksum_text(&contents, &asset.name, bytes)?", source)
        self.assertNotIn("skipping checksum verification", source)

    def test_release_entrypoints_are_draft_only_until_checksums_publish(self) -> None:
        release_yml = RELEASE_YML.read_text()
        quick_release = QUICK_RELEASE_SH.read_text()

        self.assertIn("Create draft release if missing", release_yml)
        self.assertIn('gh release create "${GITHUB_REF_NAME}"', release_yml)
        self.assertIn("--draft", release_yml)
        self.assertIn("Upload checksums to release", release_yml)
        self.assertIn("Publish completed release", release_yml)
        self.assertLess(
            release_yml.index("Upload checksums to release"),
            release_yml.index("Publish completed release"),
        )
        self.assertLess(
            release_yml.index("Generate checksums"),
            release_yml.index("Publish completed release"),
        )
        self.assertNotIn("softprops/action-gh-release", release_yml)

        self.assertIn("stages a draft release", quick_release)
        self.assertIn("▸ Staging GitHub draft release", quick_release)
        self.assertIn('gh release create "$VERSION"', quick_release)
        self.assertIn("--draft", quick_release)
        self.assertIn("gh release upload \"$VERSION\"", quick_release)
        self.assertIn("attached to draft", quick_release)
        self.assertIn("CI: building, signing, and publishing the complete release", quick_release)
        self.assertIn("The release becomes visible after all required platform gates pass.", quick_release)
        self.assertNotIn("--draft=false", quick_release)
        self.assertNotIn("--generate-notes", quick_release)
        self.assertNotIn("Users can now: jcode update", quick_release)
        self.assertNotIn("gh issue close", quick_release)


if __name__ == "__main__":
    unittest.main(verbosity=2)
