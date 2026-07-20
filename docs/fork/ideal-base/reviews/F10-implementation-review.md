# F10 implementation review (adversarial, opus-class)

Reviewed commit: c27ec5a07. Verdict: **PASS** (both gates hold, 15/15
tests at review time), with important follow-ups, two of which were fixed
immediately after review (276764206, 4b66de27c).

## Important findings

1. Reconcile ignored record.reason and always marked Crashed, polluting
   attention lists (catchup tags failed/intervention) for clean idle
   disconnects. FIXED: reason-aware mapping, clean disconnects -> Closed.
2. Contract ruling on "retry after abort": startup reconcile alone does
   not satisfy the letter; a long-running daemon would strand the session
   phantom-live for days. FIXED: one bounded deferred in-process retry
   (30s) that persists terminal state or leaves the record.
3. Liveness closure folds to dead on pid-marker lock contention (shared
   F09 finding): narrow window where a just-recovered live session could
   be marked Crashed on disk. Moderate/rare; mitigations noted (record
   must exist + Active on disk + contention in the same window; marker
   survives via remove-if-unchanged). Follow-up: tri-state liveness.

## Minor findings

serde_json::to_vec unwrap_or_default writes an empty file on serialize
failure (log instead); cross-process record races are benign but
nondeterministic (Crashed vs Closed last-writer-wins); record overwrite
on reconnect loses the original abort's timestamp (cosmetic).

## Gates executed

Full diff read: record write inside sessions write lock strictly before
remove(), all removal branches covered, successor early-return correct,
only Persisted deletes. Tests via nix develop: 15 passed including both
gate tests. Ordering vs headless recovery verified (recovery awaited
before sweep spawns).

## Not checked

Live two-daemon restart; Session::load error taxonomy (record dropped on
any load error); jcode_dir None fallback; PidMarkerLock contention
probability.
