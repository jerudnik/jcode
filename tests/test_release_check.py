from __future__ import annotations

import os
import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPT = REPO_ROOT / "scripts" / "release_check.py"


def run(cmd: list[str], cwd: Path) -> str:
    # The fixture repositories must not inherit the developer's git config.
    # A global `tag.gpgsign = true`, for example, turns the plain `git tag`
    # calls below into annotated signed tags, which then fail with
    # "Terminal is dumb, but EDITOR unset" on a machine that has it set and
    # pass on a CI runner that does not.
    env = dict(os.environ, GIT_CONFIG_GLOBAL=os.devnull, GIT_CONFIG_SYSTEM=os.devnull)
    result = subprocess.run(
        cmd, cwd=cwd, env=env, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE
    )
    if result.returncode != 0:
        raise AssertionError(f"{cmd} failed\nstdout={result.stdout}\nstderr={result.stderr}")
    return result.stdout.strip()


class ReleaseCheckTests(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        run(["git", "init", "-b", "main"], self.root)
        run(["git", "config", "user.name", "Release Tester"], self.root)
        run(["git", "config", "user.email", "release@example.invalid"], self.root)
        self.write("Cargo.toml", '[package]\nname = "jcode"\nversion = "1.0.0"\n')
        self.write("flake.nix", "{ jcode-provenance = {}; jcode-sbom = {}; packages = { jcode = {}; }; }\n")
        self.write("scripts/generate_release_notes.sh", "#!/usr/bin/env bash\necho notes\n")
        os.chmod(self.root / "scripts/generate_release_notes.sh", 0o755)
        self.write(
            ".github/workflows/release.yml",
            textwrap.dedent(
                """
                name: Release metadata
                on:
                  push:
                    tags:
                      - 'v*'
                jobs:
                  release:
                    steps:
                      - run: scripts/release_check.py
                      - run: echo metadata-only GitHub release
                      - run: echo 'assets | length'
                """
            ).lstrip(),
        )
        self.write(
            "CHANGELOG.md",
            textwrap.dedent(
                """
                # Changelog

                ## v1.0.0

                This is the official first fork release. It uses Nix and Cachix,
                SBOM, provenance, metadata-only GitHub Release, and rollback proof.

                ### Rollback test

                nix run github:jerudnik/jcode/rollback-ok --accept-flake-config -- version
                """
            ).lstrip(),
        )
        self.write("README.md", "fixture\n")
        run(["git", "add", "Cargo.toml", "flake.nix", "scripts/generate_release_notes.sh", ".github/workflows/release.yml", "CHANGELOG.md", "README.md"], self.root)
        run(["git", "commit", "-m", "initial"], self.root)
        run(["git", "tag", "rollback-ok"], self.root)
        self.write("README.md", "fixture v1\n")
        run(["git", "add", "README.md"], self.root)
        run(["git", "commit", "-m", "release prep"], self.root)
        run(["git", "tag", "v1.0.0"], self.root)

    def tearDown(self) -> None:
        self.tmp.cleanup()

    def write(self, relative: str, content: str) -> None:
        path = self.root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")

    def release_check(self, *args: str) -> subprocess.CompletedProcess[str]:
        env = os.environ.copy()
        env["RELEASE_CHECK_ROOT"] = str(self.root)
        return subprocess.run(
            [sys.executable, str(SCRIPT), *args],
            cwd=REPO_ROOT,
            env=env,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )

    def test_default_v1_release_passes_with_main_ancestor_rollback(self) -> None:
        result = self.release_check("--rollback-ref", "rollback-ok", "--require-tag")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("release check passed for v1.0.0", result.stdout)

    def test_v0_candidate_is_rejected_as_release_proof(self) -> None:
        result = self.release_check("--release", "v0.46.0", "--rollback-ref", "rollback-ok")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("v0 tag", result.stderr)

    def test_v0_rollback_ref_is_rejected(self) -> None:
        run(["git", "tag", "v0.46.0", "rollback-ok"], self.root)
        result = self.release_check("--rollback-ref", "v0.46.0")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("rollback cannot rely on a v0 tag", result.stderr)

    def test_cargo_version_must_match_release_name(self) -> None:
        self.write("Cargo.toml", '[package]\nname = "jcode"\nversion = "1.0.1"\n')
        result = self.release_check("--rollback-ref", "rollback-ok", "--allow-dirty")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("must match v1.0.0", result.stderr)

    def test_missing_rollback_ref_fails(self) -> None:
        result = self.release_check("--allow-dirty")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("--rollback-ref is required", result.stderr)

    def test_required_tag_must_point_to_head(self) -> None:
        self.write("README.md", "fixture after tag\n")
        run(["git", "add", "README.md"], self.root)
        run(["git", "commit", "-m", "after tag"], self.root)
        result = self.release_check("--rollback-ref", "rollback-ok", "--require-tag")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("must point at HEAD", result.stderr)


if __name__ == "__main__":
    unittest.main()
