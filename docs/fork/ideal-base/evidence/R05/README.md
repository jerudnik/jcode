# R05 incident: multi-client session contention

Recorded: 2026-07-20. Incident time: ~11:43 local (15:43 UTC).

## Summary

Two TUI clients were attached to session fish simultaneously: a stale
client from 01:25 (conn_1784527628584, pid 68137, left in a background
terminal window) and the user's active client from 10:06
(conn_1784556373101, pid 28750). During a long streaming response, the
stale client's stream-stall guard fired repeated cancels against the
shared session. Each cancel interrupted the live turn; the stranded
soft-interrupt recovery machinery ("Preserving N pending soft
interrupt(s)" / "Recovering N stranded soft interrupt(s) into queued
follow-ups") re-queued the user's typed message, and 18 duplicate
deliveries of the same user message accumulated and replayed. The
duplicate deliveries were surfaced to the model as 18 separate user
messages within ~40 seconds. The interrupt was additionally mislabeled
"Tool 'subagent' interrupted by server reload" in an earlier occurrence
of the same class (the actual trigger recorded in the log is
trigger=stall_guard).

The daemon did NOT reload; cliff (pid 95514) ran throughout.

## Root causes

1. No duplicate-attach policy: nothing prevents or surfaces two live
   clients on one session; their independent stall guards and cancels
   interleave destructively (log: alternating cancel/message requests
   from both connection ids at 11:43).
2. Queued-message replay: identical queued user messages are delivered
   once per copy instead of being collapsed at the turn boundary.
3. Interrupt mislabeling: stall-guard cancels are reported as "server
   reload" interruptions.

## Immediate remediation

Coordinator killed the stale clients (68137, plus an older duplicate of
the mouse session, 12768), restoring one client per session.

## Evidence

incident-log-excerpt.txt: STALLED/UNMATCHED/Recovering lines showing
both connection ids issuing interleaved requests against the session.
