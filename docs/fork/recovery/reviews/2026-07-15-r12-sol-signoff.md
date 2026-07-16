# Sol sign-off: R12 agent turn evidence ledger

- **Verdict:** PASS as an authoritative R12 adjudication ledger.
- **Pilot verdict:** BLOCKED today. The ledger correctly forbids any unqualified R12 pilot and permits only a future narrow no-tool, no-cancel, no-compaction fixture after the required end-to-end evidence test exists and passes.
- **Exact commit reviewed:** `99e153edf131f42668a0e51361904053108a8357`
- **Repository:** `/Users/jrudnik/labs/jcode-seam-r12`
- **Sign-off mode:** independent Sol read-only verification. I did not read any future Fable sign-off. No live daemon, credentials, network fetch, destructive action, publication, source edit, ref edit, stash edit, or worktree content mutation was used. The final worktree status was clean.

## Findings by severity

### Critical

None found against the committed R12 ledger. I found no unpreserved review rewrite, no source-scope creep in commit `99e153edf`, and no ledger overclaim that the current pilot may run.

### High

1. **Current R12 pilot remains blocked, and the ledger says so.**  
   Evidence: R12 ledger lines 10-13 set fork authority but `Pilot entry verdict = blocked today`; lines 92-97 state the strict fixture cannot establish global correctness and the unqualified pilot is blocked; lines 153-155 adjudicate strict fixture/current pilot and cancellation/compaction/raw-transport variants as blocked; lines 278-280 retain the remaining risks.

2. **The exact terminal-cardinality matrix is accurately adjudicated.**  
   Evidence: source inspection confirms the ledger’s matrix. Blocking and MPSC happy paths emit one `ProviderResponse{Ok}` after one request (`turn_loops.rs:103-114,677-697`; `turn_streaming_mpsc.rs:222-233,968-988`). Open errors emit one error response (`turn_loops.rs:127-153`; `turn_streaming_mpsc.rs:253-293`). Raw stream transport `Err(e)` returns without a provider response in both engines (`turn_loops.rs:201-243`; `turn_streaming_mpsc.rs:410-460`). Context-limit retry continues after an emitted request without terminally representing the abandoned attempt (`turn_loops.rs:127-140,642-650`; `turn_streaming_mpsc.rs:253-280,857-895,937`). MPSC cancel-before-open returns `Ok(())` after request with no response (`turn_streaming_mpsc.rs:222-252`), and mid-stream cancel falls through to an `Ok` response (`turn_streaming_mpsc.rs:370-385,968-988`) while `status_for_result` only maps `Ok` and generic error (`evidence.rs:181-186`).

3. **Strict post-fixture entry boundary is preserved.**  
   Evidence: R12 ledger lines 117-129 require an R06A readback fixture with exactly four schema-v1 events in sequence, shared turn ID, one request/response correlation ID, safe truncation, and no interleaving compaction/route-selection event. Lines 163-184 restrict entry/exit to disposable home, telemetry disabled, no tools/MCP/memory/manual compaction/cancellation, and exact four-event readback. Lines 215-224 split future test/fix/refactor/docs slices and do not authorize a source change in this documentation commit.

### Medium

4. **Review preservation and hash integrity pass.**  
   Evidence: committed review hashes reproduce exactly: Opus `d3c19a9576f21e008b831594c13f09189527a98a20050d044e8d7e908e462a60`; Grok `c12b96cbd935010405a05cd57a6caba7c56a5a0aca904c302ccc2cf6f52555d8`. External `/tmp` artifacts were present and `cmp -s` reported both repository copies byte-identical. Line counts were 165 and 208 respectively, matching the ledger.

5. **Fixed-ref evidence and fork-only authority pass.**  
   Evidence: `git merge-base 7ff4fc6be8dcf0410f2f61994752fdf5ee93e6e4 802f6909825809e882d9c2d575b7e478dce57d3b` returned `631935dd1d3b2e31e167e2b12ad463e54bcf4b8d`; `git merge-base --is-ancestor 7ff4fc6be8dcf0410f2f61994752fdf5ee93e6e4 16921ace18cf5c25368a376357b7636478d3928f` succeeded; the reviewed R12 source paths had no fork-to-review-head diff; `git cat-file -e 802f6909825809e882d9c2d575b7e478dce57d3b:crates/jcode-app-core/src/agent/evidence.rs` failed, confirming upstream absence.

6. **R02/R06A/R07C/R13 conditions are carried forward rather than absorbed by R12.**  
   Evidence: R02 owns route/model/provider selection and persists `session.provider_key`, `session.route_api_method`, and `session.model` (`agent/provider.rs:71-86`; R02 ledger lines 24-29, 89-117). R06A owns append/readback only, not emission timing (`R06A ledger lines 12-13`; R12 ledger lines 117-129). R07C requires a fresh disposable `JCODE_HOME` and `JCODE_NO_TELEMETRY=1` (`R07C ledger lines 18,34-40`). R13 proves the one-turn no-tool fixture cannot compact because active turns must exceed 10 and automatic threshold pressure is unreachable for the short fixture (`R13 ledger lines 19-22,75-77`). R02 itself remains pilot-blocked on stale-tier/product fixture conditions, which the R12 ledger preserves as a remaining risk.

7. **R09 posture is preserved.**  
   Evidence: R12 ledger lines 226-237 record visible R09 debt and forbid hiding it with `--update`. R09 ledger lines 26-30 bind every seam to trusted classifier semantics, no blanket baseline update, visible red debt, and pilot green-gate conditions. This documentation-only commit does not claim source-debt reduction.

### Low

8. **Commit scope is correct and implementation slice separation holds.**  
   Evidence: `git diff-tree --no-commit-id --name-status -r 99e153edf` shows only three added documentation paths under `docs/fork/recovery/seams/R12-agent-turn-evidence/`: `grok-review.md`, `ledger.md`, and `opus-review.md`. Numstat is `208/0`, `288/0`, and `165/0`. No source or test file is changed by the commit.

9. **Narrow component tests pass, but only after correcting an invalid `--exact` attempt.**  
   Evidence: my first direct `cargo` attempt failed because `cargo` was not on PATH. A later `--exact` run exited 0 but ran 0 tests, so I discarded it. The guarded rerun through the offline Nix dev shell used `/tmp/jcode-r12-sol-target`, a disposable `/tmp` `JCODE_HOME`, `JCODE_NO_TELEMETRY=1`, and required nonzero pass counts. It ran one passing test each for `finish_evidence_turn_populates_assistant_checkpoint`, `evidence_events_are_searchable_and_distinctly_labeled`, `all_v1_event_kinds_round_trip`, and `messages_for_provider_applies_manual_compaction_in_native_auto_mode`.

## Decisive commands reproduced

```bash
cd /Users/jrudnik/labs/jcode-seam-r12

git rev-parse 99e153edf
# 99e153edf131f42668a0e51361904053108a8357

git merge-base 7ff4fc6be8dcf0410f2f61994752fdf5ee93e6e4 \
  802f6909825809e882d9c2d575b7e478dce57d3b
# 631935dd1d3b2e31e167e2b12ad463e54bcf4b8d

git merge-base --is-ancestor \
  7ff4fc6be8dcf0410f2f61994752fdf5ee93e6e4 \
  16921ace18cf5c25368a376357b7636478d3928f
# exit 0

for p in \
  crates/jcode-app-core/src/agent/evidence.rs \
  crates/jcode-app-core/src/agent/turn_execution.rs \
  crates/jcode-app-core/src/agent/turn_loops.rs \
  crates/jcode-app-core/src/agent/turn_streaming_mpsc.rs; do
  git diff --shortstat \
    7ff4fc6be8dcf0410f2f61994752fdf5ee93e6e4 \
    16921ace18cf5c25368a376357b7636478d3928f -- "$p"
done
# no output for all four paths

git cat-file -e \
  802f6909825809e882d9c2d575b7e478dce57d3b:crates/jcode-app-core/src/agent/evidence.rs
# fatal: path exists on disk, but not in upstream ref

sha256sum \
  docs/fork/recovery/seams/R12-agent-turn-evidence/opus-review.md \
  docs/fork/recovery/seams/R12-agent-turn-evidence/grok-review.md
# d3c19a9576f21e008b831594c13f09189527a98a20050d044e8d7e908e462a60  opus-review.md
# c12b96cbd935010405a05cd57a6caba7c56a5a0aca904c302ccc2cf6f52555d8  grok-review.md

cmp -s /tmp/jcode-r12-opus-review.md \
  docs/fork/recovery/seams/R12-agent-turn-evidence/opus-review.md
cmp -s /tmp/jcode-r12-grok-review.md \
  docs/fork/recovery/seams/R12-agent-turn-evidence/grok-review.md
# both exit 0

git diff-tree --no-commit-id --name-status -r 99e153edf
# A docs/fork/recovery/seams/R12-agent-turn-evidence/grok-review.md
# A docs/fork/recovery/seams/R12-agent-turn-evidence/ledger.md
# A docs/fork/recovery/seams/R12-agent-turn-evidence/opus-review.md

export CARGO_TARGET_DIR=/tmp/jcode-r12-sol-target
export JCODE_HOME=$(mktemp -d /tmp/jcode-r12-sol-home.XXXXXX)
export JCODE_NO_TELEMETRY=1
export RUST_TEST_THREADS=1
nix develop --offline . --command cargo test -p jcode-app-core finish_evidence_turn_populates_assistant_checkpoint --lib
nix develop --offline . --command cargo test -p jcode-app-core evidence_events_are_searchable_and_distinctly_labeled --lib
nix develop --offline . --command cargo test -p jcode-session-types all_v1_event_kinds_round_trip --lib
nix develop --offline . --command cargo test -p jcode-app-core messages_for_provider_applies_manual_compaction_in_native_auto_mode --lib
# each selected exactly 1 test, 1 passed, 0 failed

git status --short
# clean
```

## Residual gaps and caveats

- No full R12 end-to-end fixture currently exists that runs a deterministic no-tool turn and reads back exactly `TurnStarted`, `ProviderRequest`, `ProviderResponse{Ok, usage}`, `TurnFinished{Ok}` from R06A storage. This is the central reason the pilot is blocked today.
- Transport-error, cancellation, tool-continuation, and compaction/retry paths remain high-confidence static defects or blockers, not deterministic regression tests.
- R02 is independently pilot-blocked on stale-tier/product-fixture conditions, so even a future R12 strict fixture would not by itself admit the overall pilot.
- The offline Nix shell was used only after confirming `nix develop --offline` could provide cargo. Its shell hook printed idempotent git-hook/rerere setup messages; final `git status --short` was clean and I did not edit refs, stashes, source, tests, or worktree files.
- I intentionally did not run live daemon, network, credential, publication, broad gate, or destructive commands.

## Final sign-off

Sol signs off **PASS** on commit `99e153edf131f42668a0e51361904053108a8357` as an authoritative, hash-preserving, documentation-only R12 adjudication that retains the fork evidence spine, preserves both reviews, exactly records terminal-cardinality defects, carries R02/R06A/R07C/R09/R13 obligations, separates future implementation slices, and keeps the current pilot blocked until the strict fixture and dependent seam conditions are satisfied.
