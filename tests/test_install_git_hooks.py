"""Regression tests for scripts/install-git-hooks.sh shim generation.

Linked worktrees share a single .git/hooks directory. A shim that bakes in the
repository path it was installed from is therefore wrong for every other
checkout, and breaks all of them once the installing worktree is removed:
`git commit` then fails with "No such file or directory" from the hook, which
reads like a broken repository rather than a broken hook.

These tests drive the installer from a linked worktree, delete that worktree,
and assert the primary checkout can still run its hooks.
"""

import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
INSTALLER = REPO_ROOT / "scripts" / "install-git-hooks.sh"
PRECOMMIT = REPO_ROOT / "scripts" / "git-hooks" / "pre-commit"
SENTINEL = "hook-ran-from-checkout"

GIT_ENV = {
    "GIT_AUTHOR_NAME": "test",
    "GIT_AUTHOR_EMAIL": "test@example.invalid",
    "GIT_COMMITTER_NAME": "test",
    "GIT_COMMITTER_EMAIL": "test@example.invalid",
    "GIT_CONFIG_GLOBAL": os.devnull,
    "GIT_CONFIG_SYSTEM": os.devnull,
}


def run(args, cwd, check=True, extra_env=None):
    env = dict(os.environ)
    env.update(GIT_ENV)
    if extra_env:
        env.update(extra_env)
    return subprocess.run(
        args, cwd=str(cwd), env=env, check=check,
        stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True,
    )


class InstallGitHooksTest(unittest.TestCase):
    def setUp(self):
        # Resolve: on macOS the temp root is a symlink (/var -> /private/var)
        # and git reports the resolved form, so unresolved paths never match.
        self.tmp = Path(tempfile.mkdtemp(prefix="hookshim-")).resolve()
        self.addCleanup(shutil.rmtree, str(self.tmp), True)
        self.primary = self.tmp / "primary"
        self.linked = self.tmp / "linked"

        (self.primary / "scripts" / "git-hooks").mkdir(parents=True)
        run(["git", "init", "-q"], self.primary)
        precommit = self.primary / "scripts" / "git-hooks" / "pre-commit"
        shutil.copy2(PRECOMMIT, precommit)
        precommit.chmod(0o755)
        prepush = self.primary / "scripts" / "git-hooks" / "pre-push"
        prepush.write_text(
            "#!/usr/bin/env bash\n"
            'echo "%s body=${BASH_SOURCE[0]}"\n' % SENTINEL
        )
        prepush.chmod(0o755)

        surface = self.primary / "scripts" / "git-hooks" / "check-pm-surface.sh"
        surface.write_text(
            "#!/usr/bin/env bash\n"
            'echo "%s body=${BASH_SOURCE[0]}"\n' % SENTINEL
        )
        surface.chmod(0o755)

        (self.primary / "scripts" / "check_agent_instructions.py").write_text(
            'print("agent-instructions")\n'
        )
        (self.primary / "scripts" / "lint_docs.py").write_text(
            "import sys\n"
            "from pathlib import Path\n"
            'files_from = Path(sys.argv[sys.argv.index("--files-from") + 1])\n'
            'print("lint-docs files=" + files_from.read_text().strip())\n'
        )
        (self.primary / "scripts" / "check_docs_references.py").write_text(
            'print("docs-references")\n'
        )
        (self.primary / "source.py").write_text("print('source')\n")
        shutil.copy2(INSTALLER, self.primary / "scripts" / "install-git-hooks.sh")
        run(["git", "add", "-A"], self.primary)
        run(["git", "commit", "-q", "-m", "seed"], self.primary)
        run(["git", "worktree", "add", "-q", "-b", "wt", str(self.linked)], self.primary)

    def install_from(self, cwd):
        return run(["bash", "scripts/install-git-hooks.sh"], cwd)

    def hook(self, name):
        return self.primary / ".git" / "hooks" / name

    def path_without_vale(self):
        path = self.tmp / "no-vale-bin"
        path.mkdir()
        for name in ("bash", "git", "mktemp", "python3", "rm"):
            target = shutil.which(name)
            self.assertIsNotNone(target, "%s is required by the hook test" % name)
            (path / name).symlink_to(target)
        return str(path)

    def test_shim_does_not_bake_the_installing_worktree_path(self):
        self.install_from(self.linked)
        for name in ("pre-commit", "pre-push"):
            body = self.hook(name).read_text()
            self.assertNotIn(
                str(self.linked), body,
                "%s shim hard-codes the worktree it was installed from" % name,
            )

    def test_hooks_survive_deletion_of_the_installing_worktree(self):
        self.install_from(self.linked)
        shutil.rmtree(str(self.linked))
        run(["git", "worktree", "prune"], self.primary)

        for name in ("pre-commit", "pre-push"):
            result = run([str(self.hook(name))], self.primary, check=False)
            self.assertEqual(
                0, result.returncode,
                "%s failed after the installing worktree was removed: %s"
                % (name, result.stdout),
            )
            self.assertIn(SENTINEL, result.stdout)

    def test_each_worktree_runs_its_own_hook_body(self):
        # Installed from the primary checkout, then invoked from the linked one:
        # the shim must reach the caller's own scripts/git-hooks copy, not the
        # copy belonging to whichever worktree happened to run the installer.
        self.install_from(self.primary)
        result = run([str(self.hook("pre-commit"))], self.linked, check=False)
        self.assertEqual(0, result.returncode, result.stdout)
        self.assertIn(
            "body=%s/scripts/git-hooks/check-pm-surface.sh" % self.linked,
            result.stdout,
        )

    def test_precommit_runs_both_docs_checks_for_staged_markdown(self):
        self.install_from(self.primary)
        (self.primary / "README.md").write_text("# Hook test\n")
        run(["git", "add", "README.md"], self.primary)
        fake_bin = self.tmp / "bin"
        fake_bin.mkdir()
        vale = fake_bin / "vale"
        vale.write_text("#!/usr/bin/env bash\nexit 0\n")
        vale.chmod(0o755)

        result = run(
            [str(self.hook("pre-commit"))],
            self.primary,
            check=False,
            extra_env={"PATH": "%s:%s" % (fake_bin, os.environ["PATH"])},
        )

        self.assertEqual(0, result.returncode, result.stdout)
        self.assertIn("lint-docs files=README.md", result.stdout)
        self.assertIn("docs-references", result.stdout)

    def test_precommit_runs_reference_check_for_staged_rename(self):
        self.install_from(self.primary)
        run(["git", "mv", "source.py", "moved.py"], self.primary)

        result = run([str(self.hook("pre-commit"))], self.primary, check=False)

        self.assertEqual(0, result.returncode, result.stdout)
        self.assertNotIn("lint-docs", result.stdout)
        self.assertIn("docs-references", result.stdout)

    def test_precommit_skips_only_lint_when_vale_is_missing(self):
        self.install_from(self.primary)
        (self.primary / "README.md").write_text("# Hook test\n")
        run(["git", "add", "README.md"], self.primary)

        result = run(
            [str(self.hook("pre-commit"))],
            self.primary,
            check=False,
            extra_env={"PATH": self.path_without_vale()},
        )

        self.assertEqual(0, result.returncode, result.stdout)
        self.assertIn("vale is not on PATH; skipping prose lint", result.stdout)
        self.assertNotIn("lint-docs", result.stdout)
        self.assertIn("docs-references", result.stdout)

    def test_precommit_docs_bypass_keeps_existing_guards(self):
        self.install_from(self.primary)
        (self.primary / "README.md").write_text("# Hook test\n")
        run(["git", "add", "README.md"], self.primary)

        result = run(
            [str(self.hook("pre-commit"))],
            self.primary,
            check=False,
            extra_env={"DOCS_CHECKS_OK": "1"},
        )

        self.assertEqual(0, result.returncode, result.stdout)
        self.assertIn("agent-instructions", result.stdout)
        self.assertNotIn("lint-docs", result.stdout)
        self.assertNotIn("docs-references", result.stdout)

    def test_missing_hook_body_fails_closed(self):
        self.install_from(self.primary)
        (self.primary / "scripts" / "git-hooks" / "pre-commit").unlink()
        result = run([str(self.hook("pre-commit"))], self.primary, check=False)
        self.assertNotEqual(
            0, result.returncode,
            "a missing guard body must not be silently skipped",
        )
        self.assertIn("refusing to skip the guard", result.stdout)


if __name__ == "__main__":
    unittest.main()
