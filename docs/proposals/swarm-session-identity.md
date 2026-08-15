# Swarm session identity: name reuse and attachment ambiguity

Status: proposal, evidence gathered 2026-08-13 (test-remediation pipeline session)

## Incident

An operator opened a UI window onto a swarm worker named `duckling` (the WP1-4
test-remediation implementer, rooted in `/Users/jrudnik/labs/jcode`). The
window showed an in-flight edit to `ui_header.rs` — a file no remediation work
package touches. Checking the repo showed no uncommitted `ui_header.rs` change
and no worktree. Minutes later the same window spontaneously displayed a
different agent, `guppy`, working in a different project.

While this was being diagnosed, the coordinator queried
`swarm status session_duckling_...` and got `Swarm: /Users/jrudnik` — the home
directory, not the repo the worker was spawned in — plus `attachments=1`, and
repeated `Session ... is busy` errors from `swarm summary`. The coordinator
then issued `swarm stop` against that session id without being able to confirm
which agent's work it was ending.

## What went wrong (three related defects)

1. **Friendly names are reused.** The roster held two `duckling` sessions
   (`duckling [e474ba]` the implementer, `duckling [b0570fe]` an older audit
   worker) and multiple other duplicated names (`wyvern` x2, `retriever` x2,
   `tiger` x2...). `swarm list` disambiguates with an id-suffix chip, but other
   surfaces (window titles, name-based targeting) do not.
2. **Window attachment does not appear pinned to a session id.** The observed
   duckling→guppy switch means the window followed something unstable — a
   name, a slot index, or a most-recent-activity heuristic — rather than the
   immutable session id. A human watching a worker can silently end up
   watching a different one, in a different project.
3. **Human attachment changes what the coordinator sees.** With the window
   open, the session's reported swarm root changed to the home directory and
   the session became too busy to summarize. Operator-authored note (from
   `swarm-lifecycle-remediation.md`, now incorporated here): when a human opens
   a swarm member's session, the coordinator loses visibility into it, the
   session's directory appears changed, and it is unclear what actually kills
   the member. A human should be able to open and close a member session
   without killing it, while retaining a deliberate manual kill.

## Consequences observed

- Operator could not tell whether real work was being wasted ("helping,
  harming, or just wasting tokens").
- Coordinator stopped a session by ambiguous identity; if the displayed agent
  had been the real target's namesake, the stop would have hit the wrong one.
- Post-incident forensics required cross-referencing `git status`, file
  mtimes, and the full roster — none of which should be necessary to answer
  "which agent is this window showing?"

## Stepwise remediation plan

1. **Pin window attachment to the full session id.** A viewer opened on
   `session_duckling_1786584810759_...` must never render a different session,
   even if that session ends; show a terminal state instead of switching.
2. **Qualify names everywhere.** Render `duckling [e474ba]` (name + short id)
   in window titles, status lines, and reports — `swarm list` already does
   this for duplicate names; make it unconditional and universal.
3. **Make name-based targeting fail closed.** `stop`/`dm`/`status` with a
   friendly name that matches more than one live-or-recent session should
   error with the candidate list, not pick one. (The tool description already
   promises this for ambiguous names; verify it covers recently-terminated
   namesakes too.)
4. **Decouple observation from execution.** Human attach should be read-only
   by default: no working-directory change, no interruption of the member's
   turn, coordinator queries (`status`, `summary`) keep working. An explicit
   takeover action can exist, but it must be a deliberate step, and a separate
   deliberate kill.
5. **Report attachment state honestly.** `swarm status` should say
   `attached_by: <viewer>` and keep reporting the session's true working
   directory rather than the viewer's.
6. **Tests.** Attachment does not change `Swarm:` root in status output;
   summary remains answerable while attached; duplicate-name stop errors with
   candidates; a window bound to a session id renders that id's transcript
   after a namesake spawns.

## References

- Incident session roster showing duplicate names and the
  `Swarm: /Users/jrudnik` anomaly (2026-08-13, test-remediation session).
- `docs/proposals/swarm-lifecycle-remediation.md` — sibling lifecycle-trust
  defects; its inline human note is incorporated above.
- `docs/proposals/provider-confusion.md` — same session, same theme: identity
  the operator believes is not the identity the system uses.
