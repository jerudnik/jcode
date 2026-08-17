#!/usr/bin/env python3
"""Every git dependency in Cargo.lock must have a matching outputHashes pin.

`nix/package.nix` pins fixed-output hashes for Cargo git dependencies so
vendoring never needs network access (hosted macOS CI is flaky without this).
The pins are keyed by the full Cargo.lock `source` string, including the
`?tag=` query and the `#<rev>` fragment.

A stale key fails silently: Nix prints "No output hash provided" as an
evaluation *warning* and falls back to fetching over the network. The build
still succeeds locally, so nothing surfaces the drift. This happened once
already, where the pins named agentgrep v0.1.2/v0.1.3 and mermaid-rs-renderer
v0.2.1 while the lockfile had moved to v0.1.6 and v0.3.1.

This check makes that drift a hard failure instead of a warning.
"""

from __future__ import annotations

import re
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CARGO_LOCK = ROOT / "Cargo.lock"
PACKAGE_NIX = ROOT / "nix" / "package.nix"


def locked_git_sources() -> dict[str, str]:
    """Map each git `source` string in Cargo.lock to its package name."""
    sources: dict[str, str] = {}
    for block in CARGO_LOCK.read_text().split("[[package]]"):
        source = re.search(r'^source = "(git\+[^"]+)"', block, re.MULTILINE)
        if not source:
            continue
        name = re.search(r'^name = "([^"]+)"', block, re.MULTILINE)
        sources[source.group(1)] = name.group(1) if name else "<unknown>"
    return sources


def pinned_hash_keys() -> set[str]:
    """Every key in the `outputHashes` attrset of nix/package.nix."""
    text = PACKAGE_NIX.read_text()
    block = re.search(r"outputHashes\s*=\s*\{(.*?)\n    \};", text, re.DOTALL)
    if not block:
        raise SystemExit("could not locate the outputHashes attrset in nix/package.nix")
    return set(re.findall(r'"(git\+[^"]+)"\s*=', block.group(1)))


def main() -> int:
    locked = locked_git_sources()
    pinned = pinned_hash_keys()

    missing = sorted(set(locked) - pinned)
    stale = sorted(pinned - set(locked))
    failed = False

    for source in missing:
        print(
            f"error: {locked[source]} is a git dependency in Cargo.lock with no "
            f"outputHashes pin in nix/package.nix:\n  {source}",
            file=sys.stderr,
        )
        failed = True

    for source in stale:
        print(
            "error: nix/package.nix pins an outputHash for a source that is no "
            f"longer in Cargo.lock (it is silently ignored):\n  {source}",
            file=sys.stderr,
        )
        failed = True

    if failed:
        print(
            "\nRe-pin with:\n"
            "  nix run nixpkgs#nix-prefetch-git -- --quiet --url <url> "
            "--rev <rev> --fetch-submodules | jq -r .hash\n"
            "The key must be the Cargo.lock `source` string verbatim.",
            file=sys.stderr,
        )
        return 1

    print(f"ok: {len(locked)} git dependency pin(s) match Cargo.lock exactly")
    return 0


class CargoGitOutputHashTests(unittest.TestCase):
    """Expose the same comparison to `unittest` so `just check` runs it."""

    def test_every_locked_git_source_is_pinned(self) -> None:
        missing = sorted(set(locked_git_sources()) - pinned_hash_keys())
        self.assertEqual(
            missing,
            [],
            "git dependencies in Cargo.lock with no outputHashes pin in "
            "nix/package.nix; Nix would fetch these over the network",
        )

    def test_no_pin_names_a_source_that_left_the_lockfile(self) -> None:
        stale = sorted(pinned_hash_keys() - set(locked_git_sources()))
        self.assertEqual(
            stale,
            [],
            "nix/package.nix pins an outputHash for a source that is no longer "
            "in Cargo.lock; the pin is silently ignored",
        )

    def test_the_lockfile_actually_has_git_sources(self) -> None:
        # Without this, both checks above pass vacuously if the parser breaks
        # or every git dependency is dropped from Cargo.lock.
        self.assertGreater(len(locked_git_sources()), 0)


if __name__ == "__main__":
    raise SystemExit(main())
