# Independent review: R06A / R07C / R13 pilot-prerequisite lightweight ledgers

- Reviewer: verify agent (adversarial, read-only)
- Target commit: `7aa3683d4e2bd219328362df53819091d61bbec0` in `/Users/jrudnik/labs/jcode-pilot-prereq-ledgers`
- Source: read-only; no repo, ref, stash, worktree, or baseline mutated. No network, credentials, or live daemon touched.
- Checkpoint budget: 10 decisive checkpoints, all consumed on verification (refs, schema, partial-write, kill switch, consent paths, constants, avoidance, census assignment scan, and four test runs). Did not expand.
- Ledger path prefix note: actual files live under `docs/fork/recovery/seams/`, not `docs/fork/seams/` as the task text implied. Verified via `git show --name-only 7aa3683d4`.

## Shared preconditions (PASS)

- Fixed refs resolve and are internally consistent: `git merge-base f5a8999d8 802f69098` = `631935dd1d3b...` (matches all three ledgers). HEAD parent (`git rev-parse HEAD^`) = `f5a8999d81311d237d1c106a9d980fd86fa34b6e`, the declared fork baseline, so each ledger was authored one commit atop its own stated ref. Consistent.
- R00/R09/R11 overlay obligations are honored by all three: every ledger states fork/upstream/merge-base SHAs, records exact reproduction commands, recommends `retain-fork`, and adds no `--update` or gate-script edits. Append-only posture intact (commit adds 212 lines across three new files, edits nothing).
- Overlay coordinator-approval status: R00/R09/R11 show `coordinator approval: pass`; all three seam ledgers show `coordinator approval: pending` and `Fable review: pending`. Consistent with their `adjudicated` state and pre-pilot posture. Not a defect, but they are not yet accepted.

---

## R06A durable session evidence: PASS with one IMPORTANT accuracy defect

Confidence: high.

Verified decisive evidence:
- Fork-only subsystem confirmed. `git ls-tree 802f69098 -- crates/jcode-base/src/session/evidence.rs crates/jcode-session-types/src/evidence.rs` returns empty; both files exist at `f5a8999d8`. Disposition space is correctly `retain-fork`/`delete`. PASS.
- Line counts exact: `evidence.rs` 342 lines, `jcode-session-types/src/evidence.rs` 340 lines. Matches ledger. PASS.
- Schema is versioned: `SESSION_LOG_EVENT_SCHEMA_VERSION: u16 = 1` at line 5; `#[serde(default = ...)]` on `schema_version`; all six `SessionLogEventKind` variants present with `event_id`/`sequence`/`timestamp`/`session_id`/parent+child ids/`node`/`git`/`correlation`. Matches cross-seam invariant #4 structural claim. PASS.
- Partial-write semantics confirmed by direct read of `read_session_evidence_from_path` (lines 115-144): blank lines skipped, first unparsable JSONL line logs a warning and `break`s (stops, does not fabricate), survivors `sort_by_key(sequence)`. `append` (lines 94-105) writes exactly the claimed fields via `append_json_line_fast`. No terminal-record fabrication path exists. PASS.
- Fixture API names all exist and match signatures: `SessionEvidenceContext::local(working_dir, git)` (line 25), `SessionEvidenceWriter::for_path(...)` (line 68), `next_sequence_for_evidence_path` (line 165). Fixture is buildable as described. PASS.
- Tests reproduced: `bash scripts/dev_cargo.sh test -p jcode-session-types --lib` -> 11 passed / 0 failed in a disposable `JCODE_HOME`, including `all_v1_event_kinds_round_trip` and `omitted_schema_version_defaults_to_v1`. PASS.

IMPORTANT (supporting-evidence overclaim, not decisive-claim failure):
- The ledger states upstream `crates/jcode-base/src/session/` "contains only `journal.rs` and `persistence.rs`." This is false. `git ls-tree 802f69098 -- crates/jcode-base/src/session/` returns eight files: `crash.rs`, `journal.rs`, `maintenance.rs`, `memory_profile.rs`, `model.rs`, `persistence.rs`, `render.rs`, `storage_paths.rs`. The decisive claim (no `evidence.rs` upstream) still holds and is independently verified, so the `retain-fork` disposition is unaffected, but the enumerated file list is wrong and should be corrected before sign-off. This is exactly the kind of factual assertion the R00 provenance overlay requires to be accurate.

Negative findings independently confirmed: no evidence-file locking (concurrent interleave possible, correctly scoped out of a one-process pilot); no dedicated `jcode-base` unit test for `SessionEvidenceWriter` (fixture closes this); no read-side completion synthesis.

Verdict: integration-ready for the pilot once the upstream-session-dir file list is corrected. The correction does not change the disposition.

---

## R07C telemetry consent: PASS

Confidence: high.

Verified decisive evidence:
- Global kill switch confirmed by direct read of `is_enabled()` (lib.rs:284-296): returns false if `JCODE_NO_TELEMETRY` or `DO_NOT_TRACK` is set, or if `$JCODE_HOME/no_telemetry` exists via `storage::jcode_dir()`. PASS.
- All four consent paths confirmed in source:
  1. env `JCODE_NO_TELEMETRY` (opt-out, line 285),
  2. env `DO_NOT_TRACK` (opt-out, line 285),
  3. marker `$JCODE_HOME/no_telemetry` (opt-out, lines 288-293),
  4. marker `$JCODE_HOME/telemetry_share_content` (opt-in for content sharing) via `content_sharing_enabled()` (lines 311-322) and `set_content_sharing_enabled()` (line 325). `content_sharing_enabled` requires `is_enabled()` AND re-checks the opt-out env vars AND the marker, so a fresh `JCODE_HOME` yields false. Off-by-default confirmed. PASS.
- Temp-`JCODE_HOME` telemetry tests reproduced: `bash scripts/dev_cargo.sh test -p jcode-telemetry-core --lib -- --test-threads=1` in a fresh empty `JCODE_HOME` -> 17 passed / 0 failed, including `test_opt_out_env_var`, `test_do_not_track`, `test_discovery_event_serialization_excludes_free_text`, `test_sanitize_telemetry_label_strips_ansi_and_controls`. PASS.
- Kill-switch negative finding independently reproduced: with `$JCODE_HOME/no_telemetry` present, `test_error_counters` and `test_error_counters_no_session_is_noop` FAIL (session-begin path early-returns when disabled). This both confirms the ledger's documented test-isolation hazard and positively demonstrates the kill switch suppresses the session/reporting path. PASS. Correctly classified in the ledger as a test-isolation defect, not a consent defect.

Negative findings independently confirmed: every `pub fn` reaching `send_payload` re-checks `is_enabled()`; no send path found in `crates/jcode-base/src/session/` (no R06A -> R07C leak); free-text limited to explicit `/feedback` with `sanitize_feedback_text`.

Verdict: integration-ready. Consent posture for the fixture (disposable `JCODE_HOME` + `JCODE_NO_TELEMETRY=1`, no content marker) is provably reporting-disabled. Gaps (server-side worker, upstream 134-line delta) are correctly scoped out since sends are disabled.

---

## R13 compaction and context budget: PASS with one IMPORTANT census-completeness defect

Confidence: high on avoidance and thresholds; medium on census completeness (see below).

Verified decisive evidence:
- Fork-dominant, upstream-empty confirmed at the file level (compaction files are fork-only post-base). `retain-fork` sound. PASS.
- Thresholds exact: `DEFAULT_TOKEN_BUDGET = 200_000`, `COMPACTION_THRESHOLD = 0.80`, `CRITICAL_THRESHOLD = 0.95`, `MANUAL_COMPACT_MIN_THRESHOLD = 0.10`, `RECENT_TURNS_TO_KEEP = 10` (lib.rs:6-19). PASS.
- One-turn avoidance arithmetic confirmed by direct read of `should_compact_with` (compaction.rs:857-873): every mode gates on `active.len() > RECENT_TURNS_TO_KEEP` (10). A one-turn pilot has `active.len() <= 1 < 10`, so no threshold math runs; automatic reactive additionally needs usage `>= 0.80` (~160k tokens); manual needs an explicit command; 413 needs a provider payload error. All four triggers are unreachable for one small no-tool turn. The avoidance proof is arithmetic and sound. PASS.
- Estimation tests reproduced: `jcode-compaction-core --lib` -> 18 passed / 0 failed, including `image_token_cost_is_bounded_not_base64_length` (flat `IMAGE_TOKEN_COST = 1_600`, lib.rs:53-57), so image payload size cannot spuriously trip the threshold. PASS.
- Joint invalidation test reproduced: `jcode-app-core --lib -- messages_for_provider_applies_manual_compaction_in_native_auto_mode` -> 1 passed. Read of `agent_tests.rs:426-476` confirms it seeds both `agent.provider_session_id` and `agent.session.provider_session_id` to a stale value, runs manual compaction, then asserts BOTH `is_none()` (lines 464-465). This is the exact defense-in-depth check invariant #3 requires. PASS.
- Compaction reset sites clear both copies: verified `compaction.rs` lines 7-8, 163-164, 224-225, 268-269 and `agent.rs:687-688` each set `self.provider_session_id = None; self.session.provider_session_id = None;` as a pair. PASS.

Workspace-wide non-test writer/reset census (independent reproduction):
- I ran `git grep -n "provider_session_id\s*=" f5a8999d8 -- '*.rs'` filtered to non-test assignment sites and cross-checked every entry against the ledger's tables.
- Every writer and reset the ledger lists is present and correctly classified. Restore-writer sites (`tui_lifecycle_runtime.rs:629,648`, `turn_execution.rs:250-251,599`, `session.rs:351,387,711`, `commands.rs:1998-1999`) and the R12 single-copy `turn.rs:724` are all confirmed exactly as described.
- `crash.rs:743` (`session.provider_session_id = Some(...)`) is correctly excluded: it sits below `#[cfg(test)]` at line 660 (a test fixture), so it is not a production writer. The ledger's `test`-exclusion methodology handled it correctly.

IMPORTANT (census completeness / falsified enumeration claim):
- The census omits a real non-test single-copy reset: `crates/jcode-tui/src/tui/app/conversation_state.rs:835` `self.provider_session_id = None;` inside `pub(super) fn recover_session_without_tools` (defined at line 803, no enclosing `#[cfg(test)]`). It resets only the agent copy, not `self.session.provider_session_id`.
- The ledger explicitly asserts: "The single-copy sites are `turn.rs:724` (writer, R12) and `turn_execution.rs:189` (`clear`...)." That enumeration is incomplete. `conversation_state.rs:835` is a third single-copy site (a reset), and it is precisely the class of site the invariant-#3 census exists to surface. Its omission falsifies the "full census" / "workspace-wide symbol search" completeness claim.
- Adversarial divergence assessment: this site is benign in practice because line 836 immediately does `self.session = new_session;`, replacing the whole session (and its `provider_session_id`) wholesale, so no agent/persisted divergence survives the function. It belongs with R13's "reset followed by session replacement" class (analogous to the ledger's own treatment of `turn_execution.rs:189`). So this is a completeness/accuracy defect, not a live divergence bug, hence IMPORTANT rather than CRITICAL. It should be added to the census and classified before the census is treated as authoritative for invariant #3, and the "single-copy sites are only X and Y" sentence must be corrected.

Verdict: integration-ready for the one-turn pilot on the strength of the arithmetic avoidance proof (which does not depend on census completeness). The census, however, must add and classify `conversation_state.rs:835` before it is relied on as the complete invariant-#3 enumeration.

---

## Integration-ready verdict

| Ledger | Verdict | Blocking? |
|---|---|---|
| R06A | PASS with 1 IMPORTANT (upstream `session/` file list overclaimed as two files; actually eight) | Not blocking the pilot; correct the file list before sign-off |
| R07C | PASS, no corrections required | Ready |
| R13 | PASS with 1 IMPORTANT (census omits non-test single-copy reset `conversation_state.rs:835`; "only two single-copy sites" claim is false) | Not blocking the one-turn pilot; correct the census before treating it as the complete invariant-#3 enumeration |

No CRITICAL findings. All three `retain-fork` dispositions are sound and independently supported. All cited tests exist and pass in disposable `JCODE_HOME`s; the R07C kill switch is positively demonstrated by both passing (fresh home) and failing (marker present) runs. R00/R09/R11 overlays are respected. The two IMPORTANT items are accuracy defects in supporting evidence and enumeration, not failures of any decisive claim or of the pilot entry/exit checks; the pilot may proceed while they are corrected as append-only amendments per the R11 overlay.

Residual risk correctly deferred: R06A emission-order defects belong to R12; R13 413/emergency-truncation behavior is unreachable in the pilot; R07C server-side and upstream deltas are moot when sends are disabled. Escalation triggers in each ledger are appropriate and specific.
