# Fork recovery progress

The coordinator owns this file. Append checkpoints instead of rewriting history.

## Phase status

| Phase | Gate | State | Evidence or blocker |
|---|---|---|---|
| Setup | Durable workspace and launch prompt exist | `complete` | Structurally validated and independently reviewed on 2026-07-15 |
| 0 | Truth and pre-screen | `complete` | Refreshed refs, preserved topology, mechanical pre-screen, trusted gate semantics, exact debt attribution, and explicit unknowns recorded through `f9c70d1be` |
| 1 | Responsibility map and triage | `complete` | Independent Luna mapper, Sonnet critic, focused Opus unclassified-path review, coordinator adjudication, and final Opus approval with findings addressed are recorded in `RESPONSIBILITIES.md` and `reviews/` |
| 2 | Priority seam ledgers | `complete` | All seventeen non-deferred responsibility ledgers are integrated. Full-seam disagreements, failed sign-offs, append-only corrections, bounded re-reviews, and blocked source states remain visible rather than being converted into approval. |
| 3 | Bounded pilot | `pending independent gate` | Previously named strict prerequisite nodes: **0**. Sequential R02 and R01/R03A corrections, two independent correction reviews per slice, combined validation, and current R09 truth are preserved under `evidence/`. This is not pilot authorization: the gate remains **OPEN, pending independent G2 adversarial adjudication**, which may inject new blockers. |
| 4 | Cross-seam architecture plan | `pending` | Fable review informed by pilot results and spot checks |
| 5 | Remediation | `pending` | Isolated implementation slices with validation |
| 6 | Final sign-off | `pending` | Grok audit evidence plus Sol and Fable partnership |

## Checkpoints

| UTC time | Phase | Summary | Commits | Next gate |
|---|---|---|---|---|
| 2026-07-15 | Setup | Established, reviewed, and structurally validated the durable recovery records and next-session prompt. | scaffold commit | Begin Phase 0 on a dedicated recovery branch |
| 2026-07-15T06:44:58Z | 0 | Created `recovery/2026-07-15`, preserved the dirty prompt and four stashes, fetched both remotes, and appended exact divergence/runtime/gate evidence. | `d756d6a2c` | Audit quarantined parsers, pre-screen seams, and list explicit unknowns |
| 2026-07-15T06:56:00Z | 0 | Added the mechanical responsibility pre-screen, incident attribution, stable patch-ID search, and provisional top-six full-review candidates without treating path overlap as authority. | `c2f2f4c73` | Repair the invalid duplicated test classifier and classify real gate debt |
| 2026-07-15T07:41:04Z | 0 | Integrated the independently reviewed shared Rust production filter, split parser-semantic and stale-ratchet corrections, and CI-wired 17 adversarial tests. | `fb1168a6a`, `0508e3f7b`, `0674fe53d`, `f9c70d1be` | Run the integrated truth-gate matrix and preserve review evidence |
| 2026-07-15T07:50:55Z | 0 | Passed the Phase 0 truth gate: trusted green gates pass, exact red debt remains visible, original-baseline replay passes, branch/worktree topology is recorded, reclaimed clean orchestrator worktrees are restored, and review debate is durable. | `7ff4fc6be` | Dispatch the independent mapper and map critic |
| 2026-07-15T08:23:16Z | 1 | Adjudicated 22 behavior/governance responsibilities from independent Mapper, Critic, and focused Opus gap evidence. Added the previously unowned agent-turn and compaction responsibilities, assigned exactly six full reviews, documented mandatory overlays and cross-seam invariants, and bounded the provider-turn pilot. | this Phase 1 docs checkpoint | Obtain final independent review before dispatch |
| 2026-07-15T08:33:29Z | 1 | Final Opus review approved the map after requiring a conservative all-writers provider-session invariant. The preserved user `ORCHESTRATOR_PROMPT.md` edit was explicitly recorded as out of Phase 1 scope rather than modified; its diff hash remains unchanged. | this Phase 1 docs checkpoint | Complete bounded re-review of the two findings |
| 2026-07-15T08:36:40Z | 1 | Bounded Opus re-review verified both IMPORTANT findings and all minor notes resolved with no remaining CRITICAL or IMPORTANT findings. The six-seam cap and preservation invariants remain intact. | this Phase 1 docs checkpoint | Dispatch at most two full seam teams and the prerequisite light ledgers |
| 2026-07-15T09:42:30Z | 2 | Integrated independently reviewed R01 and R02 full ledgers plus R06A, R07C, and R13 pilot-prerequisite light ledgers. R01 retains fork reload authority but blocks pilot entry on dirty-build identity projections. R02 composes fork provenance/routing with bounded upstream candidates but blocks on stale-tier safety and product-owned tier fixtures. R06A storage round trip, R07C reporting opt-out, and R13 one-turn compaction avoidance are approved; independent sign-offs and bounded correction reviews are preserved in `reviews/`. | `df0e67f0d`, `6c5104e32`, `8317f0e3d` plus this sign-off checkpoint | Finish R03A, R04, R05B, and R12 full ledgers and remaining prerequisite light ledgers before implementing pilot fixes |
| 2026-07-15T10:39:40Z | 2 | Integrated independently reviewed R03A and R12 full ledgers. R03A retains the fork-only additive handshake but blocks pilot entry on canonical projection, fail-closed server enforcement, advertising-client verdict handling, and second-mismatch refusal. R12 retains the fork-only evidence spine but records global under-emission and cancellation defects; an initial Sol pass and Fable IMPORTANT finding were preserved, Terra corrected the blocking `StreamEvent::Error` matrix without rewriting history, and bounded Sol/Fable re-reviews passed. | `31b5f0a5b`, `69dee1d62`, `02db723d5`, `ccb5c37be`, `05a129d72` | Complete R04 and R05B full ledgers plus R03B, R05A, R07A, R08A, and R10 light ledgers before any pilot implementation |
| 2026-07-15T11:44:00Z | 2 | Completed the ledger gate. Integrated R03B, R05A, R07A, R08A, and R10 light ledgers after independent Opus review; the warmed isolated R03B socket and live-attachment fixtures passed 20/20 and 1/1 after a zero-test filter was corrected append-only. Integrated R05B as `retain-fork` but source-blocked for swarm widening; Sol and Fable passed the blocked ledger. Integrated R04 with preserved Opus/Grok reviews, then preserved independent Sol/Fable failures that found an unsafe `reconcile_dead_owner`/disconnect marker-removal ordering and an incomplete terminal-writer census. Terra appended a source-blocking correction, and bounded Sol/Fable re-reviews passed the correction without approving the unfixed source. | `4f6470dc9` through `041106400`, `e75a5af71`, `fe32e3aad`, `1c933a01c`, `9ca490316`, `c2e3d6cb6`, plus this checkpoint | Hold Phase 3. Implement and independently verify only the named prerequisite fixes and fixtures before considering the bounded pilot. |
| 2026-07-15T13:03:23Z | 3 | Integrated the narrow R04 marker-durability prerequisite after two independent PASS reviews. Terminal state now persists before conditional marker cleanup; save failures remain observable and retain retry evidence; removal requires unchanged content and file identity; unstable observations and lock/agent-timeout paths fail closed; replaced live successors survive. Focused storage, reconciliation, and disconnect fixture matrices passed. Reviewer follow-ups for streaming-marker partial-outcome coverage and disconnect return-contract clarity remain visible but do not block this narrow prerequisite. | `a371fe758`, `d6733bd5c`, `eab42e1b5`, `61f9f9e60`, `f42ccd720` | Remaining blocker count: 3. Finish R12 re-review/integration, then R01/R03A and R02. No pilot is authorized. |
| 2026-07-15T13:20:57Z | 3 | Integrated the corrected narrow R12 package after preserving the initial Opus PASS/Fable FAIL disagreement and obtaining two bounded PASS re-reviews. The coordinator deliberately omitted the superseded in-place documentation rewrite and committed the reviewed cumulative ledger as a zero-deletion append-only amendment. Integrated validation passed 29/29 focused R04/R12 tests; the R09 matrix preserved 17 classifier tests, warning, and wildcard gates green while panic, swallowed-error, production-size, and test-size debt remained visibly red without `--update`; the full TUI selfdev build passed. No reload or daemon activation occurred. | `a4d673ffd`, `8bb7afc16`, `2ef1041f9`, `8ac1c0f55`, `6663e5b3e`, `1b1dc2a6d`, plus this checkpoint | Remaining blocker count: 2. Start isolated R01/R03A and R02 teams within the two-team cap. No pilot is authorized. |
| 2026-07-15T19:34:00Z | 3 gate preparation | Rehydrated exact HEAD and preservation state; byte-exact archived the surviving twelve-step combined validation and R09 logs; reproduced review hashes; recorded the immutable no-reload build; and reran the non-build R09 matrix with encoded exits. R02 and R01/R03A integrated correction chains reduce the previously named strict prerequisite-node count to zero. Historical swallowed counts 3,077 and 3,072 remain preserved; current fixed-HEAD truth is 3,074. | R02 `3063fe0fa` through `cb924b3ae`; R01/R03A `615ab1d9a` through `6c6a4f2c8`; this evidence/status checkpoint | Pilot authorization remains OPEN pending independent G2 adjudication. No pilot is authorized by this checkpoint. |

## Active blockers

- All seventeen non-deferred responsibility ledgers are integrated: R00, R01, R02, R03A, R03B, R04, R05A, R05B, R06A, R07A, R07C, R08A, R09, R10, R11, R12, and R13. `complete` means the evidence ledger is complete, not that every source seam is approved.
- The previously named strict prerequisite-node count is zero at integrated HEAD `6c6a4f2c8`. R01/R03A now project exact dirty-source identity, preflight incompatible initial Subscribe before mutation, and preserve compatible/legacy behavior; R02 fails closed for stale, unknown, malformed, contradictory, and authoritative-denial tier states using product-owned fixtures. Pilot authorization is still OPEN pending independent G2 adjudication, not passed.
- The narrow R04 prerequisite is integrated and independently verified. Reviews are preserved as `reviews/2026-07-15-r04-marker-fix-opus-review.md` (SHA-256 `7a8f24490806a6aa30bf4d16947a6e4ff2fee76c67589972fcadc0d96fb1a9de`) and `reviews/2026-07-15-r04-marker-fix-fable-review.md` (SHA-256 `1ec0ceb5c333da18c814ba96a9392fd6fad398b6e3df9b00aafd0c1ee902f73d`). Both pass the narrow source fix; Fable's non-blocking follow-ups remain recorded.
- The narrow R12 prerequisite is integrated and independently verified. The initial disagreement is preserved by `reviews/2026-07-15-r12-evidence-fix-opus-review.md` (SHA-256 `69089dd0f3fa60af4e4c186bae676aaece6fc38ee0ab374845fba9ce1545c40c`) and `reviews/2026-07-15-r12-evidence-fix-fable-review.md` (SHA-256 `96af645b7454015a2a25fec8391e27d20511e591d8eadc47a99df143c172da63`); bounded correction passes are `reviews/2026-07-15-r12-evidence-fix-opus-rereview.md` (SHA-256 `529b398ae809d65ac5160235cf6820de725ea0100bf9121ded65db5c6b0a7466`) and `reviews/2026-07-15-r12-evidence-fix-fable-rereview.md` (SHA-256 `5c7335f855de29dace69377b7dfad72243b0ac52b480195cff805b689dfaa771`). Cancellation, retry/compaction, live-provider, and tool-continuation widening remain blocked.
- R03B's isolated Unix transport fixtures now pass from a warmed cache with a disposable `JCODE_HOME`: 20 socket tests and one exact live-attachment fanout test. WebSocket/mobile attach remains explicitly deferred to full review.
- R05B retains unique fork churn/reclaim defenses, but explicit `Visible` spawn silently falls back to headless, stale direct takeover resets progress history, cap-fail overwrites checkpoint provenance, and liveness predicates are duplicated. These block any swarm-driven widening but are outside the strict no-swarm pilot prerequisite set.
- R08A onboarding/import, R07A tool/MCP execution, and R10 remote release/install/update activation remain fail-closed outside the bounded pilot. Their light ledgers do not authorize credentials, MCP/network access, publication, installation, updater execution, or daemon activation.
- No exact stable patch-ID cluster was shared across the compared non-merge
  ranges. Semantic equivalence hidden by curated synchronization remains an
  explicit per-seam research question.
- The production-size, test-size, panic, and swallowed-error ratchets remain red
  with real debt. Phase 0 classified it but did not weaken or remediate it; R09
  and the owning behavior seams must decide bounded cleanup slices.
- `vendor/upstream` remains pinned to the merge base and cannot be treated as a
  current upstream source.
- The R12 agent-turn and R13 compaction responsibilities were absent from both
  initial maps and were found only by inspecting the unclassified change set.
  Phase 2 must keep unclassified support paths assigned to an observable owner
  rather than reverting to keyword file buckets.
- The user's pre-existing `ORCHESTRATOR_PROMPT.md` edit remains preserved and is
  not adopted as a Phase 1 authority change. Its diff SHA-256 remains
  `8e8e6a92dad180b3925bc0b2a3b7b951bc6a6f5c9e4f8a57c9f522d03ad85c00`; any
  alteration remains user-controlled.

## Resolved Phase 0 blockers

- The duplicated panic/swallowed-error classifier was replaced with one shared,
  adversarially tested implementation and independently approved after its
  baseline corrections were split by cause.
- The three clean orchestrator worktrees reclaimed while stopping stale sessions
  were recreated at their exact recorded paths, branch names, and SHA.
- Old maintenance notes were reconciled against ancestry; already-landed fixes
  and preserved hot-path stashes are evidence, not replay instructions.

### 2026-07-15T19:49:00Z G2 authorization amendment

Independent Opus G2 review returned **PASS** for one precisely bounded, fixture-backed Phase 3 pilot at reviewed commit `16e52bf4bcdffb0e8aea46266488960673e8ee5f`. The byte-exact review is [`reviews/2026-07-15-g2-pilot-gate-opus.md`](./reviews/2026-07-15-g2-pilot-gate-opus.md), SHA-256 `abb7b2694abccb0c32385fc552dcc29bf0eba854d439c5c43dc82ba4f3991e4f`. No Cargo/Nix build or test was run by the reviewer; the verdict used source inspection plus manifest-verified preserved logs.

G2 authorizes G3 design, not immediate execution. Before G4, create and separately commit the required noninteractive validation driver. The pilot remains one process, one session, one compatible subscribe, and one no-tool turn in disposable paths with telemetry disabled. Current R09 truth is panic `46`, swallowed `3,074`, production-size `61`, and test-size `31`; no `--update` is permitted.

### 2026-07-15T20:23:37Z G4 bounded-pilot amendment

The exact offline fixture pilot passed all ten driver steps at source HEAD `505cd86726f86dc0eedaf3998afae6ed83290d5d`. Expected-red R09 gates remained red at panic `46`, swallowed `3,074`, production-size `61`, and test-size `31`; classifier `17/17`, wildcard `16`, warning `0`, shell syntax, and diff check passed. The pilot emitted exactly one deterministic observation and zero forbidden-output hits.

Preflight and postflight both recorded only the preserved prompt edit, prompt diff SHA-256 `8e8e6a92dad180b3925bc0b2a3b7b951bc6a6f5c9e4f8a57c9f522d03ad85c00`, four stashes, `vendor/upstream` at `631935dd1d3b2e31e167e2b12ad463e54bcf4b8d`, and no active build. The successful evidence manifest SHA-256 is `b4692dc023075d89fcbe94065d089234fa59bbc5777215082870eb00c3842343`; failed launch, preflight, and observation-framing attempts remain append-only under `evidence/2026-07-15-g4-attempt-history/`.

G4 coordinator validation is complete. Phase advancement remains blocked pending independent review of the fixed evidence/status commit. No live provider, credential, network, daemon, reload, tool/MCP, memory, publication, installation/update, cancellation, retry, compaction, disconnect/takeover, or quality-baseline authority is inferred.

### 2026-07-15T20:34:20Z G5 independent-review amendment

Independent Anthropic Opus reviewed fixed G4 evidence/status commit `da7c155b9d34ff719e065c855338eea3574d62a9` and returned **PASS** with high confidence and no blocking findings. The reviewer independently verified both evidence manifests and memberships, all named hashes, ten expected/actual exits, one standalone observation, zero forbidden hits, preservation equality, fixture behavior, driver fail-closed properties, append-only history, commit separation, the prompt hash, and four stashes. The artifact SHA-256 is `37f094d26b196612f2171de98d52238abb72bb8b69d59b149e7bb00999db86d3`.

G4/G5 therefore close the exact bounded-pilot gate. This does not approve live providers, credentials, network, daemon/reload, tools/MCP, memory, publication/install/update, cancellation, retry, compaction, disconnect/takeover, generic-client identity, or quality-baseline changes. The next recovery action must remain a separately authorized architecture/planning gate informed by this narrow result.

### 2026-07-15T20:54:37Z Phase 4 architecture-plan amendment

Fable synthesized all seventeen ledgers and the reviewed G4/G5 pilot into a proposed curated-composition plan. Coordinator audit rejected two bounded overclaims in v1: it counted only thirteen retain-fork dispositions by omitting R04, and it required exactly one request on retry paths rather than one correlated terminal response per emitted request. Corrected v2 records fourteen retain-fork, two compose, one defer and the exact R12 cardinality invariant.

The coordinator-audited [`RECOVERY_PLAN.md`](./RECOVERY_PLAN.md) orders W0 record closure; W1 R12 cancellation/retry evidence; W2 R05B spawn/reclaim safety; W3 R04 lifecycle widening; product-gated W4 R02 composition; W5 onboarding consent; W6 acquisition/release integrity; and optional W7 refactors. W0 is next only after independent review of the fixed plan commit. No implementation or widened pilot is authorized by this checkpoint.

### 2026-07-15T21:02:12Z architecture-gate PASS amendment

Independent Anthropic Opus reviewed fixed plan commit `76ead5607032ef9e574979a779f6fddc60607b23` and returned **PASS** with high confidence and no blocking findings. It independently reproduced the plan and Fable hashes, four-hunk correction history, all seventeen disposition lines, W0-W7 mapping, R12 retry cardinality, blockers, concurrency rules, preservation, and claim limits. Review SHA-256: `3f2d31cb5fb9ead893ed8b1e4ce451072757cc5d0206236833dac1b3a886fe92`.

Phase 4 architecture is complete. The next approved slice is W0 docs/evidence record-consistency closure. No source remediation or external action is authorized by this checkpoint.

### 2026-07-15T21:04:40Z W0 record-consistency amendment

W0 appended superseding status to nine ledgers without deleting historical prose. It closes the stale R04 source-review requirement using independent Opus/Fable PASS artifacts, accepts the five remaining light ledgers only within their original fail-closed boundaries, discharges R00/R09/R11 Fable-pending lines through the approved Phase 4 plan, and records production-size `60` as historical versus current `61`.

W0 also found and corrected one plan-record omission: W3 now explicitly carries the R04 streaming-marker partial-outcome fixtures alongside disconnect outcome clarity and marker-lock liveness. No source, test, daemon, network, credential, external action, or quality baseline changed. W0 remains pending independent mechanical review.

### 2026-07-15T21:12:19Z W0 independent PASS amendment

Independent Opus mechanical review of fixed commit `11a78a858f14a2722f67efdaefc3025360dc19c6` returned **PASS**, high confidence, with no blocking findings. The review reproduced the append-only diff, five cited artifact hashes, nine ledger closures, 60/61 count history, W3 streaming-marker correction, sole dirty prompt path/hash, and four stashes.

The byte-preserved review is [`reviews/2026-07-15-w0-opus-review.md`](reviews/2026-07-15-w0-opus-review.md), SHA-256 `bd662db1792edcfed7276aed3203fd173f047daa58747ca8bcbabca290999fd3`. W0 is complete. W1/W2 source work remains stopped pending separate authorization.


### 2026-07-16T03:55:17Z W1 integration PASS amendment

W1 R12 cancellation/retry terminal evidence is integrated on
`recovery/2026-07-15`. The nine-commit source, test, ledger, disagreement,
remediation, review, and final-approval chain was cherry-picked from
`602709895be96a85a6090690c0b27d5681d17321` through integrated review head
`14682f2f2a9a811edc7213762fdc7dfa423bd0cc`. The final independent reports are
[`reviews/2026-07-16-w1-remediation-opus-rereview.md`](reviews/2026-07-16-w1-remediation-opus-rereview.md), SHA-256
`6be07ab6a4c360b414105046555c72f9ba7a1e6f28589903fab37a44f541206f`, and
[`reviews/2026-07-16-w1-remediation-fable-rereview.md`](reviews/2026-07-16-w1-remediation-fable-rereview.md), SHA-256
`bd32b46d57aa2b345f0fa4d1c82315b5b394f7498d9b1d82d802aa7e1912fd43`.
Both return **PASS** and close the prior Fable IMPORTANT raw-text persistence
finding without deleting the earlier Opus PASS/Fable FAIL disagreement.

The authoritative post-integration fixture run set
`FORK_NUDGE_MAX_AGE=2147483647`, `FORK_NUDGE_AUTOSYNC=0`,
`JCODE_NO_TELEMETRY=1`, used a disposable `JCODE_HOME`, and invoked
`nix develop --offline` before
`cargo test -p jcode-app-core --lib -- r12_ --nocapture`. Exit `0`:
`11 passed; 0 failed; 0 ignored; 1090 filtered out`. Targeted Rust formatting
for `agent/evidence.rs`, `agent/turn_loops.rs`,
`agent/turn_streaming_mpsc.rs`, and `agent_tests.rs` exited `0`. Warning budget
remained `current=0`, `baseline=0`.

The full R09 matrix matched every pre-encoded exit without `--update`:
classifier `0` with 17/17 tests; panic `1` at `31 -> 46`; swallowed-error `1`
at `2987 -> 3074`; production-size `1`, including W1-touched
`turn_loops.rs` `1251 -> 1314` and `turn_streaming_mpsc.rs` `1774 -> 1840`;
test-size `1`, including `agent_tests.rs` `1321 -> 2309`; wildcard `0` at
`16`; warning `0`; shell syntax `0`; diff check `0`. A workspace-wide
`cargo fmt -- --check` attempt reproduced known unrelated formatting drift and
was not used as W1 evidence; no file was changed.

One boundary incident is preserved rather than hidden. The first
post-integration `scripts/dev_cargo.sh` attempt entered the repo dev shell
without explicit Nix offline mode, whose stale fork-nudge hook launched a
background `git fetch --quiet --prune github`. It updated `FETCH_HEAD` and the
fork-nudge timestamp but moved no remote-tracking ref; every `github/*` reflog
entry predates the run and the fetched hashes matched existing refs. No fetch
process remained. The initial test, a malformed auxiliary shell-syntax probe,
and the unrelated workspace-format failure are byte-preserved as deterministic
gzip transcripts under
[`evidence/2026-07-16-w1-integration/`](evidence/2026-07-16-w1-integration/).
Evidence commit: `901f9970ec53b2b5736d78cbb6aac6d44b7155ea`; manifest SHA-256:
`0318017399e36bf2a3b41355f5aeb313d6c59d598034c985154b60dde7658890`.

Preservation checks still show only the user's
`ORCHESTRATOR_PROMPT.md` edit, diff SHA-256
`8e8e6a92dad180b3925bc0b2a3b7b951bc6a6f5c9e4f8a57c9f522d03ad85c00`,
four stashes, W1 worktree head `63309f670ee27e4479ebea3a0867456f36f87e4e`,
and paused clean W2 head `66cc395417eab926f728b5d42ad2241da22d1074`.
No provider, credential, daemon, reload, tool/MCP, publication, installation,
updater, release, or quality-baseline action occurred.

W1 is complete only for the exact deterministic offline R12 cancellation,
retry, strict non-retry, and closed provider/turn error-class boundary. It does
not approve live providers, daemon/reload behavior, tools, generic compaction,
schema changes, or a widened pilot. W3's W1 prerequisite is now satisfied, but
no W3 work is started by this checkpoint. W2 remains unintegrated and blocked
at the recorded R03A wire-governance authorization boundary.

### 2026-07-16T04:31:30Z fork-governance decision amendment

The operator selected the low-friction W2 resolution and supplied W4 product
truth. W2 will retain explicit `Visible` failure, `Auto` fallback, member-detail
and event-history evidence, and churn/reclaim safety while removing the three
new `CommSpawnResponse` fields and their persisted replay copies. Public
response metadata is deferred to a future R03A-governed proposal rather than
being added outside protocol-version governance.

For W4, the fork explicitly rejects synchronization of upstream's expanding
jcode subscription tiers, prices, budgets, and model floors. Current
`Plus`/`Flagship` handling is temporary fail-closed compatibility, not product
endorsement. A future fork-owned seam will separate dynamic model/provider
capability from commercial entitlement after the independent basics work.

The cross-surface naming question exposed by W2 is pinned at
[`docs/proposals/observability-field-naming.md`](../../proposals/observability-field-naming.md).
This checkpoint changes no source behavior and authorizes no live, external,
credentialed, telemetry, payment, publication, or release action.
