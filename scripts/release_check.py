#!/usr/bin/env python3
"""Validate the fork release promotion contract before publication.

The checker is intentionally conservative. It treats the first official fork
release as v1.0.0 by default and refuses to use pre-fork v0 tags as release
proof. GitHub Releases remain metadata-only; Nix and Cachix are the binary
channel, so this script verifies the release workflow can point at cached Nix
outputs, SBOM, provenance, install, and rollback evidence instead of building a
second artifact stream.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
from pathlib import Path

DEFAULT_RELEASE = "v1.0.0"
TAG_RE = re.compile(r"^v(?P<major>\d+)\.(?P<minor>\d+)\.(?P<patch>\d+)(?:[-+][0-9A-Za-z.-]+)?$")


class ReleaseCheckError(RuntimeError):
    pass


def run_git(args: list[str], *, cwd: Path) -> str:
    result = subprocess.run(
        ["git", *args],
        cwd=cwd,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if result.returncode != 0:
        command = "git " + " ".join(args)
        raise ReleaseCheckError(f"{command} failed: {result.stderr.strip() or result.stdout.strip()}")
    return result.stdout.strip()


def parse_version(tag: str) -> str:
    match = TAG_RE.match(tag)
    if not match:
        raise ReleaseCheckError(f"release name {tag!r} must be a semver tag like v1.0.0")
    if int(match.group("major")) == 0:
        raise ReleaseCheckError(
            f"{tag} is a v0 tag; old pre-fork tags are not official fork release proof"
        )
    return tag[1:]


def cargo_version(root: Path) -> str:
    cargo = root / "Cargo.toml"
    for line in cargo.read_text(encoding="utf-8").splitlines():
        if line.startswith("version = "):
            return line.split("=", 1)[1].strip().strip('"')
    raise ReleaseCheckError("Cargo.toml does not contain a root version")


def assert_clean(root: Path, allow_dirty: bool) -> None:
    if allow_dirty:
        return
    dirty = run_git(["status", "--porcelain"], cwd=root)
    if dirty:
        raise ReleaseCheckError("worktree must be clean before tagging or publishing")


def assert_head_on_main(root: Path, main_ref: str) -> None:
    head = run_git(["rev-parse", "HEAD"], cwd=root)
    main = run_git(["rev-parse", main_ref], cwd=root)
    if head != main:
        raise ReleaseCheckError(f"HEAD {head} must match authoritative {main_ref} {main}")


def assert_tag(root: Path, tag: str, main_ref: str, require_tag: bool) -> None:
    if not require_tag:
        return
    tag_commit = run_git(["rev-list", "-n", "1", tag], cwd=root)
    head = run_git(["rev-parse", "HEAD"], cwd=root)
    if tag_commit != head:
        raise ReleaseCheckError(f"{tag} must point at HEAD before publication")
    subprocess.run(
        ["git", "merge-base", "--is-ancestor", tag_commit, main_ref],
        cwd=root,
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )


def assert_changelog(root: Path, version: str) -> None:
    changelog = root / "CHANGELOG.md"
    if not changelog.exists():
        raise ReleaseCheckError("CHANGELOG.md must document the candidate release")
    text = changelog.read_text(encoding="utf-8")
    required = [
        f"## v{version}",
        "official first fork release",
        "metadata-only GitHub Release",
        "Nix and Cachix",
        "SBOM",
        "provenance",
        "rollback",
    ]
    missing = [item for item in required if item not in text]
    if missing:
        raise ReleaseCheckError("CHANGELOG.md is missing release proof terms: " + ", ".join(missing))


def assert_release_notes(root: Path, tag: str, version: str) -> None:
    script = root / "scripts" / "generate_release_notes.sh"
    if not script.exists():
        raise ReleaseCheckError("scripts/generate_release_notes.sh is required for metadata release notes")
    changelog_json = root / "changelog" / f"v{version}.json"
    if changelog_json.exists():
        data = json.loads(changelog_json.read_text(encoding="utf-8"))
        if data.get("version") != version:
            raise ReleaseCheckError(f"{changelog_json} version must be {version}")
    else:
        # First fork release may use CHANGELOG.md as the reviewed summary, but
        # the notes generator must still be able to fall back to commits.
        run_git(["log", "-1", "--pretty=%s"], cwd=root)


def assert_workflow(root: Path) -> None:
    workflow = root / ".github" / "workflows" / "release.yml"
    text = workflow.read_text(encoding="utf-8")
    required = [
        "tags:",
        "v*",
        "scripts/release_check.py",
        "metadata-only GitHub release",
        "assets | length",
    ]
    missing = [item for item in required if item not in text]
    if missing:
        raise ReleaseCheckError("release workflow is missing required publication safeguards: " + ", ".join(missing))


def assert_nix_evidence(root: Path) -> None:
    flake = (root / "flake.nix").read_text(encoding="utf-8")
    required = ["jcode-provenance", "jcode-sbom", "packages", "jcode"]
    missing = [item for item in required if item not in flake]
    if missing:
        raise ReleaseCheckError("flake.nix is missing release evidence outputs: " + ", ".join(missing))


def assert_rollback(root: Path, rollback_ref: str | None, main_ref: str) -> None:
    changelog = (root / "CHANGELOG.md").read_text(encoding="utf-8")
    if "Rollback test" not in changelog:
        raise ReleaseCheckError("CHANGELOG.md must include a rollback test command")
    if not rollback_ref:
        raise ReleaseCheckError("--rollback-ref is required so rollback stays testable")
    if rollback_ref.startswith("v0"):
        raise ReleaseCheckError("rollback cannot rely on a v0 tag as official fork release proof")
    rollback_commit = run_git(["rev-parse", f"{rollback_ref}^{{commit}}"], cwd=root)
    subprocess.run(
        ["git", "merge-base", "--is-ancestor", rollback_commit, main_ref],
        cwd=root,
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--release", default=DEFAULT_RELEASE, help="candidate fork release tag")
    parser.add_argument("--main-ref", default="main", help="authoritative main ref to compare against")
    parser.add_argument("--rollback-ref", help="previous promoted fork release or main ancestor rollback ref")
    parser.add_argument("--allow-dirty", action="store_true", help="allow dirty worktree for local development tests")
    parser.add_argument("--require-tag", action="store_true", help="require the candidate tag to exist and point at HEAD")
    parser.add_argument("--skip-main-head", action="store_true", help="do not require HEAD to equal --main-ref")
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    root = Path(os.environ.get("RELEASE_CHECK_ROOT", ".")).resolve()

    try:
        version = parse_version(args.release)
        assert_clean(root, args.allow_dirty)
        if not args.skip_main_head:
            assert_head_on_main(root, args.main_ref)
        if cargo_version(root) != version:
            raise ReleaseCheckError(
                f"Cargo.toml version {cargo_version(root)} must match {args.release}; "
                "use v1.0.0 unless the operator chose another name before tagging"
            )
        assert_changelog(root, version)
        assert_release_notes(root, args.release, version)
        assert_workflow(root)
        assert_nix_evidence(root)
        assert_rollback(root, args.rollback_ref, args.main_ref)
        assert_tag(root, args.release, args.main_ref, args.require_tag)
    except (ReleaseCheckError, subprocess.CalledProcessError) as exc:
        print(f"release check failed: {exc}", file=sys.stderr)
        return 1

    print(f"release check passed for {args.release}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
