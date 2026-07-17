# Current fork normalization status

Recorded: 2026-07-17

This is the current operating checkpoint for the recovery-to-normalization
program. It supersedes pre-promotion starting facts in `BASELINE.md` and
`COORDINATOR_BRIEF.md` where they conflict, without rewriting those historical
records.

## Current source and runtime

- Canonical checkout: `/Users/jrudnik/labs/jcode`
- Canonical branch: `main`
- Promoted source/documentation checkpoint before this amendment:
  `42aa9cc64183741efb000a6d58c2c920de77e146`
- Fork remote `main` before this amendment: the same `42aa9cc64` checkpoint
- Product/runtime commit: `8962bccb32eede3b6746c42bfe6d265df29e4471`
- Runtime label: `8962bccb3-release`
- Runtime SHA-256:
  `6cf81221e8c0cee86ae714d2f1fc9fb55fe8715f45ee8082dc2ecf034a2515fc`
- Runtime channels: `current`, `stable`, and `shared-server` all select the exact
  immutable release
- Preserved user state: modified `docs/fork/recovery/ORCHESTRATOR_PROMPT.md`
  with diff SHA-256 `8e8e6a92dad180b3925bc0b2a3b7b951bc6a6f5c9e4f8a57c9f522d03ad85c00`,
  plus untracked `opencode.json`

The signed recovery source and sign-off commits are preserved on the recovery
line and in rollback archives. They are not ancestors of curated `main`; commit
`c786be6c3` imported the forensic record into the curated history.

## Milestone and gate disposition

| Surface                                 | Status                           | Evidence and boundary                                                                                                              |
| --------------------------------------- | -------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------- |
| N0 safety inventory                     | Complete                         | Verified all-ref and stash bundles plus committed N0 evidence                                                                      |
| N1 curated integration                  | Complete                         | Curated tree and recovery-equivalence evidence preserved                                                                           |
| N2 W7 and promotion                     | Complete with post-signoff delta | Original signoff at `62b3946b6`; final operational fixes at `1c368592f` and `8962bccb3` are explicitly recorded in `N2_SIGNOFF.md` |
| Runtime promotion                       | Complete                         | Exact release, channels, daemon identity, subscribed ping, and no-op reload verified                                               |
| N3 documentation and task normalization | In progress                      | Current status and archival boundaries are now explicit; issue #15 owns post-soak cleanup                                          |
| N4 full isolated runtime matrix         | Open                             | The committed live package proves bounded runtime promotion, not every D6 tool/MCP/swarm/provider scenario                         |
| N5 host normalization                   | Soak-gated                       | Thirty worktrees and local rollback assets remain retained through 2026-07-24                                                      |
| N6/D9 final sign-off                    | Open                             | No claim that two fresh reviewers signed every D0-D9 gate at one final commit and host manifest                                    |

The honest label is **promoted runtime validated, normalization incomplete**.
Neither “fully runtime-validated” nor unqualified D0-D9 completion is claimed.

## Private GitHub branch archive

GitHub stores Git objects and refs, not linked worktree directories. To make the
later local cleanup safer, all 41 local branch tips and the detached
`/private/tmp/jcode-up` worktree tip were copied exactly to the private repository:

<https://github.com/jerudnik/jcode-recovery-archive>

The archive is private. A branch-history gitleaks scan reported 19 test/example
patterns, all already reachable from public fork `main`; no newly unpublished
finding was introduced by the archive. Exact local and remote manifests are in
[`evidence/2026-07-17-post-promotion-checkpoint/`](evidence/2026-07-17-post-promotion-checkpoint/).

This remote copy does not authorize deletion. Worktree directories, caches,
ignored/untracked files, stash objects, and rollback bundle payloads remain local
through the soak. Stashes and bundles require a separate privacy review before
any cloud upload because they may contain user-owned content not represented by
normal branch history.

## Next authorized checkpoint

Issue [jerudnik/jcode#15](https://github.com/jerudnik/jcode/issues/15) is the
earliest cleanup gate on 2026-07-24. After fresh runtime and archive verification,
produce an exact deletion manifest, obtain approval for that manifest, remove
obsolete worktrees rather than relocating them, and move only still-active
worktrees with `git worktree move`.
