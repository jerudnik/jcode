# W0.1 drift classification against BASELINE.md

Baseline facts were recorded at railway creation (seed source commit
`923c6353e04266f71dc6cc06fc8516e502a9c07f`). Observations captured
2026-07-18T06:13Z at HEAD `daecffa8f`.

## Matches (no drift)

| Baseline fact | Observation | Status |
| --- | --- | --- |
| Canonical checkout `/Users/jrudnik/labs/jcode`, branch `main` | Confirmed | match |
| One registered worktree | `git worktree list --porcelain` shows exactly one | match |
| Protected prompt SHA-256 `ca3f1998...eed5b6` | Exact match | match |
| Recovery refs, four stashes, bundles, archives preserved | 4 stashes, 87 `refs/archive` refs present | match |
| Seed commit reachable from HEAD | `git merge-base --is-ancestor` confirms | match |
| `stable` = `8962bccb3-release` | `stable-version` file and manifest agree | match |
| No push/publication performed | HEAD is 4 ahead of `origin/main`, nothing pushed | match |

## Drift 1: source head advanced past the seed (expected, benign)

Baseline says the seed source was clean at `923c6353e` and two commits ahead of
`origin/main`. Current HEAD is `daecffa8f`, four commits ahead. The two extra
commits are the railway itself:

- `a1dc5c7aa` docs(fork): add ideal-base execution railway
- `daecffa8f` fix(fork): keep coordinator bootstrap copyable

Classification: expected consequence of creating this program. The authority
commit for the graph is `a1dc5c7aa` (derived via
`git log -1 --format=%H -- docs/fork/ideal-base/WORK_GRAPH.json`). No action.

## Drift 2: runtime channels and selfdev pending state changed after seeding

Baseline: `current`, `stable`, `shared-server` all selected
`8962bccb3-release`; no canary or pending activation.

Observed:

- `current-version` = `923c6353e-dirty-5a0f07fa7495`
- `shared-server-version` = `923c6353e-dirty-5a0f07fa7495`
- `stable-version` = `8962bccb3-release` (unchanged)
- manifest `canary` = `923c6353e-dirty-5a0f07fa7495`, `canary_status` = `testing`
- manifest `pending_activation` present, requested 2026-07-18T05:45:12Z by
  `session_peacock_1784221108198_12fe3e2e04160f62`; that session is `Closed`
  and has no active PID marker.
- The live shared server (PID 63766, started Jul 17 15:42) runs from
  `builds/shared-server/jcode`; 27 session PID markers point at it and are live.

Classification: a selfdev build/canary activation of the seed-source build
(`923c6353e-dirty`) occurred between railway creation and this session,
initiated by a now-dead session, leaving a stale `pending_activation` whose
`new_version` equals `previous_current_version`. This is exactly the failure
class targeted by node F09 (stale selfdev pending-activation reconciliation).
It is preserved as live reproduction evidence. Per instruction, nothing was
promoted, rolled back, or reloaded.

Action: recorded here and in `DECISIONS.md` (D006). F09 must account for this
concrete manifest shape (pending activation with dead initiating session and
`new_version == previous_current_version`).

## Drift 3: none observed for protected assets

No evidence, review, or seam ledger under `docs/fork/recovery/` or
`docs/fork/normalization/` was modified. Verified via `git status --short`
(only the new `evidence/W0.1/` directory is untracked).

## Amendment (2026-07-18T06:15Z, user-confirmed cause for Drift 2)

The user confirmed that a selfdev `switch` (activation) was performed
immediately before this coordinator session started. That explains the
canary/current/shared-server channel change and the `pending_activation`
record requested at 2026-07-18T05:45:12Z. The initiating session
(`session_peacock_...12fe3e2e04160f62`) is now `Closed`, so the record is a
genuine live instance of the stale-pending-activation class F09 targets:
the activation itself proceeded (channels point at the new build, the live
shared server was started 2026-07-17 15:42 and predates it), but the
`pending_activation` marker was never cleared. Disposition unchanged:
preserved untouched per D006 as an F09 fixture.

## Correction (2026-07-18T06:19Z)

`jcode debug server:info` reports the live shared server is
`v0.46.0-dev (923c6353e, dirty)` with uptime ~2000s at 06:18Z, i.e. it took the
activated canary via in-place reload around 05:45Z (the `ps lstart` value of
2026-07-17 15:42 reflects the original process spawn; reload exec preserves the
PID). The earlier sentence claiming the running server "predates" the
activation is therefore wrong about the running image: the activation itself
completed successfully end to end. Only the `pending_activation` marker was
left uncleared, which remains the F09 fixture.
