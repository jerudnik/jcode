# Fable sign-off: R03A wire compatibility ledger

Date: 2026-07-15 UTC
Repository: `/Users/jrudnik/labs/jcode-seam-r03a`
Exact commit signed: `a60cc2d6c53153b492f16160a5d64e20fe23f60c`

## Verdict

**PASS for the committed R03A ledger as an evidence-backed adjudication.**

The ledger correctly selects **retain-fork** for the fork-only NS1 wire/verdict mechanism and correctly keeps **pilot entry blocked**. I do not sign off any implementation/pilot acceptance. The current implementation is not fail-closed for incompatible advertised clients, but the ledger states that as a blocker rather than overclaiming readiness.

## Severity-ranked findings

### Blocking for pilot, not blocking ledger sign-off

1. **Server-side incompatible verdict is not terminal.**
   - Evidence: `client_lifecycle.rs:1368-1373` calls `evaluate_and_notify(...)` and discards the returned `HandshakeCompatibility`; subscription processing then continues through `handle_subscribe(...)` at `client_lifecycle.rs:1469-1494` or `1497-1522` and sets `client_subscribed = true` at `client_lifecycle.rs:1527`.
   - Fixture evidence: `end_to_end.rs:721-747` expects `HandshakeVerdict::IncompatibleReconnect`; `end_to_end.rs:749-756` then explicitly expects a successful `Done` after the verdict.
   - Ledger treatment: correctly captured at `R03A ledger:52`, adjudicated as a present defect at `R03A ledger:72`, and converted into a fail-closed contract at `R03A ledger:82` and fixture requirement at `R03A ledger:96`.

2. **Generic `server::Client` advertises identity but exposes only raw verdict handling.**
   - Evidence: `client_api.rs:84-101` sends `protocol_version: Some(PROTOCOL_VERSION)` and `build_hash: Some(GIT_HASH)` from generic `subscribe_with_info`; `client_api.rs:107-115` exposes raw `read_event()` with no handshake-aware subscribe result.
   - Ledger treatment: correctly captured at `R03A ledger:53`, narrowed in adjudication at `R03A ledger:73`, and made a required fixture only if generic client is in scope at `R03A ledger:97` and `105`.

3. **TUI re-exec loop guard can attach after a still-incompatible verdict.**
   - Evidence: `tui/app/handshake.rs:61-66` returns `Attach` when `already_reexeced`; `tui/app/handshake.rs:70-77` also attaches when target equals current; tests assert those outcomes at `tui/app/handshake.rs:232-254`.
   - Ledger treatment: correctly captured at `R03A ledger:54`, adjudicated at `R03A ledger:74`, and corrected in the exact contract at `R03A ledger:83` and fixture matrix at `R03A ledger:98`.

4. **R01 canonical identity is not represented by `Subscribe.build_hash`.**
   - Evidence: R01 defines canonical runtime identity as source, executable, and activation provenance and labels `Request::Subscribe.build_hash` as only an R03A compatibility projection at `R01 ledger:35-41`. Current carriers stamp only `jcode_build_meta::GIT_HASH`: TUI at `backend.rs:335-339`, generic client at `client_api.rs:93-98`, server at `handshake.rs:71-74`; `jcode-build-meta/src/lib.rs:8-11` exposes version and git hash, not dirty/source/channel tuple.
   - Ledger treatment: correctly scoped at `R03A ledger:29-31`, captured as evidence at `R03A ledger:51`, and made a missing deterministic fixture at `R03A ledger:95`.

### Medium

5. **Source comments overclaim enforcement even though ledger does not.**
   - Evidence: `wire.rs:34-36` says the server “enforces” the verdict at connect time; `wire.rs:833-835` says the client re-execs instead of attaching. The implementation has server continuation and guard attach paths above.
   - Impact: this is a documentation/source-comment mismatch, not a ledger defect. The ledger supersedes the comments by blocking pilot and specifying fail-closed semantics.

6. **Protocol-bump governance is necessary and sufficiently stated in the ledger.**
   - Evidence: `PROTOCOL_VERSION` is documented as needing a bump for unsafe disagreement at `lib.rs:15-26`; `HandshakeVerdict.compatibility` is a serialized enum at `wire.rs:837-848`, so new verdict variants are not additive for existing advertising clients.
   - Ledger treatment: correctly called out at `R03A ledger:55` and contractually governed at `R03A ledger:85`.

### Low / hygiene

7. **Review preservation verified; authored ledger hygiene is clean.**
   - Evidence: SHA-256 reproduced as Opus `5d0cf7131f5ff43e932e79aa061d7734ec1399e622b447fa8b4142ab946f689e`, Grok `c161f8951215b2413bbab62c292cb8864d02da018cb06c79b325cee4be6f0945`; `cmp -s` against `/tmp/jcode-r03a-{opus,grok}-review.md` succeeded. Trailing-space scan found `ledger.md: []`, `opus-review.md: []`, and `grok-review.md: [3,4,5,6]`. `git diff --check` reported no authored ledger issue. The Grok trailing spaces match the intentional byte-identical preservation note in `R03A ledger:127`.

8. **R09 posture is preserved.**
   - Evidence: R09 forbids blanket `--update` and requires red debt visibility at `R09 ledger:24-30`; R03A changed no Rust source in the ledger commit and states no new debt plus future no-`--update` obligations at `R03A ledger:121-127`.

## Required challenge points

| Challenge point | Result | Evidence |
|---|---|---|
| Fork authority | Supported | Fixed-ref symbol search found `HandshakeCompatibility`/`HandshakeVerdict` absent at base/upstream and present at fork/head; R03A records this at `ledger:47-48` and `70`. |
| Legacy additivity | Supported | Optional `protocol_version`/`build_hash` use `serde(default, skip_serializing_if)` at `wire.rs:221-232`; old-shape decode to `None` at `misc_events.rs:382-418`; no verdict to legacy at `handshake.rs:57-65` and `end_to_end.rs:818-867`. |
| Projection defect | Supported | R01 ledger `35-41`; TUI/generic/server all use only `GIT_HASH`; ledger blocks pilot at `51`, `71`, `95`. |
| Server-side continuation after incompatible verdict | Supported | `client_lifecycle.rs:1368-1527`; `end_to_end.rs:749-756`; ledger `52`, `72`, `82`, `96`. |
| Generic client exposure | Supported | `client_api.rs:84-115`; ledger `53`, `73`, `97`, `105`. |
| Reexec guard behavior | Supported | `handshake.rs:61-66`, `70-77`, `232-254`; ledger `54`, `74`, `83`, `98`. |
| Fail-closed contract | Supported as ledger requirement, not implementation | Current implementation fails; ledger requires terminal incompatible at `82-83` and fixture changes at `96-98`. |
| Protocol-bump governance | Supported | `lib.rs:15-26`; enum event `wire.rs:837-848`; ledger `55`, `85`. |
| Deterministic fixtures | Supported as acceptance plan with gaps | Existing floor in protocol/server temp-socket tests; missing/blocking fixture rows at `R03A ledger:91-99`. |
| Blocked pilot verdict | Supported | Top-level `Pilot entry verdict blocked` at `R03A ledger:11`; recommendation at `101-106`. |
| Scope boundaries | Supported | R03A owns/excludes lines `28-31`; R01 owns identity at `R01 ledger:28-31,35-41`; R02 excludes R03A verdicts at `R02 ledger:24-29`. |
| Review preservation | Supported | `R03A ledger:15-24`; hashes/cmp reproduced; Grok trailing spaces isolated. |
| R09 posture | Supported | `R09 ledger:24-30`; R03A `121-127`. |
| R11 posture | Supported | Append-only/hash requirements at `R11 ledger:24-29`; R03A preservation/hash table at `15-24`. |
| R00 posture | Supported | Fixed refs, provenance, no stash replay, stop budgets at `R00 ledger:28-31`; R03A states fixed refs and no mutation at `24`. |

## Commands and read-only evidence reproduced

```bash
git rev-parse HEAD
git rev-parse a60cc2d6c
git status --short
git show -s --format='%H%n%an%n%ae%n%ad%n%s' a60cc2d6c
find docs/fork/recovery -path '*/reviews/*sol*' -prune -o -type f -print | sort
find . -path ./.git -prune -o -type f \( -iname '*R03A*' -o -iname '*R00*' -o -iname '*R01*' -o -iname '*R02*' -o -iname '*R09*' -o -iname '*R11*' -o -iname '*opus*' -o -iname '*grok*' \) -print | sort
git grep -n 'HandshakeCompatibility\|HandshakeVerdict\|PROTOCOL_VERSION' <base/upstream/fork/head> -- crates/jcode-protocol crates/jcode-app-core crates/jcode-tui
git show <base/upstream/head>:crates/jcode-protocol/src/wire.rs | grep -n -A45 -B5 'Subscribe {'
git grep -n 'deny_unknown_fields' <base/upstream/head> -- crates/jcode-protocol/src/wire.rs
shasum -a 256 docs/fork/recovery/seams/R03A-wire-compatibility/{opus-review.md,grok-review.md}
cmp -s /tmp/jcode-r03a-opus-review.md docs/fork/recovery/seams/R03A-wire-compatibility/opus-review.md
cmp -s /tmp/jcode-r03a-grok-review.md docs/fork/recovery/seams/R03A-wire-compatibility/grok-review.md
python3 trailing-space scan over R03A ledger, Opus review, Grok review
git diff --check -- docs/fork/recovery/seams/R03A-wire-compatibility/{ledger.md,opus-review.md,grok-review.md}
```

I also read targeted line ranges in the binding ledgers and source/tests listed above. I did not run cargo tests, a daemon, network access, credentials, stash replay, destructive commands, or publication.

## Residual gaps and process notes

- This is a ledger sign-off, not a remediation sign-off. The implementation remains blocked on server terminal behavior, R01 projection fixture, generic-client handling if in scope, and TUI guard refusal.
- I did not perform live daemon or real `exec` checks, consistent with the stop boundaries.
- A broad `git grep` over `docs` emitted one line from `docs/fork/recovery/reviews/2026-07-15-r01-r02-sol-signoff.md`. I did not use that line as evidence. I did not read or rely on any R03A Sol sign-off, and no R03A Sol sign-off was found in the file discovery output.
- Confidence: high that the ledger is truthful and sufficiently adversarial for retain-fork plus blocked-pilot disposition; medium-high for completeness of generic-client production exposure because I used static evidence and the ledger's preserved Grok note rather than a whole-callgraph proof.
