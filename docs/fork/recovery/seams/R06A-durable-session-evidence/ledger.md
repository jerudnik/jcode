# R06A Durable session evidence, journals, snapshots, and replay: lightweight ledger

| Field | Value |
|---|---|
| State | `adjudicated` |
| Baseline | fork `f5a8999d81311d237d1c106a9d980fd86fa34b6e`; upstream `802f6909825809e882d9c2d575b7e478dce57d3b`; merge base `631935dd1d3b2e31e167e2b12ad463e54bcf4b8d` |
| Review mode | `light` (pilot fixture prerequisite per `RESPONSIBILITIES.md`) |
| Research budget | 10 decisive checkpoints for the R06A/R07C/R13 batch; shared batch consumed 10 |
| Recommended disposition | `retain-fork` |
| Confidence | high for schema and round-trip behavior; medium for full replay-surface coverage (see gaps) |

R06A owns the durable evidence schema, history and snapshot persistence, deterministic replay, provenance fields, partial-write handling, and resume round trips. It explicitly excludes live turn emission: R12 owns when and with what content a `ProviderRequest`/`ProviderResponse` event is emitted during a turn (`crates/jcode-app-core/src/agent/turn_loops.rs:105,142,678` and `crates/jcode-app-core/src/agent/turn_streaming_mpsc.rs:224,282,913,969` call into the R06A writer but their ordering and correlation semantics are R12 authority). R06A owns only what happens once `SessionEvidenceWriter::append` is called: durable append, sequencing, and faithful read-back. The R00/R09/R11 overlays bind this ledger: fixed refs above, no gate baseline changes, append-only recovery truth.

## Findings

| Finding | Evidence | Consequence |
|---|---|---|
| The evidence subsystem is fork-only; upstream has no counterpart | `git ls-tree 802f6909825809e882d9c2d575b7e478dce57d3b -- crates/jcode-base/src/session/evidence.rs crates/jcode-session-types/src/evidence.rs` returns empty; upstream `crates/jcode-base/src/session/` contains `crash.rs`, `journal.rs`, `maintenance.rs`, `memory_profile.rs`, `model.rs`, `persistence.rs`, `render.rs`, and `storage_paths.rs` but no `evidence.rs` (`git ls-tree --name-only 802f69098 -- crates/jcode-base/src/session/`); fork adds `evidence.rs` (342 lines) plus `jcode-session-types/src/evidence.rs` (340 lines) per `git diff --stat 631935dd..HEAD` | Nothing upstream to adopt; disposition space is `retain-fork` or `delete`, and the pilot requires the evidence round trip, so `retain-fork` |
| Schema v1 is versioned and self-describing | `crates/jcode-session-types/src/evidence.rs:5` `SESSION_LOG_EVENT_SCHEMA_VERSION: u16 = 1`; `SessionLogEventKind` (lines 91+) covers `TurnStarted`, `TurnFinished`, `ProviderRequest`, `ProviderResponse`, `ToolStarted`, `ToolFinished`; every event carries `event_id`, `sequence`, `timestamp`, `session_id`, parent/child ids, `NodeSnapshot`, optional `GitSnapshot`, and `CorrelationIds` (`crates/jcode-base/src/session/evidence.rs:87-105`) | Provenance and correlation are structural, satisfying cross-seam invariant #4's "correlated request/result" requirement at the storage layer |
| Round trip is deterministic and tested | `bash scripts/dev_cargo.sh test -p jcode-session-types --lib` passed 11/11 on 2026-07-15, including `all_v1_event_kinds_round_trip` and `omitted_schema_version_defaults_to_v1` | Serialization of every v1 event kind survives serde round trip without loss |
| Partial writes fail safe, not silently | `read_session_evidence_from_path` (`crates/jcode-base/src/session/evidence.rs:115-144`) skips blank lines, stops at the first unparsable JSONL line with a logged warning, and sorts surviving events by `sequence`; `next_sequence_for_evidence_path` resumes sequencing from the existing file | A torn append truncates replay at the tear instead of fabricating completion; no duplicated or invented terminal record |
| Journal/snapshot resume round trip works in isolation | `JCODE_HOME=$(mktemp -d) bash scripts/dev_cargo.sh test -p jcode-base --lib -- journal_and_load_replays` and `-- session_exists_roundtrip` both passed 2026-07-15 (`session::tests::cases::test_save_appends_journal_and_load_replays_it`, `test_session_exists_roundtrip`); `provider_session_id` persists via `journal_meta`/`apply_journal_meta` (`crates/jcode-base/src/session.rs:519,705`) | Session history and metadata survive save/load; the journal path the pilot's resume-free run touches is exercised by existing tests |
| Storage is isolated by `JCODE_HOME` | `crates/jcode-storage/src/lib.rs:75-82` roots all session, journal, and evidence paths (`session/storage_paths.rs`) under `$JCODE_HOME` when set | The pilot fixture can run entirely inside a disposable directory; no writes escape it |

## Negative findings

- No evidence-file locking or cross-process append coordination was found in `evidence.rs`; concurrent writers to one evidence file would interleave sequences. The pilot uses one process and one session, so this is out of pilot scope but recorded.
- No dedicated unit test exists for `SessionEvidenceWriter` in `jcode-base` itself; the only in-tree consumer test is `crates/jcode-app-core/src/tool/session_search_tests.rs:321,358`, which exercises write-then-read indirectly. The pilot fixture below closes this gap for the pilot's exact path.
- No fabricated-completion path was found: nothing in `evidence.rs` synthesizes a `ProviderResponse` on read; a missing terminal event stays missing.

## Minimal pilot evidence fixture (required round trip)

The pilot's R06A exit check is this storage-layer round trip, run in a disposable `JCODE_HOME`:

1. Construct `SessionEvidenceContext::local(None, None)` and `SessionEvidenceWriter::for_path(session_id, <tmp>/evidence.jsonl, context)`.
2. Append exactly four events in order: `TurnStarted`, `ProviderRequest { provider, model, route, message_count, tool_count, prompt: None }`, `ProviderResponse { status: success, duration_ms, usage, .. }`, `TurnFinished { status: success, .. }`.
3. Read back with `read_session_evidence_from_path` and assert: four events, sequences `0..=3`, identical field content, exactly one `ProviderResponse` for the one `ProviderRequest`, and schema_version 1 on every event.
4. Truncation probe: append a fifth garbage line to the file and assert read-back still returns exactly the four valid events (partial-write behavior).

This is storage-side only. R12's ledger must separately prove the live turn emits these events in this order with correct content; R06A accepts what R12 emits and must not be blamed for emission-order defects.

## Reproduction

```bash
git ls-tree 802f6909825809e882d9c2d575b7e478dce57d3b -- \
  crates/jcode-base/src/session/evidence.rs crates/jcode-session-types/src/evidence.rs  # empty
git diff --stat 631935dd1d3b..HEAD -- crates/jcode-session-types crates/jcode-base/src/session
bash scripts/dev_cargo.sh test -p jcode-session-types --lib          # 11 passed
JCODE_HOME=$(mktemp -d) bash scripts/dev_cargo.sh test -p jcode-base --lib -- journal_and_load_replays
JCODE_HOME=$(mktemp -d) bash scripts/dev_cargo.sh test -p jcode-base --lib -- session_exists_roundtrip
```

## Explicit gaps

- The `b3ed82a6b` curated sync was not searched for an absorbed upstream evidence predecessor; upstream at `802f69098` has no evidence module, so absorption is unlikely but was not proven absent inside the squash.
- Full replay surfaces (`new_for_replay*` in `crates/jcode-tui/src/tui/app/tui_lifecycle_runtime.rs:6-13`) and crash recovery (`session/crash.rs`) were located but not behavior-tested here; they are outside the resume-free pilot.
- Snapshot-vector persistence modes (`PersistVectorMode`, `session/journal.rs:54`) were not exercised.

## Disposition and conditions

- Recommended disposition: `retain-fork`. The evidence schema, writer, and reader are fork-only with no upstream counterpart, versioned, and round-trip tested.
- Pilot entry check: `jcode-session-types` lib tests pass and `JCODE_HOME` isolation is confirmed. Pilot exit check: the minimal fixture round trip above passes, including the truncation probe, inside the disposable environment.
- Acceptance or retirement condition: this ledger is accepted for the pilot when the coordinator approves it and the fixture round trip passes; it retires into a full R06 review only if evidence storage becomes a sync target or the schema version changes.
- Escalate to full review if: the pilot fixture shows loss, duplication, or fabricated completion; `SESSION_LOG_EVENT_SCHEMA_VERSION` changes; R12's ledger finds the emitted event stream cannot satisfy invariant #4 through this storage; or multi-process evidence writes enter pilot scope.
- Confidence: high on the storage round trip; the residual risk is emission-side and belongs to R12.
- Coordinator approval: `pass`; fixed refs, source boundary, partial-write behavior, and the 11-test schema round trip were reproduced before integration.
- Independent Opus review: `pass` after bounded correction and re-review; see [`2026-07-15-pilot-prereq-ledgers-opus-review.md`](../../reviews/2026-07-15-pilot-prereq-ledgers-opus-review.md), SHA-256 `bb763b0924cd16196785e9129663531990e6364225a7d57467f0a834e4bf73b4`, and [`2026-07-15-pilot-prereq-ledgers-opus-rereview.md`](../../reviews/2026-07-15-pilot-prereq-ledgers-opus-rereview.md), SHA-256 `cf66e5ffa0efd12e89a61c3a505ee0cd9ba0cefaf80a4476b045517f913134ba`.
