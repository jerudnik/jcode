# S01-FIX-1 contract amendment: F03 client-connection post-release window

## What changed

`docs/fork/ideal-base/evidence/F03/lease_class_fixtures.sh` no longer asserts
the "alive 4s after release (full new idle window)" step for the
`client-connection` class. Every other class keeps the assertion unchanged.
The held-past-timeout assertion, the exit-44 assertion, and the residue check
still run for all eight classes, including `client-connection`.

## Why this is a fixture repair, not a green-wash

The fixture asserted a promise design 4.1 never made. The coverage matrix
(F01 design.md, section 4.1) specifies C1 `ClientConnection` as **abandon**
for every drain path: connections are closed by intake shutdown, not waited
for. The product implements exactly that:

- `shutdown.rs` `drain_blocking_count()` deliberately excludes
  `ActivityClass::ClientConnection`, with a comment citing design 4.1 C1.
- The idle pollers (`lifecycle.rs`) compute quiescence from
  `clients == 0 && drain_blocking_count() == 0`, so a held client-connection
  *lease* (with no counted client) is invisible to the idle epoch.
- Only the atomic idle claim (`shutdown.rs`, "table completely empty")
  sees the lease, which is why the daemon never exits while it is held.

Consequently the design's testable promise for this class is:

1. no exit while the lease is held (claim refuses), and
2. exit 44 after release, with zero residue.

It is NOT "a full idle window elapses after release". Whether a full
post-release window happens depends on whether a poll tick with
`elapsed >= timeout` lands during the hold: if one does, the claim's refusal
resets the epoch (PASS under the old assertion); if none does, the epoch
carries over and the daemon exits ~1 tick after release (FAIL under the old
assertion). Same binary, same commit, opposite verdicts, ~1-in-10.

## Evidence the mechanism is understood (S01 FINDINGS.md, S01-F6)

- 32-run contingency table: verdict is perfectly separated by whether
  `claim lost to new activity` appears in the daemon's own log
  (3 FAIL / refusals=0; 29 PASS / refusals>=1; no off-diagonal cell).
- Differential control at HOLD=24 (hold spans >= 2 poll ticks, so a
  refusal is guaranteed): 8/8 PASS, as the mechanism predicts.
- Reproducer preserved at `evidence/S01/repro-f03-cc.sh`.

## The alternative we rejected

Making the product enforce a full post-release window for client-connection
leases would mean adding ClientConnection to `drain_blocking_count`, which
reverts a deliberate design decision (C1 abandon; resolves the review's C1
duplication note in design 3.1) and changes shutdown behavior for real
clients. No user-visible defect motivates that. The race is between the
fixture's expectation and the design, not inside the product.

## Secondary repair in the same edit

The fixture `rm -rf`'d its runtime dir on exactly the failing branches,
destroying the daemon log that explains the verdict. Both failure branches
in section A now `tail -20 "$DIR/daemon.log"` before cleanup (S01-F6 noted
this as a second, smaller fixture defect).

## Residual risk

If the design ever changes client-connection to a draining class, this
fixture must be re-amended; the per-class `if` makes that a one-line change.
The epoch-reset behavior itself (I1: hold-past-timeout-then-release starts a
FULL new window) remains covered by the seven drain-blocking classes.
