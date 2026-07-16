# Responsibility index

Status: Phase 1 adjudicated on 2026-07-15

This index defines behavior and governance responsibilities, not file ownership. It was adjudicated from fixed refs fork `7ff4fc6be8dcf0410f2f61994752fdf5ee93e6e4`, upstream `802f6909825809e882d9c2d575b7e478dce57d3b`, and merge base `631935dd1d3b2e31e167e2b12ad463e54bcf4b8d`.

The evidence debate is preserved in:

- [`reviews/2026-07-15-responsibility-mapper-luna.md`](./reviews/2026-07-15-responsibility-mapper-luna.md), SHA-256 `2c73d75a34e8daad4e92bb1e307c3c266c8d835fd3042e98e0420921c87055fd`
- [`reviews/2026-07-15-responsibility-map-critic-sonnet.md`](./reviews/2026-07-15-responsibility-map-critic-sonnet.md), SHA-256 `bc215eca527d490b1260b197f845d1d6194cda3424337ea3e886270e5fd5f7c0`
- [`reviews/2026-07-15-responsibility-unclassified-gap-opus.md`](./reviews/2026-07-15-responsibility-unclassified-gap-opus.md), SHA-256 `dfdfba30231388481e15809ad0eb5847ef3cda48309426194dd55d48b57766ad`
- [`reviews/2026-07-15-responsibility-adjudication-final-opus.md`](./reviews/2026-07-15-responsibility-adjudication-final-opus.md), SHA-256 `21fd96c43c9b6c73fac3cb2ab420d5699bf6db0570f7348bfa390f40fee51540`
- [`reviews/2026-07-15-responsibility-adjudication-rereview-opus.md`](./reviews/2026-07-15-responsibility-adjudication-rereview-opus.md), SHA-256 `3b6bb2d13cd06ed4484f78eb43ed7d183003143504f79fde53853c23453e88ba`

`full` requires two independent seam reviews and an adjudication. `light` requires a concise evidence-backed ledger and may escalate. `defer` requires the named trigger before work begins. An `overlay` is mandatory policy applied across relevant seams but does not consume a full runtime seam team.

## Approved map

| ID | Responsibility | Owns and protects | Excludes | Review | Pilot |
|---|---|---|---|---|---|
| R00 | Integration provenance and sync governance | fixed refs, ancestry, curated-sync provenance, equivalence claims, preservation, rollback and stop budgets | runtime behavior and implementation authority | `light overlay` | required |
| R01 | Runtime build identity and reload authority | canonical executable, source fingerprint, build hash, current/published/pending targets, reload-target selection, and the meaning of identity carried elsewhere | wire encoding and compatibility verdicts, client handoff, release publication | `full` | required |
| R02 | Configuration, auth readiness, provider/model entitlement, and routing | layered provenance, credential references, account/provider/model selection, sidecars, route outcome, tier-gated model admission, `/v1/me` tier truth, and offline cached-tier fallback | agent turn execution, wire verdicts, evidence persistence | `full` | required |
| R03A | Wire compatibility and subscribe handshake | stable wire schema, protocol/build compatibility verdict, safe short/full hash comparison, legacy handling, and carriage of R01/R02 identity | source of build or provider truth, transport execution, session recovery | `full` | required |
| R03B | Transport and client attach lifecycle | Unix/WebSocket attach, takeover, disconnect, reconnect mechanics, and idempotent mapping cleanup | compatibility policy and session business state | `light` | conditional |
| R04 | Session, child-process, and background-task lifecycle | create, attach, resume, cancel, shutdown, reload handoff, interruption, detached task adoption, orphan reconciliation, process markers, backoff, liveness, and terminal state | binary selection, DAG truth, worker assignment and reclaim policy | `full` | conditional |
| R05A | Plan, DAG, and control-log semantics | dependency readiness, node transitions, event vocabulary/fold, replay, artifacts, and coordinator control state | process spawn, worker health, render state | `light` | no |
| R05B | Worker dispatch, spawn mode, liveness, reclaim, and failure backoff | assignment dispatch, headless/visible authority, dead-worker detection, bounded reclaim, retry limits, session-growth containment, and observable failure | graph truth and generic child-process implementation | `full` | no, unless swarm-driven |
| R06A | Durable session evidence, journals, snapshots, and replay | evidence schema, history and snapshot persistence, deterministic replay, provenance, partial-write handling, and resume round trips | live turn emission, memory ranking, process supervision | `light` | fixture prerequisite |
| R06B | Memory, backup, and recall policy | memory scope, recall/rerank provenance, backup retention, memory replay, and regression fixtures | transcript correctness and agent turn timing | `defer` | only if memory is exercised |
| R07A | Tool execution and MCP lifecycle/schema authority | tool registry and dispatch, MCP pool, connection cooldown, schema cache hints, per-session handles, and execution-time consent | discovery admission, telemetry, provider routing | `light` | only if tools are exercised |
| R07B | Capability discovery and network/consent policy | discovery admission, sponsored-result disclosure and opt-out, external capability declarations, and consent before network side effects | MCP process lifecycle, telemetry reporting, provider route selection | `defer` | only if discovery/network is exercised |
| R07C | Telemetry, reporting, and analytics consent | reporting scope, channel labels, analytics opt-in/opt-out, and prevention of secret or session-content leakage | discovery ranking and provider usage accounting | `light` | opt-out check required |
| R08A | Operator input and command semantics | CLI parsing, keymaps, interrupt/cancel commands, dangerous-action consent, and command-to-server intent | rendering and backend authority | `light` | smoke only |
| R08B | TUI render state and operator feedback | cards, tiles, progress, status/error visibility, layout stability, and non-mutating presentation | backend truth and input policy | `defer` | no |
| R08C | Session and account selection surfaces | filtering, displayed metadata, resume/create target selection, and picker actions | backend session lifecycle and provider routing | `defer` | no |
| R08D | Desktop, mobile, and platform adapters | platform launch, window/shell adaptation, shared-contract preservation, and platform identity display | shared protocol, core TUI state, release authority | `defer` | no |
| R09 | Quality-gate semantics, debt attribution, and ratchet policy | trusted classifier behavior, current red debt visibility, inherited/fork attribution, CI interpretation, and no blanket baseline updates | behavior remediation and synchronization decisions | `light overlay` | required |
| R10 | Packaging, release, update, and distribution | Nix and package outputs, launchers, channels, updater, release metadata, and reversible acquisition | live daemon target choice and sync governance | `light` | identity smoke only |
| R11 | Documentation, incidents, and backlog governance | active recovery truth, incident hashes, maintenance state, stale-instruction retirement, and append-only decisions | runtime authority and gate verdicts | `light overlay` | required record |
| R12 | Agent turn execution and durable evidence emission | prompt assembly, provider invocation, streaming and tool-continuation ordering, exactly one terminal result, correlated turn/request/response evidence, usage summary, and liveness status | provider selection, evidence storage format, individual tool authority, UI rendering | `full` | required |
| R13 | Compaction and context-budget policy | token/image estimation, thresholds and modes, emergency truncation, 413 recovery, recent-turn preservation, and post-compaction invalidation of provider session/cache/tool state | provider route selection, evidence storage, memory recall | `light` | conditional |

Every row is `mapped`. Disposition remains `undecided` until its ledger distinguishes `adopt-upstream`, `retain-fork`, `compose`, `upstream-patch`, `delete`, or `defer` from reproduced semantic evidence.

## Six full-review seams

The cap is exactly six. Review depth is based on divergence, operational risk, contested authority, protected invariants, and pilot dependency. Governance and quality overlays remain mandatory but do not consume runtime seam slots.

| Rank | ID | Score | Deciding reason |
|---:|---|---:|---|
| 1 | R01 | 16/16 | Concrete stale-daemon incident, four-way identity consistency risk, and pilot-critical executable truth |
| 2 | R02 | 16/16 | Leading two-sided configuration/provider divergence, credential and entitlement risk, and primary pilot route |
| 3 | R12 | 15/16 | Material responsibility omitted by both initial maps, strong fork-dominant divergence, and the producer of the pilot's request/result evidence |
| 4 | R05B | 15/16 | Quantified spawn storm, missing spawn-mode authority, and bounded-liveness requirement |
| 5 | R03A | 14/16 | Near-total two-sided protocol overlap and unsafe consequences from false compatibility or false reconnect |
| 6 | R04 | 13/16 | Session and detached-process supervision spans reload handoff, background loops, marker/recovery behavior, and terminal-state authority |

R00 is a mandatory governance overlay rather than a peer runtime seam. R09 is a mandatory quality overlay because Phase 0 already repaired and independently approved classifier semantics. Remaining debt is attributed and paid down by the behavior seam that owns it. R05A remains light because graph truth is separately testable from the incident-bearing worker lifecycle. R07 and R08 were split before assigning depth and have explicit escalation triggers.

## Cross-seam invariants

1. **One runtime identity:** manifest and launcher state, daemon reload state, R03A subscribe fields, and R04 restart snapshots must describe the same build/channel identity. R01 owns the meaning. R03A and R04 must not re-derive competing truth.
2. **One provider outcome:** R02's selected account, provider, model, entitlement, and route must equal the identity recorded by R12. No stale ambient configuration may silently substitute another route.
3. **One provider-session invalidation rule:** at least two known paths write or invalidate provider-session identity, including R02 model changes and R13 compaction completion. The R13 ledger must enumerate and classify every writer, including R12 agent-turn and R04 background-task reset sites, then prove agent and persisted session copies cannot diverge.
4. **One terminal turn record:** each R12 provider request has exactly one correlated success or error response. R06A must persist and replay it without loss, duplication, or fabricated completion.
5. **One liveness authority per layer:** R04 owns process/task life and terminal state. R05B owns assignment, reclaim, and retry policy. A dead process cannot cause unbounded assignment or session-file growth, and a reclaim cannot erase history.
6. **Ordered reload recovery:** R01 authorizes a target, R04 interrupts and hands off clients and sessions, then R03A evaluates compatibility on reconnect. No layer may claim success before the next layer's observable passes.
7. **Consent before external effects:** R07A, R07B, and R07C must agree on tool, network, discovery, and reporting consent. Credentials, prompts, session content, and durable evidence must not leak through discovery or telemetry.
8. **Debt follows behavioral ownership:** R09 records policy and attribution. Each behavior seam owns any panic, swallowed-error, production-size, or test-size change it introduces. No seam may use `--update` to hide inherited or new debt.

## Bounded pilot prerequisites

The smallest safe Phase 3 question is:

> On disposable fixed fork and upstream refs, can one fixture-backed, non-secret provider/model route resolve from declared configuration and entitlement provenance, execute one no-tool agent turn, carry canonical build/protocol identity through subscribe, and emit one correlated request/result record while trusted gate verdicts remain unchanged?

Before that pilot:

1. R00 fixes refs, preservation checks, research/time/conflict budgets, rollback, and stop conditions.
2. R01, R02, R03A, and R12 full ledgers pass.
3. R06A round-trips the minimal evidence fixture used by the pilot.
4. R09's classifier tests and trusted green gates pass without `--update`; existing red debt remains visible and attributed.
5. R07C confirms reporting remains disabled for the fixture run and that no secret or session content leaves the disposable environment.
6. R13 enumerates and classifies every writer of provider-session identity across R02, R04, R12, and R13, then proves the short pilot cannot trigger compaction or supplies the joint invalidation check.
7. R04 is not a prerequisite unless the pilot adds reload, resume, cancellation, or a detached/background task. R05B, R06B, R07A/B, and R08 are not prerequisites unless the chosen stack exercises them.

Stop rather than broaden the pilot if it requires real credentials, payment, external publication, the live user daemon, tools/MCP, memory, UI/platform behavior, a quality baseline update, an unowned identity writer, or more conflict/time/semantic rewrite than the R00 ledger permits.

### 2026-07-15 integrated prerequisite amendment

The seven prerequisite rules above are unchanged. Their previously named strict source-fix nodes now count **zero** at coordinator HEAD `6c6a4f2c8c78a7f9a08e39a4356e2ab401370de3`:

- R02 integrated fail-closed tier/auth corrections and product-owned fixtures in `3063fe0fa` through `cb924b3ae`; independent correction reviews are Opus PASS SHA-256 `2e5e3c0e0acc63fd22bade8015fdafb003c7fcfb1d0884088345ed92b25388a2` and Fable PASS SHA-256 `1a5ac839a8ea5a83fda1323427e6688210f7921a30f32dcfbbd8d3d6a513dcf3`.
- R01/R03A integrated exact dirty-source identity projection and fail-before-mutation initial Subscribe handling in `615ab1d9a` through `6c6a4f2c8`; the preserved initial Opus PASS, Fable provider failure, and Grok FAIL remain evidence, followed by correction Opus PASS SHA-256 `f382998ca7fd56dbc302a43a7f234b3189e8d56979b58175fec342393fdd17f2` and Grok PASS SHA-256 `9b265115ace7786b3698e4affeb006463a0b33903f266ccca73f031af77eafc6`.
- The combined validation manifest SHA-256 is `41ece4820891461de774dbc5ab06d8e8a66c00630be62274d00dc1f5a9952291`; the G0 R09 manifest SHA-256 is `eadb5441bfdf5aef353a2356b2f04454a33912924a07c8eb7e207146ba992614`. Exact linkage and intentionally absent commit categories are recorded in [`evidence/README.md`](./evidence/README.md).

This amendment does **not** authorize the pilot. The distinct pilot-authorization gate remains **OPEN, pending independent G2 adversarial adjudication**. Any G2 finding becomes a new named blocker rather than being forced through.

## Adjudicated disagreements

- **R00 and R09:** the critic's category objection is accepted. Both are mandatory overlays, not runtime peers competing for the six full seams.
- **R01 and R03A:** R01 owns canonical build/reload identity. R03A owns wire carriage and compatibility verdict. Their composition is a required joint invariant, not a reason to collapse either authority.
- **R04 and R05B:** code location does not decide responsibility. R04 owns process and detached-task lifecycle. R05B owns worker assignment, spawn-mode choice, reclaim, and retry policy. Their incident path must be validated together.
- **R02 hot paths:** R02 remains one end-to-end route responsibility. Preserved hot-path stashes are evidence, not open work or a separate full seam. Hot-path re-read and warning/log dedup become checks inside the R02 ledger.
- **R07 and R08:** the broad seed rows were rejected. They are split by execution, external policy, reporting, input, rendering, selection, and platform adaptation.
- **Unclassified paths:** the focused gap review found R12 and R13, expanded R02 for subscription-tier gating, and assigned the remaining material clusters. Background/detached work belongs to R04; usage and entitlement data belong to R02 with R08B display; sponsored discovery belongs to R07B; prompt logic belongs to R12; platform/setup surfaces belong to R08D/R10; governance files belong to R00/R11; support types follow the behavior they support.
- **Coordinator score deltas:** R03A moved from the Mapper's 13/16 to 14/16 because the approved pilot directly exercises compatibility composition with R01/R02. R04 moved from 12/16 to 13/16 after the unclassified background-task, orphan-reconciliation, and process-marker surfaces were assigned to its lifecycle authority.
- **Preserved prompt edit:** the final Opus review correctly observed that the user's pre-existing `ORCHESTRATOR_PROMPT.md` diff removes a numbered safety rule. Phase 1 neither adopts nor edits that change. Preservation is explicit, its diff SHA-256 remains `8e8e6a92dad180b3925bc0b2a3b7b951bc6a6f5c9e4f8a57c9f522d03ad85c00`, and changing it remains user-controlled.

## Index editing rules

- Keep detailed evidence, authorship, debate, and disposition in `seams/<ID>-<slug>/ledger.md`.
- No more than two full seam teams may be active at once, even though six seams are approved for full review.
- A light or deferred seam escalates before implementation if its trigger is exercised, a protected invariant fails, or confidence remains low.
- The coordinator alone edits this index during parallel review. Seam teams propose amendments in their ledger.
- Phase 1 approval does not select fork or upstream authority and does not authorize replay, merge, rebase, ratchet update, remediation, or publication.

### 2026-07-15 G2 pilot-authorization amendment

The distinct independent G2 gate is now **PASS** for exactly the smallest safe Phase 3 question stated above. The authoritative byte-exact verdict is [`reviews/2026-07-15-g2-pilot-gate-opus.md`](./reviews/2026-07-15-g2-pilot-gate-opus.md), SHA-256 `abb7b2694abccb0c32385fc552dcc29bf0eba854d439c5c43dc82ba4f3991e4f`.

The authorization is conditional on G3 preserving the exact boundary and seven required observations in that verdict. Any need for real credentials, network egress, a live daemon, reload, tools/MCP, memory, discovery, cancellation, retry, compaction, disconnect/takeover, generic-client identity, publication/install/update, an unowned identity writer, or a quality-baseline update stops the pilot rather than widening it.

### 2026-07-15 G4 execution amendment

The authorized cross-seam composition executed successfully at source HEAD `505cd86726f86dc0eedaf3998afae6ed83290d5d`: R02 supplied live Plus fixture truth and symbolic subscription admission; R01/R03A supplied distinct runtime projections and a compatible typed Subscribe verdict; R12 supplied one correlated request/response/terminal evidence spine and truncated-tail replay; R06A supplied disposable storage; R07C telemetry remained disabled; R13 compaction was not exercised. The full result and hashes are in [`G4_RESULT.md`](./G4_RESULT.md).

This observation does not transfer ownership between seams and does not approve adjacent behaviors. The pilot exercised no live provider, real credential, network, daemon, reload, tool/MCP, memory, publication/install/update, cancellation, retry, compaction, disconnect/takeover, generic-client identity, or baseline update. Independent review of the fixed G4 evidence/status commit remains the next gate.

### 2026-07-15 G5 independent-review amendment

Independent Anthropic Opus review of the fixed G4 package returned **PASS** with no blocking findings. The review is [`reviews/2026-07-15-g5-g4-evidence-opus.md`](./reviews/2026-07-15-g5-g4-evidence-opus.md), SHA-256 `37f094d26b196612f2171de98d52238abb72bb8b69d59b149e7bb00999db86d3`. It confirmed that the observed composition respects existing R02, R01/R03A, R12, R06A, R07C, and R13 ownership boundaries without transferring authority or approving excluded adjacent behavior.

### 2026-07-15 Phase 4 architecture-plan amendment

The coordinator-audited cross-seam plan is [`RECOVERY_PLAN.md`](./RECOVERY_PLAN.md). It accounts for all seventeen non-deferred responsibilities and resolves the integrated disposition arithmetic as fourteen `retain-fork`, two `compose`, and one `defer`. The initial Fable omission of R04 and retry-cardinality overstatement are preserved in the initial artifact and corrected in the v2 artifact. Responsibility ownership remains unchanged; implementation is pending independent fixed-plan review.

### 2026-07-15 architecture-gate PASS amendment

Independent Anthropic Opus review approved the fixed cross-seam plan with no blocking findings. The review is [`reviews/2026-07-15-phase4-plan-opus-review.md`](./reviews/2026-07-15-phase4-plan-opus-review.md), SHA-256 `3f2d31cb5fb9ead893ed8b1e4ce451072757cc5d0206236833dac1b3a886fe92`. Existing ownership and dispositions remain intact; W0 record closure is the only immediate authorized workstream.

### 2026-07-15 W0 record-consistency amendment

All integrated light-ledger approval states and R00/R09/R11 Phase 4 review states now have dated superseding amendments. R04's narrow marker/persistence source package is independently approved while lifecycle widening remains blocked under W3. Ownership and dispositions are unchanged; this is docs/evidence closure only.

### 2026-07-16 Phase 6 responsibility-status reconciliation

The initial statement that dispositions were undecided and the W0 statement
that W3 remained blocked are historical plan-time facts. The active recovery
state is:

- all 17 non-deferred responsibilities remain mapped with their original
  authority boundaries;
- the disposition arithmetic remains 14 `retain-fork`, 2 `compose`, and 1
  broader `defer`;
- W1 through W6 are implemented or evidence-closed, independently reviewed,
  serially integrated, and post-integration validated;
- W7 is optional and deferred;
- R08A remains the single broader `defer`, while its named W5 expiry-as-consent
  defect is closed and does not authorize broader UI or credential exercise;
- no Phase 5 protocol change, provider-session writer, or production identity
  writer outside its owner was introduced;
- live provider, credential, network, daemon/reload, swarm, tool/MCP, release,
  installer/updater, signing, publication, and baseline-update paths remain
  outside recovery authority.

The coordinator audit evidence is
[`evidence/2026-07-16-phase6-final-audit/`](./evidence/2026-07-16-phase6-final-audit/),
`SHA256SUMS` SHA-256
`9af58f1563f266066edd6da9208983da62eeb0b1997ec78f9c26318221dcd2a3`.
Final overlay retirement remains pending the required independent reviews and
joint Sol/Fable sign-off.

#### 2026-07-16 spot-check correction

The candidate hash above is preserved as reviewed. After the independent spot
checker distinguished 62 real checks from 76 TSV physical lines, metadata-only
wording was corrected. The current package `SHA256SUMS` SHA-256 is
`ca8ff5b9f3b6c09dc0ff05de9b3c1c426fc2373706eeeca26cad87126f2e14d8`.
No authority, disposition, command, or result changed.

### 2026-07-15 W0 gate PASS amendment

W0 commit `11a78a858` received independent Opus **PASS** with high confidence and no blockers. Review: [`reviews/2026-07-15-w0-opus-review.md`](reviews/2026-07-15-w0-opus-review.md), SHA-256 `bd662db1792edcfed7276aed3203fd173f047daa58747ca8bcbabca290999fd3`. Record-consistency ownership is closed; behavioral ownership and later-workstream authorization are unchanged.

### 2026-07-16 final responsibility-state amendment

Joint Sol/Fable sign-off at fixed head `17586246a` returned PASS with zero
unresolved IMPORTANT or CRITICAL findings. Phase 6 is complete.

R00, R09, and R11 are retired as special recovery overlays. Their responsibilities
do not disappear:

- provenance and preservation return to normal integration ownership;
- trusted-gate operation and visible expected-red debt return to normal quality
  governance;
- append-only decision evidence and active-document consistency return to
  normal documentation governance.

All product authorities remain exactly as mapped above. W7 architecture debt
remains with R12/R04/R05A/R05B owners under the mandatory triggers in
`RECOVERY_PLAN.md` section 17. No authority moved during final closure.
