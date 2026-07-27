#!/usr/bin/env python3
"""Tests for scripts/docs_impact_advisory.py."""

from __future__ import annotations

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


sys.path.insert(0, str(Path(__file__).resolve().parent))

import docs_impact_advisory as advisory  # noqa: E402


class DocsImpactAdvisoryTests(unittest.TestCase):
    def test_globs_are_path_segment_aware(self) -> None:
        self.assertFalse(advisory.matches("docs/*", "docs/guides/setup.md"))
        self.assertTrue(advisory.matches("docs/**", "docs/guides/setup.md"))
        self.assertTrue(advisory.matches("**/*.py", "tool.py"))
        self.assertTrue(advisory.matches("**/*.py", "src/tool.py"))

    def test_discovers_and_groups_nearest_scopes_with_parent_sources(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            primitive_dir = root / ".apm" / "instructions"
            primitive_dir.mkdir(parents=True)
            (primitive_dir / "root.instructions.md").write_text(
                '---\napplyTo: "**"\n---\nroot\n', encoding="utf-8"
            )
            (primitive_dir / "docs.instructions.md").write_text(
                '---\napplyTo: "docs/**"\n---\ndocs\n', encoding="utf-8"
            )

            scopes = advisory.discover_scopes(root)
            groups = advisory.group_impacts(
                ["crates/core/src/lib.rs", "docs/guide.md"], scopes
            )

            self.assertEqual([group.pattern for group in groups], ["**", "docs/**"])
            docs_group = groups[1]
            self.assertEqual(docs_group.paths, ("docs/guide.md",))
            self.assertEqual(
                docs_group.sources,
                (
                    ".apm/instructions/docs.instructions.md",
                    ".apm/instructions/root.instructions.md",
                ),
            )

    def test_scope_declaration_must_be_in_frontmatter(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            primitive_dir = root / ".apm" / "instructions"
            primitive_dir.mkdir(parents=True)
            (primitive_dir / "invalid.instructions.md").write_text(
                '# invalid\n\napplyTo: "**"\n', encoding="utf-8"
            )

            with self.assertRaisesRegex(ValueError, "missing YAML frontmatter"):
                advisory.discover_scopes(root)

    def test_changed_paths_uses_complete_three_dot_diff(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.run_git(root, "init", "-q")
            self.run_git(root, "config", "user.name", "Test")
            self.run_git(root, "config", "user.email", "test@example.invalid")
            self.run_git(root, "config", "commit.gpgsign", "false")
            (root / "old.txt").write_text("old\n", encoding="utf-8")
            self.run_git(root, "add", "old.txt")
            self.run_git(root, "commit", "-qm", "base")
            base = self.run_git(root, "rev-parse", "HEAD").strip()

            (root / "old.txt").rename(root / "new.txt")
            (root / "docs").mkdir()
            (root / "docs" / "guide.md").write_text("guide\n", encoding="utf-8")
            self.run_git(root, "add", "-A")
            self.run_git(root, "commit", "-qm", "head")
            head = self.run_git(root, "rev-parse", "HEAD").strip()

            self.assertEqual(
                advisory.changed_paths(root, base, head),
                ["docs/guide.md", "new.txt", "old.txt"],
            )

    def test_packet_is_explicitly_advisory_and_limits_large_path_lists(self) -> None:
        paths = [f"docs/file-{index}.md" for index in range(25)]
        group = advisory.ImpactGroup(
            pattern="docs/**",
            paths=tuple(paths),
            sources=(".apm/instructions/dox-docs.instructions.md",),
        )

        packet = advisory.render_markdown("a" * 40, "b" * 40, paths, [group])

        self.assertIn("This check is advisory", packet)
        self.assertIn("If none changed, no documentation edit is required", packet)
        self.assertIn(
            "**Documentation or instruction paths changed:** 25  \n"
            "**Scope matching:** best-effort",
            packet,
        )
        self.assertIn("... and 5 more", packet)
        self.assertNotIn("docs/file-24.md", packet)

    @staticmethod
    def run_git(root: Path, *args: str) -> str:
        completed = subprocess.run(
            ["git", "-C", str(root), *args],
            check=True,
            capture_output=True,
            text=True,
        )
        return completed.stdout


if __name__ == "__main__":
    unittest.main()
