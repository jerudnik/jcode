# W6 synthesis: human-noticed product defects repaired

W6 exists because operating jcode surfaced user-visible TUI/CLI faults that were
deliberately not fixed in-program, to avoid widening scope during signoff. Each
was root-caused into `human-noticed-issues/` and deferred to this wave.

Eleven children, all complete: ten accepted, one superseded.

| Node | What closed | Evidence |
|---|---|---|
| R08 | Todo-subsystem terminal-state handling: rejected updates now name the offending group and distinguish missing-goal from missing-score from below-threshold | human-noticed-issues/TODO_POKE_TERMINAL_STATES.md |
| R09 | Remote-session stall: routes memo TTL was shorter than the build it cached, and the client watchdog called a slow session gone | evidence/R09_routes_memo_ttl.md |
| R10 | Reload staleness: a same-path in-place republish compared candidate mtime against process start, which `Command::exec` preserves | human-noticed-issues/RELOAD_STALENESS_PROCESS_START.md |
| F30-FIX-1 | Distribution-policy scan inverted from an 8-file opt-in to opt-out over all tracked active docs | evidence/W6/F30-FIX-1.md |
| F30-FIX-2 | Seven boundary-anchored regexes for the AUR, curl-pipe, wget-pipe and PowerShell-pipe install idioms the substring list missed | evidence/W6/F30-FIX-2.md |
| F30-FIX-3 | Workflow lint-list completeness, with two of three named workflows dispositioned as stale rather than fixed | evidence/W6/F30-FIX-3.md |
| F30-FIX-4 | Orphaned installer residue retired; two of three named items dispositioned as stale with reasons | evidence/W6/F30-FIX-4.md |
| R05-FIX-1 | Stall-guard cancel cause propagated, so a cancellation is no longer mislabeled as a server reload | evidence/W6/R05-FIX-1.md |
| F26-FIX-1 | **Superseded.** The premise was falsified by measurement: production unregister already exists | evidence/W6/F26-FIX-1.md |
| D01-FIX-1 | Safety action classifier wired into tool dispatch, so the documented tier gate actually runs for ambient sessions | evidence/D01-FIX-1/TIER_GATE_WIRE.md |
| D01-FIX-2 | Two dangling ambient wires connected; the third was measured as not a wire at all and referred back | evidence/D01-FIX-2/README.md |

## Why the wave is closable

The W6 purpose was repair of defects a human noticed while using the product, so
the honest test is behavioral rather than textual: does the fault still reproduce?

Every accepted node carries a control that **fails when its fix is removed**, and
where a node listed several fixes, the controls fail on **different assertions**
so that no single fix masks another. D01-FIX-2 is the clearest case: wire 1 is
proven by a record-count equality and wire 3 by a pause boolean, and both were
re-run after a module extraction so the evidence describes shipped code rather
than the code as it stood when the control was written.

## F26-FIX-1: superseded, not fixed

F26-FIX-1 is the one child that is not accepted, and the reason is the wave's
most useful finding. The node asserted that a session path never unregisters its
active-PID marker, so a leaked marker lingers up to a 24h liveness window.

Measurement falsified the premise rather than confirming it:

- Production unregister exists (`Session::mark_closed_and_persist`), with seven
  callers including five headless ones in the ambient runner.
- The sweep reclaims by **PID liveness**, not by a 24h window. Grepping for
  `86400`, `24 * 60` and `hours(24)` returns nothing.
- An empirical sweep of the live machine found 27 active-PID markers and 5
  streaming-PID markers, all 32 live, 0 dead. If markers leaked as the node
  describes, a machine that has run thousands of sessions would show dead-PID
  residue. It shows none.

The claims were re-derived for this closure rather than inherited, which mattered:
**every path in the original evidence was stale**, because the code moved crates
(`jcode-base` split into `jcode-app-core` and `jcode-storage`). All three claims
still hold at the new locations. The re-verification is appended to
`evidence/W6/F26-FIX-1.md` rather than overwriting the original, since the
original numbers were true when they were written.

`superseded` rather than `rejected` is the disposition because the node describes
work that no longer needs doing, not work that was refused. This distinction is
load-bearing: `rejected` is not in `DEPENDENCY_COMPLETE`, so a rejected child can
never satisfy a dependency and would have deadlocked the entire signoff tail
(W6 cannot synthesize, so W7 never opens, so D01-FIX-3/4 stay pending, so
S01/S02/S03 stay blocked). That deadlock was found by simulating the state change
before writing it, and the re-disposition was authorized by the owner rather than
self-authorized.

## Non-vacuity

Two nodes were **partially** implemented (F30-FIX-3, F30-FIX-4) and say so. In
both cases the majority of the node's named targets turned out to be stale, and
the disposition, with reasons, is the substance of the node rather than a
shortfall against it. Recording "2 of 3 items were not defects" is the outcome a
rubber-stamp cannot produce.

One node (D01-FIX-2) had its scope **narrowed** mid-flight, on a measurement: the
third listed wire is not a connection at all, since no `StreamEvent` carries
headers and `RateLimitInfo` has zero non-test constructors, leaving 42 provider
impls between the HTTP response and the scheduler. It was referred back as
D01-FIX-3 rather than quietly built.
