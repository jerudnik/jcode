# 2026-07-17 post-promotion checkpoint

This package closes the documentation gap between the original N2 candidate
signoff and the final promoted runtime, and records the private GitHub copy of
all worktree-backed branch history.

## Focused validation

Validation ran from clean `normalize/integration` source head
`42aa9cc64183741efb000a6d58c2c920de77e146`:

- CLI command tests: 41 passed, 0 failed.
- `cargo clippy -p jcode --lib -- -D warnings`: passed.
- `scripts/test_install_release.sh`: profile-qualified label test passed.
- `git diff --check`: passed.

The original 54-gate N2 matrix remains the authority for candidate
`62b3946b6`. This package does not mislabel the focused final-delta checks as a
full matrix rerun.

## Private archive

- Repository: <https://github.com/jerudnik/jcode-recovery-archive>
- Visibility: private
- Exact local branches copied: 41/41
- Detached worktree tip copied:
  `59c6a0ba0923b5ca9661611015abf8025bb72be2`
- Remote archive heads after copy: 42
- Local-versus-remote branch-tip mismatches: 0

A gitleaks scan of exactly the 41 local branches plus the detached worktree tip
inspected 6,210 commits and reported 19 test/example patterns. Every detected
commit was already an ancestor of public fork `main`; the private archive
introduced no newly unpublished finding.

GitHub does not preserve a worktree's directory placement, index, ignored files,
caches, or untracked state. Stashes and bundle payloads were deliberately not
uploaded because they require a separate user-content and privacy review.

## Files

- `local-branches.tsv`: local branch name and exact object ID.
- `worktrees.tsv`: worktree path, exact HEAD, branch, and detached flag.
- `private-archive-heads.tsv`: GitHub archive branch refs after the push.
- `archive-summary.txt`: counts, privacy state, and scan disposition.
- `gitleaks-disposition.tsv`: redacted finding metadata and public-main
  reachability for every archived-ref finding.
- `raw/`: focused validation logs and the exact post-N2 commit list.
- `SHA256SUMS`: hashes for every other file in this package.
