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
SENTINEL = "hook-ran-from-checkout"

GIT_ENV = {
    "GIT_AUTHOR_NAME": "test",
    "GIT_AUTHOR_EMAIL": "test@example.invalid",
    "GIT_COMMITTER_NAME": "test",
    "GIT_COMMITTER_EMAIL": "test@example.invalid",
    "GIT_CONFIG_GLOBAL": os.devnull,
    "GIT_CONFIG_SYSTEM": os.devnull,
}


def run(args, cwd, check=True):
    env = dict(os.environ)
    env.update(GIT_ENV)
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
        for name in ("pre-commit", "pre-push"):
            stub = self.primary / "scripts" / "git-hooks" / name
            stub.write_text(
                "#!/usr/bin/env bash\n"
                'echo "%s body=${BASH_SOURCE[0]}"\n' % SENTINEL
            )
            stub.chmod(0o755)
        shutil.copy2(INSTALLER, self.primary / "scripts" / "install-git-hooks.sh")
        run(["git", "add", "-A"], self.primary)
        run(["git", "commit", "-q", "-m", "seed"], self.primary)
        run(["git", "worktree", "add", "-q", "-b", "wt", str(self.linked)], self.primary)

    def install_from(self, cwd):
        return run(["bash", "scripts/install-git-hooks.sh"], cwd)

    def hook(self, name):
        return self.primary / ".git" / "hooks" / name

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
        self.assertIn("body=%s/scripts/git-hooks/pre-commit" % self.linked, result.stdout)

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
