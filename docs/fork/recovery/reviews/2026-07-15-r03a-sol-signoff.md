# Sol sign-off: R03A wire compatibility ledger

## Verdict

**PASS** for the committed R03A ledger as an accurate authoritative adjudication document at commit `a60cc2d6c53153b492f16160a5d64e20fe23f60c`.

**FAIL / BLOCKED** for pilot entry, exactly as the ledger states. The fork should be retained for R03A, but current incompatible-handshake enforcement is not fail-closed and the pilot must not proceed until the named blockers are fixed and tested.

## Exact commit

- Repository: `/Users/jrudnik/labs/jcode-seam-r03a`
- Commit checked: `a60cc2d6c53153b492f16160a5d64e20fe23f60c`
- Commit subject: `docs: adjudicate R03A wire compatibility seam`
- Worktree after verification: `git status --short` returned no output.

## Severity findings

### High, implementation blockers preserved by ledger

1. **Server continuation after incompatible verdict is real.**
   - Ledger evidence: `docs/fork/recovery/seams/R03A-wire-compatibility/ledger.md:52`, adjudication at lines 72 and exact contract at line 82.
   - Source evidence: `crates/jcode-app-core/src/server/client_lifecycle.rs:1368-1373` calls `evaluate_and_notify(...)` and discards the verdict. Normal subscribe handling reaches `client_subscribed = true` at `crates/jcode-app-core/src/server/client_lifecycle.rs:1527`.
   - Fixture evidence: `crates/jcode-app-core/src/tool/communicate_tests/end_to_end.rs:749-756` explicitly expects `Done` after a mismatched `IncompatibleReconnect` verdict.
   - Conclusion: ledger correctly rejects event ordering as enforcement. The fail-closed contract must make an advertised incompatible subscribe terminal.

2. **Generic client advertises identity without enforcing the verdict.**
   - Ledger evidence: `docs/fork/recovery/seams/R03A-wire-compatibility/ledger.md:53`, narrowed adjudication at line 73, fixture gap at line 97.
   - Source evidence: `crates/jcode-app-core/src/server/client_api.rs:84-101` sends `protocol_version` and `build_hash`; `crates/jcode-app-core/src/server/client_api.rs:107-115` exposes only raw `read_event()`.
   - Conclusion: any generic-client pilot remains blocked unless the client consumes and enforces the verdict or stops advertising identity.

3. **TUI re-exec guard/same-target paths attach after incompatibility.**
   - Ledger evidence: `docs/fork/recovery/seams/R03A-wire-compatibility/ledger.md:54`, adjudication at line 74, exact action contract at line 83.
   - Source evidence: `crates/jcode-tui/src/tui/app/handshake.rs:61-66` returns `Attach` when `already_reexeced`; `crates/jcode-tui/src/tui/app/handshake.rs:70-77` returns `Attach` when target equals current.
   - Test evidence: `crates/jcode-tui/src/tui/app/handshake.rs:232-254` asserts both attach behaviors.
   - Conclusion: ledger correctly blocks TUI pilot until second mismatch and same-target cases refuse with a visible diagnostic and no attach.

### Medium-high, semantic blocker preserved by ledger

4. **`Subscribe.build_hash` is only a compatibility token, not canonical R01 identity.**
   - R03A ledger evidence: lines 31, 39, 51, 71, 84, 95, 105, and 115.
   - R01 ledger evidence: `docs/fork/recovery/seams/R01-runtime-build-identity/ledger.md:35-41` defines canonical identity as source, executable, and activation provenance; `:41` states `Request::Subscribe.build_hash` is only the R03A compatibility projection.
   - Source evidence: `crates/jcode-tui/src/tui/backend.rs:338-339` and `crates/jcode-app-core/src/server/handshake.rs:72-74` use `jcode_build_meta::GIT_HASH`; `crates/jcode-build-meta/src/lib.rs:9-11` exposes version and short git hash, not dirty/fingerprint/channel.
   - Conclusion: ledger correctly prevents any claim that a compatible verdict equals canonical identity equality.

### Medium, protocol governance risk preserved by ledger

5. **Verdict vocabulary is not freely additive for advertising clients.**
   - Ledger evidence: `docs/fork/recovery/seams/R03A-wire-compatibility/ledger.md:55` and protocol-bump rule at line 85.
   - Source evidence: `crates/jcode-protocol/src/wire.rs:38-47` defines a serde enum with two known variants; `crates/jcode-protocol/src/wire.rs:838-849` serializes the enum in `HandshakeVerdict`.
   - Conclusion: the ledger’s protocol-bump governance is necessary before new verdict variants, required fields, legacy-`None` reinterpretation, incompatible-action changes, or richer projection reliance.

### Hygiene and preservation

6. **Review hashes and byte preservation verified.**
   - Ledger claims: Opus `5d0cf7131f5ff43e932e79aa061d7734ec1399e622b447fa8b4142ab946f689e`; Grok `c161f8951215b2413bbab62c292cb8864d02da018cb06c79b325cee4be6f0945`.
   - Commands reproduced both hashes and `cmp -s` identity against `/tmp/jcode-r03a-opus-review.md` and `/tmp/jcode-r03a-grok-review.md`.
   - The authored ledger hash observed: `626a064967527f0a75c6cdefa81b2972e9cbd59a0d6ddf41ff77d5e3a7d59609`.

7. **Trailing-space diagnostics are preservation, not ledger-authored hygiene debt.**
   - `perl -ne 'print "$.:$_" if /[ \t]+$/ ' docs/fork/recovery/seams/R03A-wire-compatibility/ledger.md` returned no lines.
   - The same check on `grok-review.md` returned exactly lines 3, 4, 5, and 6 with trailing spaces, matching Markdown hard-break preservation in the byte-identical Grok artifact.

## Required topic checklist

- **Fixed-ref authority:** PASS. `git rev-parse --verify` succeeded for fork `7ff4fc6be8dcf0410f2f61994752fdf5ee93e6e4`, upstream `802f6909825809e882d9c2d575b7e478dce57d3b`, and merge base `631935dd1d3b2e31e167e2b12ad463e54bcf4b8d`; `git merge-base fork upstream` returned the recorded base.
- **No upstream/base R03A authority:** PASS. `git grep -n 'HandshakeCompatibility'` and `git grep -n 'PROTOCOL_VERSION'` found no matches in base or upstream under the protocol/app-core/TUI source paths, and many fork matches.
- **Additive wire semantics:** PASS. Current `Subscribe.protocol_version` and `build_hash` are `Option` fields with `serde(default, skip_serializing_if = "Option::is_none")` at `crates/jcode-protocol/src/wire.rs:221-232`. Base/upstream `Subscribe` bodies are identical and lack these fields; no `deny_unknown_fields` was present in base/upstream `wire.rs`.
- **Compatibility-token projection limits:** PASS. Ledger correctly binds R03A to R01 projection semantics and blocks canonical identity claims.
- **Terra’s three enforcement defects:** PASS. Server continuation, generic advertisement without enforcement, and re-exec guard attach all reproduce in source/tests.
- **Exact fail-closed contract:** PASS. Ledger lines 80-85 are precise: incompatible advertised Subscribe is terminal; no subscribed state, resume/takeover, connection/member/PID/tool mutation, or successful `Done`; clients advertising identity must consume verdict first; one distinct re-exec only; no target, same target, failed re-exec, or second mismatch refuses visibly.
- **Protocol bump governance:** PASS. Ledger line 85 correctly identifies non-additive semantic/vocabulary changes requiring `PROTOCOL_VERSION` bump, migration docs, and fixture matrix expansion.
- **Fixture plan:** PASS. Ledger lines 87-99 cover legacy serde, pure verdict matrix, missing R01 projection, server terminal temp-socket fixture, generic-client fixture, TUI action fixture, and no-network guard. Current server mismatch fixture intentionally proves the opposite of the desired terminal behavior.
- **R09 policy:** PASS. R09 ledger lines 24-30 forbid hidden gate/baseline updates and require visible attributed red debt; R03A ledger line 123 adopts this and claims no new Rust debt because it is a docs-only ledger.
- **Scope and R02 exclusion:** PASS. R03A ledger lines 28-31 and 56 correctly exclude provider/model/account truth; R02 ledger lines 24-29 reserve provider/model/account outcomes to R02/R12.
- **Blocked pilot verdict:** PASS. R03A ledger lines 11, 103-106, and 131 consistently state retain-fork but pilot blocked.

## Commands run

```bash
pwd && git rev-parse --show-toplevel && git rev-parse HEAD && git status --short && git show --no-patch --format='%H%n%ci%n%s' a60cc2d6c
find . -type f | rg -i 'r03a|r00|r01|r02|r09|r11|opus|grok|terra|ledger|review' | rg -vi 'fable|target|\.git' | sort
git rev-parse a60cc2d6c
for r in 7ff4fc6be8dcf0410f2f61994752fdf5ee93e6e4 802f6909825809e882d9c2d575b7e478dce57d3b 631935dd1d3b2e31e167e2b12ad463e54bcf4b8d; do git rev-parse --verify "$r^{commit}"; done
git merge-base 7ff4fc6be8dcf0410f2f61994752fdf5ee93e6e4 802f6909825809e882d9c2d575b7e478dce57d3b
for r in 631935dd1d3b2e31e167e2b12ad463e54bcf4b8d 802f6909825809e882d9c2d575b7e478dce57d3b 7ff4fc6be8dcf0410f2f61994752fdf5ee93e6e4; do git grep -n 'HandshakeCompatibility' "$r" -- crates/jcode-protocol/src crates/jcode-app-core/src crates/jcode-tui/src || true; git grep -n 'PROTOCOL_VERSION' "$r" -- crates/jcode-protocol/src crates/jcode-app-core/src crates/jcode-tui/src || true; done
git show 631935dd1d3b2e31e167e2b12ad463e54bcf4b8d:crates/jcode-protocol/src/wire.rs | nl -ba | sed -n '100,130p'
git show 802f6909825809e882d9c2d575b7e478dce57d3b:crates/jcode-protocol/src/wire.rs | nl -ba | sed -n '100,130p'
git grep -n 'deny_unknown_fields' 631935dd1d3b2e31e167e2b12ad463e54bcf4b8d -- crates/jcode-protocol/src/wire.rs || true
git grep -n 'deny_unknown_fields' 802f6909825809e882d9c2d575b7e478dce57d3b -- crates/jcode-protocol/src/wire.rs || true
shasum -a 256 docs/fork/recovery/seams/R03A-wire-compatibility/opus-review.md docs/fork/recovery/seams/R03A-wire-compatibility/grok-review.md docs/fork/recovery/seams/R03A-wire-compatibility/ledger.md
cmp -s /tmp/jcode-r03a-opus-review.md docs/fork/recovery/seams/R03A-wire-compatibility/opus-review.md && echo 'opus cmp: identical'
cmp -s /tmp/jcode-r03a-grok-review.md docs/fork/recovery/seams/R03A-wire-compatibility/grok-review.md && echo 'grok cmp: identical'
perl -ne 'print "$.:$_" if /[ \t]+$/ ' docs/fork/recovery/seams/R03A-wire-compatibility/ledger.md
perl -ne 'print "$.:$_" if /[ \t]+$/ ' docs/fork/recovery/seams/R03A-wire-compatibility/grok-review.md
CARGO_TARGET_DIR=/tmp/jcode-r03a-cargo-target bash scripts/dev_cargo.sh test -p jcode-protocol handshake_verdict -- --nocapture
CARGO_TARGET_DIR=/tmp/jcode-r03a-cargo-target bash scripts/dev_cargo.sh test -p jcode-app-core --lib server::handshake -- --nocapture
CARGO_TARGET_DIR=/tmp/jcode-r03a-cargo-target bash scripts/dev_cargo.sh test -p jcode-app-core handshake_emits_incompatible_verdict_for_mismatched_client -- --nocapture
git status --short
```

## Test results

- `jcode-protocol handshake_verdict`: **passed**, `7 passed; 0 failed`.
- `jcode-app-core --lib server::handshake`: **passed**, `3 passed; 0 failed`; one pre-existing dead-code warning for `drop_control_log_handle`.
- `jcode-app-core handshake_emits_incompatible_verdict_for_mismatched_client`: **passed**, `1 passed; 0 failed`; this is evidence of current behavior and confirms the fixture still expects post-verdict `Done`, not fail-closed terminal behavior.

## Residual gaps and limits

- I did **not** read any R03A future Fable sign-off. A path inventory found only `docs/fork/recovery/reviews/2026-07-15-r01-r02-fable-signoff.md`; I did not open it.
- I did not use a live daemon, credentials, network-oriented tooling, stash replay, destructive operations, publication, or source edits.
- I did not run broad R09 gates or compile TUI action tests. The R03A ledger is docs-only and the decisive TUI defect is directly visible in source and existing tests. Broad gates would exceed the requested decisive-only scope.
- The app-core tests required entering the repo Nix dev shell and writing build artifacts under isolated `CARGO_TARGET_DIR=/tmp/jcode-r03a-cargo-target`; no repository changes resulted.
- Pilot remains blocked until the R01 projection fixture, server terminal-action fixture, generic-client enforcement fixture if in scope, and TUI refusal fixture pass.
