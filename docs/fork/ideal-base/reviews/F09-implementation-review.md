# F09 implementation review (adversarial, opus-class)

Reviewed commit: d5d388028. Verdict: **PASS**. All three acceptance gates
verified by execution (58 build-support tests green incl. 8 reconcile
fixtures); ordering and guard logic correct.

## Important findings (non-blocking follow-ups)

1. Cross-process canary steal window: the canary guard checks a manifest
   snapshot, but complete re-loads and overwrites unconditionally; a
   start_canary between check and save loses. Same class: Skipped saves a
   stale snapshot wholesale (lost update). Requires the already-planned
   manifest file lock; single-daemon deployments (the norm) are unaffected.
2. Liveness folds to false on pid-marker lock contention
   (observe_session_pid_markers returns default when acquire_bounded
   fails), which could make a live initiator look dead at a busy startup.
   Prefer tri-state (unknown => alive). Mitigated by the 10-minute min_age
   and the canary guard.
3. Identity check verifies sidecar metadata, not binary content: an
   empty/truncated binary with an intact sidecar passes. Follow-up: file
   size/nonzero check or content digest in the sidecar.

## Minor findings

Partial strip can leave current/shared-server on divergent versions (by
design, undocumented); rollback sets canary_status=Failed unconditionally
(can misattribute a dead foreign canary); new_version used as a path
component unvalidated (trusted writer); README says 7 new tests, there are
8; clock skew handled correctly (future requested_at => StillFresh).

## Gates executed

scripts/dev_cargo.sh test -p jcode-build-support: 58 passed. All 8
reconcile fixtures read and mapped to gates. Canary guard verified to run
BEFORE complete; rollback newer-publish guard direction verified; startup
hook verified panic-safe and sequenced after reload handoff (min_age
covers in-flight reloads).

## Not checked

Live daemon boot of the hook; empirical two-process races; atomic symlink
swap crash-consistency; wrapper/payload sidecar resolution on the
reconcile path.
