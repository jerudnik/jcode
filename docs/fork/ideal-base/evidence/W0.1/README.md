# W0.1 evidence: baseline revalidation and drift classification

Captured: 2026-07-18T06:13Z
HEAD at capture: `daecffa8f837b34f7acf27fc9db1bcf99bf172d4` (branch `main`)

Files:

- `transcript.txt`: raw command transcript (railway check, git identity,
  worktrees, protected hash, stashes, archive refs, runtime channels, live
  processes).
- `drift.md`: classification of every observed difference from `BASELINE.md`.

Result: railway valid, protected hash intact, one worktree, stashes and 87
archive refs preserved. Two classified drifts (expected railway commits on top
of the seed; a post-seed selfdev canary activation with a stale
pending_activation whose initiating session is dead). No unsafe mutation was
performed. No promotion, rollback, or reload was triggered.
