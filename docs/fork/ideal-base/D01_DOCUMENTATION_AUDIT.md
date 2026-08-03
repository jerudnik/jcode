# D01 general documentation audit input

Recorded: 2026-07-27

Status: `re_audited` (D01-A re-audit 2026-08-02; see disposition table below)

This document is the source-backed input register for D01. It records defects
found before the distribution handoff so they survive into the ideal-base
pipeline without expanding the active distribution change. It is not authority
to start D01 early or to edit a path owned by another worker.

At activation, D01-A must re-audit every item against the merged `main` tree.
Each item must then be marked confirmed, superseded by the merged implementation,
or converted into a separately owned product repair. The current source, tests,
CLI help, workflows, flake outputs, and deterministic behavior remain more
authoritative than this snapshot.

## Execution boundary

- Keep this register as research input while distribution and W4 owners are
  active.
- Start D01 only after the dependencies in
  [`POST_DISTRIBUTION_ORCHESTRATOR_PLAN.md`](POST_DISTRIBUTION_ORCHESTRATOR_PLAN.md)
  are accepted.
- D01-A owns the resnapshot and exact path manifest.
- D01-B through D01-F may edit in parallel only after D01-A assigns every
  document to one owner.
- D01-V starts after all editing lanes and must disposition every finding below.
- A source defect discovered here becomes its own repair node and pull request;
  D01 documents only the accepted result.

## Confirmed finding register

| ID | Severity | Lane | Finding and source evidence | Required disposition |
| --- | --- | --- | --- | --- |
| `D01-F01` | high | D01-B | Provider login runs live post-login validation unless `--no-validate` is supplied (`src/cli/login.rs:353-364`). The validation enables both basic and tool smoke requests (`src/cli/auth_test/run.rs:140-159`). `OAUTH.md:228-248` describes real `auth-test` requests but does not disclose automatic post-login validation, retry/spend risk, or the opt-out. | Document the default live behavior, distinguish network and spending paths, explain `--no-validate`, `--no-smoke`, and `--no-tool-smoke`, and ensure no example silently authorizes provider use. |
| `D01-F02` | high | D01-B | `OAUTH.md:282-287` names `OPENAI_API_KEY` and `MiniMax-M2.7`; canonical metadata names `MINIMAX_API_KEY` and `MiniMax-M3` (`crates/jcode-provider-metadata/src/catalog.rs:334-343`). | Correct the preset metadata and anchor the claim to the canonical provider catalog. |
| `D01-F03` | high | D01-C | `docs/MEMORY_BUDGET.md:68-70` states Mermaid limits of 64, 12, and 8. The implemented render, image-state, and source-cache limits are 512, 24, and 16 (`crates/jcode-tui-mermaid/src/mermaid_cache_render.rs:12`, `crates/jcode-tui-mermaid/src/lib.rs:487,648`). | Correct the active guardrail values, update their crate-split source paths, and verify the debug surfaces expose the same limits. |
| `D01-F04` | medium | D01-C | `docs/MEMORY_ARCHITECTURE.md:3` calls graph support planned while `docs/MEMORY_ARCHITECTURE.md:723-728` calls it implemented. Its petgraph and storage examples at lines 104-133 and 670-684 do not describe the implemented HashMap graph (`crates/jcode-memory-types/src/graph.rs:229-256`). | Separate current behavior from roadmap material and describe the actual graph API and storage layout. |
| `D01-F05` | medium | D01-C/D01-E | `docs/AMBIENT_MODE.md:3,922-962` and `docs/SAFETY_SYSTEM.md:3,509-539` remain design documents with unchecked implementation phases despite live ambient configuration/runtime and substantial safety persistence, notification, and channel code. Some advertised safety review commands remain unimplemented. | Record an explicit implemented/planned matrix. Do not mark proposed CLI, TUI review, custom-rule, webhook, or SMS behavior as current. D01-A must assign each file to exactly one of D01-C or D01-E before edits. |
| `D01-F06` | medium | D01-D | `docs/PROVIDER_DOCTOR.md:12-13` says the command works with OpenAI-compatible providers, while native provider drivers are selected in `src/cli/provider_doctor.rs:23-45`. | Describe native and OpenAI-compatible coverage without hard-coding a list that can drift from provider metadata. |
| `D01-F07` | medium | D01-F | `docs/CLOUDFLARE_EXPERIMENT_STRATEGY.md:4` calls `distro/nix` the current packaging rail. `docs/fork/SYNC_MODEL.md:11-22` defines the independent single-rail model. | Archive or explicitly reclassify the strategy and remove its claim of current branch authority. |
| `D01-F08` | medium | D01-F | Ten non-archive Markdown links in the pre-handoff snapshot target `~/notes/...`, including server, interaction, modular architecture, desktop, and provider-session documents. These targets are unavailable to repository consumers. | Recount after the distribution merge. Import contract-critical material into the repository; otherwise label the reference as private project-management context rather than a durable dependency. |
| `D01-F09` | medium | D01-A/D01-V | The documentation tree has no `docs/README.md` authority map. Root `README.md:718-730` links only a small unclassified subset. `scripts/check_agent_instructions.py:168-172` validates links only in `docs/agent-workflows.md`, and `scripts/ideal_base_railway.py:257-274` is intentionally scoped to ideal-base control documents. | Add the classified docs map and one deterministic general-doc checker for local links, maintained repository paths, and a narrow retired-claim denylist. Keep archives and frozen evidence outside current-policy rules. |
| `D01-F10` | low | D01-B/D01-C | `OAUTH.md:26-34` and `docs/MEMORY_BUDGET.md:25-31` retain source paths from before the crate split. | Replace stale implementation references with current owning crates and files. |
| `D01-F11` | low | D01-B | `TELEMETRY.md:283` cited `src/telemetry.rs`, which does not exist. The implementation is `crates/jcode-telemetry-core/` (5 files, 3811 lines), reached through a `pub mod telemetry` re-export in `jcode-base`. Found while building F09's checker, not in the original census. | Correct the path. Verify the surrounding no-other-telemetry-calls claim rather than assuming only the path is wrong. |
| `D01-F12` | medium | D01-B/D01-V | Documentation cites **545** concrete code paths across live documents and **390** of them do not exist. 306 sit in two dated audit snapshots (`CODE_QUALITY_AUDIT_2026-04-18.md` 274, `PROVIDER_SESSION_SHARED_CONTRACT_AUDIT.md` 32), which are point-in-time records and correctly frozen, leaving **84 real stale citations across 13 files** (worst: `WINDOWS.md` 18, `MODULAR_ARCHITECTURE_RFC.md` 14, `CRATE_OWNERSHIP_BOUNDARIES.md` 13). A further **7** `path:line` references resolve today but are fragile by construction: correct now, silently wrong after any edit above the cited line. F10 and F11 were each one instance of this class, found by hand. Re-derive with `scripts/measure_code_path_drift.py`. | Extend `check_docs_references.py` with a code-path rule ratcheted from 84, excluding dated snapshots. Prefer symbol anchors over `path:line`. **Correction:** I first recorded 1008 cited paths and 102 fragile refs from an unsaved script. Rebuilding the measurement as a committed script gives 545 and 7, and three attempts to reproduce 1008/102 (backticked, bare, and wider path patterns) produced 545, 847, 3451, and 636 - never 1008. The stale figures (84, 306, 13 files) reproduced exactly, so the defect is in the original denominator, not the finding. The unreproducible numbers are withdrawn; the script is now the source. IWE cannot catch this class: it is a document-graph tool and never reads code. |
| `D01-F13` | high | product (not D01) | **Product defect, found while documenting F05.** The safety action classifier is inert. `SafetySystem::classify` and the `AUTO_ALLOWED` tier table (`crates/jcode-base/src/safety.rs:132,177`) are complete, but `ActionTier` appears in **no file outside `safety.rs` and its tests**: tool dispatch gates on `SessionToolPolicy` (`crates/jcode-app-core/src/tool/mod.rs:568`) instead. The documented tier 1/2/3 gate therefore does not run, and an unattended agent is restricted only by session policy and by voluntarily calling `request_permission`. The review pipeline behind it (queue, reviewers, notifications) is real and works; the trigger is unwired. | Not a documentation repair. `docs/SAFETY_SYSTEM.md` now states plainly that the gate is inert, per the standing rule that prose must not be softened to match a shortfall. Needs a separately owned product node to consult `classify` in the ambient tool path. **Now owned by `D01-FIX-1`**, seeded in WORK_GRAPH.json and STATE.json as `pending` depending on D01, and verified dispatchable: with D01 accepted, `ideal_base_railway.py next` lists it. The claim was re-derived by grep on 2026-08-03 before the node was written, rather than inherited from this row. |
| `D01-F14` | medium | product (not D01) | **Product defect, found while documenting F05.** Three dangling wires make three complete ambient implementations inert. (1) `UsageLog::record` (`ambient/scheduler.rs:26`) has no non-test caller, so `usage.json` is never written and every rate query returns 0. (2) `RateLimitInfo` has no production constructor and no `x-ratelimit` header is parsed anywhere, so both `calculate_interval` call sites (`ambient/runner.rs:686,819`) pass `None` and the adaptive scheduler always returns the max interval. (3) `active_user_sessions` (`ambient/runner.rs:53`) has reads at :176, :585 and :879 but **no writer in the workspace**, so it is permanently 0 and ambient never pauses for an active user. The budget bar is inert downstream of (1) and (2): both producers hardcode `budget_percent: None`. | Not a documentation repair. `docs/AMBIENT_MODE.md` marks each row Inert with evidence. Each fix is a small connection, not a feature build. Needs a separately owned product node. **Now owned by `D01-FIX-2`**, seeded in WORK_GRAPH.json and STATE.json as `pending` depending on D01, and verified dispatchable: with D01 accepted, `ideal_base_railway.py next` lists it. The claim was re-derived by grep on 2026-08-03 before the node was written, rather than inherited from this row. |

## Provisional distribution-owned surfaces

The pre-handoff audit did not treat the following moving files as final:

- `README.md`
- `RELEASING.md`
- `docs/BRANCHING.md`
- `docs/DESKTOP_APP_ARCHITECTURE.md`
- `docs/IOS_APP.md`
- `docs/NIX.md`
- `docs/WINDOWS.md`
- `docs/WRAPPERS.md`
- distribution and release workflow documentation
- the root instruction primitive

D01-A must inspect their merged content and F30 evidence before assigning
corrections. It must not carry pre-handoff line numbers or assumptions forward
without verification.

## Rejected preliminary claims

Do not reopen these claims without new source evidence:

- `JCODE_SERVER_DISPLAY_NAME` is implemented in
  `crates/jcode-app-core/src/server.rs` and covered by server tests.
- No tracked `.DS_Store` was found in the audit snapshot.
- The alleged obsolete render dependencies were not present in current Cargo
  manifests.
- `/resume` picker behavior and the `--resume` CLI option are distinct surfaces;
  the preliminary report conflated them.
- The preliminary scan did not establish a broken repository-relative Markdown
  target. D01-V must still run the deterministic checker on the final tree.

## D01-V acceptance additions

D01-V must produce a final table mapping every `D01-Fxx` item to one of:

- `corrected`, with the accepted commit and source evidence;
- `superseded`, with the merged behavior that invalidated the finding;
- `product_repair`, with the blocking repair node and accepted commit; or
- `not_reproducible`, with deterministic evidence and independent review.

No item may disappear through prose cleanup alone. The final D01 evidence must
also include the classified document census, checker fixtures proving each rule
is non-vacuous, command parser or `--help` evidence, generated-instruction drift
results, and an independent source-to-prose review.

## D01-A re-audit against merged main (2026-08-02)

Every finding above was re-checked against the shipped tree rather than trusted
from the snapshot. **All ten reproduce.** None was superseded by the merged
implementation, so nothing here is stale bookkeeping.

Three were mechanically verifiable against source, so they are corrected now.
Each was fixed by extracting the value from the code and diffing it against the
prose, not by reading and retyping:

| ID | Disposition | Evidence |
| --- | --- | --- |
| `D01-F02` | `corrected` | `OAUTH.md` MiniMax preset said `OPENAI_API_KEY` / `MiniMax-M2.7`; `catalog.rs` `MINIMAX_PROFILE` says `MINIMAX_API_KEY` / `MiniMax-M3`. Checked **all** `OpenAiCompatibleProfile` presets against their `OAUTH.md` sections by parsing both, not just the reported one; MiniMax was the only drift, 2 fields. |
| `D01-F03` | `corrected` | `docs/MEMORY_BUDGET.md` said 64/12/8; source says `RENDER_CACHE_MAX=512`, `IMAGE_STATE_MAX=24`, `SOURCE_CACHE_MAX=16`. Corrected in both the budget table and the summary list, and each `<=` value is now confirmed equal to its `const` by parsing both files. `ACTIVE_DIAGRAMS_MAX=128` and the two disk caps were already accurate. |
| `D01-F10` | `corrected` | Every pre-crate-split path in `OAUTH.md` and `docs/MEMORY_BUDGET.md` was confirmed non-existent, remapped to its current owning crate, and re-checked so that every referenced `.rs` file resolves on disk. |
| `D01-F11` | `corrected` | Path repaired in this pass. The adjacent claim that no other telemetry-related network calls exist was checked rather than assumed: `TELEMETRY_HTTP_CLIENT` is confined to `jcode-telemetry-core/src/lib.rs`, and the other `api.jcode.sh` callers are subscription and config. The claim holds, so it was sharpened, not softened. |

The remaining seven need editorial judgement about what the product should
claim, not a lookup, so they stay open under their assigned lanes:

| ID | Disposition | Re-audit note |
| --- | --- | --- |
| `D01-F01` | `corrected` | `OAUTH.md` documented `auth-test` as something you run deliberately but never disclosed that `jcode login` runs it automatically. Verified: `src/cli/login.rs:364` calls `run_post_login_validation` unless `--no-validate`, and `src/cli/auth_test/run.rs:140-159` passes both smoke flags `true`, so a successful login issues **two** real billable requests - a completion smoke and a tool-enabled smoke that asks the model to make a `bash` call running `echo JCODE_TOOL_OK`. `OAUTH.md` now leads the section with this, names both requests, and states the spend and rate-limit consequence. **Correction made during the fix:** I first wrote that `--no-smoke` and `--no-tool-smoke` narrow the login behavior. They do not; they belong to `auth-test` (`src/cli/args.rs:475,479` sit under `AuthTest`, while `no_validate` at `:205` sits under `Login`). Confirmed by running `jcode login --help`, which lists only `--no-browser`, `--no-selfdev`, `--no-validate`. The document now says post-login validation is all-or-nothing, because it is. |
| `D01-F04` | `corrected` | The file contradicted itself and the header was the wrong half. Verified against `crates/jcode-memory-types/src/graph.rs`: `MemoryGraph` ships with `HashMap` node/edge maps, `EdgeKind` carries all six variants the document lists, and `cascade_retrieve` does a `VecDeque` BFS, so the Phase 4 checklist was accurate and the `Status: Planned (Graph-Based Hybrid)` header was stale. Header now says implemented. Three further petgraph claims were false and are fixed: the Rust example imported `petgraph::graph::DiGraph`, the architecture diagram drew a `petgraph DiGraph` node, and the storage layout listed a `graph.json` 'Serialized petgraph'. petgraph appears in **no** Cargo.toml and **no** Rust source file in the workspace; it is a transitive lockfile entry only. The example now shows the real structs, and the layout is corrected against a live on-disk file whose top-level keys are exactly `graph_version, memories, tags, clusters, edges, metadata`: there is no separate `graph.json`, the graph is serialized into each project file. |
| `D01-F05` | `corrected` | Both files carried `Status: Design` over shipped code, with **50 unchecked boxes and zero checked** between them (30 ambient, 20 safety) despite substantial implementations. Replaced both checklists with verified implementation matrices. Every one of the 50 rows was checked against a cited symbol, and the matrices use four states rather than a checkbox because a checkbox cannot express the most important distinction found: **inert** code, fully implemented with nothing calling it, which reads as working and is not. Ambient: 15 shipped, 5 inert or partial-by-wire, 6 prompt-only (the prompt asks the model; no code does it), 4 absent. Safety: 7 shipped, 1 inert, 5 partial, 7 absent. Two inert clusters were severe enough to file as separate product findings, D01-F13 (the safety tier classifier is not consulted by tool dispatch) and D01-F14 (three dangling wires in ambient). Both matrices also record substantial shipped work the checklists never listed, roughly a third of the ambient surface. Verification note: the per-item sweep was delegated, but every consequential verdict was re-checked directly before publication - `classify` and `log_action` have only test callers, `ActionTier` appears nowhere outside `safety.rs`, `active_user_sessions` has only reads, `budget_percent` is always `None`, both `calculate_interval` callers pass `None`, and no `safety` clap subcommand exists. |
| `D01-F06` | `corrected` | `docs/PROVIDER_DOCTOR.md` claimed OpenAI-compatible providers only, but `src/cli/provider_doctor.rs:26` routes native providers first via `native_doctor_supports_provider` (`crates/jcode-base/src/auth/doctor.rs:18`), which matches claude, antigravity, openai, gemini, cursor, copilot, bedrock, jcode, azure-openai. Both families are now documented with the native/OpenAI-compatible split and the bespoke Claude/Antigravity drivers named. Per the finding's own remedy the document points at the routing function as the authority and tells the reader to run `jcode provider-test-coverage` rather than trusting either list to stay complete. |
| `D01-F07` | `corrected` | Reclassified rather than rewritten. `docs/CLOUDFLARE_EXPERIMENT_STRATEGY.md` is a dated 2026-06-16 experiment record whose Cloudflare findings are still the reason it is kept; only its branch-model framing was false. Verified `distro/nix` exists as **no** branch, local or remote. It now carries `Status: frozen record` and a header naming `docs/fork/SYNC_MODEL.md` as the authority, stating that the branch it called current does not exist and that its branch-model statements must not be followed. Rewriting a dated record to match today would have destroyed the decision history. |
| `D01-F08` | `corrected` | All 25 machine-local references repaired and the ratchet driven to **0**, so the rule is now fatal on first occurrence like the other two. Four classes, handled differently because they failed differently: (a) 13 Markdown links to private planning notes became plain titles labelled `(private planning note, not in this repository)` - the notes do exist locally and all seven were confirmed present, but no other reader can follow the link, so the honest form names the document without pretending it is reachable; (b) 7 hard-coded `/Users/jrudnik/labs/jcode` checkout paths became repo-relative, including a bootstrap `cd` that is now `cd "$(git rev-parse --show-toplevel)"` and therefore works for any reader; (c) 4 in `MCP_BRIDGE_B_FEASIBILITY.md` were placeholdered to `<home>`/`<repo>` - one is captured probe output, so it was redacted rather than rewritten, because editing recorded evidence to look tidy would falsify it, while the other three are a config example a reader copies and must not carry one operator's home; (d) 1 in `CROSS_DEVICE_WORKTREE_SYNC.md` needed two differing paths to make its point about cross-machine hashing, so the sentence now states the macOS/Linux home difference in prose. Verified by control: adding any new machine-local reference now exits 1. |
| `D01-F09` | `corrected` | Both halves now exist. The map: `docs/README.md` classifies every document into four authority tiers (binding contract, current subsystem, proposal, frozen record), with all nine of its counts verified against the tree, and root `README.md:717` now points at it instead of offering an unclassified list. The checker: `scripts/check_docs_references.py`, 23 tests, a mutation control per rule. What is still missing is ENFORCEMENT - the checker is not wired into `.github/workflows/fork-ci.yml` because that is a governance-protected path (Governance Root failed on PR #94), so nothing re-runs it automatically and it prevents nothing today. Stays open until it gates.  **Enforcement landed.** `Quality Guardrails` now runs `check_docs_references.py` plus its 25-test suite, and `docs/**`/`*.md` route into the `scripts` filter so the job cannot skip on a docs-only PR. **Proven by control, not by a green build:** PR #99 planted a broken link and a machine-local path and CI failed with the expected message; PR #100 passes with `137 active documents, 0 machine-local at baseline`. **The prior `authorization_blocked` note was my error, and I re-tested rather than inheriting it:** `protected` means the audit gate must *name* the path (`governance_compare.py:831`), not that the path is immutable; `.github/workflows` has taken 42 commits since `fork-point`. **CI also caught a defect no local run could:** `docs/README.md` linked to `docs/AGENTS.md`, which `apm compile` generates and `.gitignore:25` excludes, so it resolved here and 404s on a clean clone. The checker now asks git, not the filesystem. Writing that test exposed a second bug of my own: comparing a resolved target against an unresolved root made `relative_to` raise on macOS `/var` vs `/private/var`, so the rule silently passed everything. |
| `D01-F12` | `confirmed` | Reproduced by `scripts/measure_code_path_drift.py`: **84** stale code-path citations across **13** live files, plus **306** in two dated audit snapshots that are frozen by nature and not drift. Worst offenders `docs/WINDOWS.md` (18), `docs/MODULAR_ARCHITECTURE_RFC.md` (14), `docs/CRATE_OWNERSHIP_BOUNDARIES.md` (13). Not yet repaired and not yet gated: the measurement script is advisory only. Needs a ratcheted rule in `check_docs_references.py` starting at 84. |
| `D01-F13` | `referred` | Product defect, outside D01's documentation mandate. Verified directly: `ActionTier` appears in no file outside `crates/jcode-base/src/safety.rs` and its tests, and `classify` has only test callers, so the safety tier gate never runs. D01's obligation is discharged by stating this plainly in `docs/SAFETY_SYSTEM.md` rather than softening the prose. The repair needs a product node with its own owner. |
| `D01-F14` | `referred` | Product defect, outside D01's documentation mandate. Verified directly: `UsageLog::record` has no non-test caller, both `calculate_interval` call sites pass `None`, `active_user_sessions` has three reads and no writer, and `budget_percent` is hardcoded `None` at both producers. `docs/AMBIENT_MODE.md` marks each row Inert with evidence. The repair needs a product node with its own owner. |

Note on F03 and F02 as a class: both are a documented constant drifting from its
source constant, which is the same failure mode this program keeps finding in
code. `D01-F09`'s checker is the durable fix; correcting the values by hand only
resets the clock.

## D01-A census against `82277c6df` (2026-08-03)

The re-audit above was recorded against the tree of 2026-08-02. `main` has moved
since, so every finding was re-run against the current tree rather than carried
forward. **All ten hold**: the three `corrected` items are still correct in the
shipped tree, and the seven `confirmed` items still reproduce.

Re-verification of the corrected items, checked against source rather than
re-read as prose:

| ID | Still correct? | Check run now |
| --- | --- | --- |
| `D01-F02` | yes | `OAUTH.md:285,287` says `MINIMAX_API_KEY` / `MiniMax-M3`; `catalog.rs:338` `MINIMAX_PROFILE` agrees. |
| `D01-F03` | yes | `MEMORY_BUDGET.md:69-71` says 512/24/16; `mermaid_cache_render.rs:12` and `lib.rs:487,648` agree. |
| `D01-F10` | yes | Every `.rs` path cited in the two documents still resolves on disk. |

The seven open items each reproduce verbatim: `login.rs:353` still validates by
default, `MEMORY_ARCHITECTURE.md:3` still reads `Implemented (Core), Planned
(Graph-Based Hybrid)` above an all-`[x]` graph phase, `AMBIENT_MODE.md:3` and
`SAFETY_SYSTEM.md:3` still read `Status: Design`, `PROVIDER_DOCTOR.md:12-13`
still hard-codes its provider list, `CLOUDFLARE_EXPERIMENT_STRATEGY.md:4` still
calls `distro/nix` the current packaging rail, and `docs/README.md` still does
not exist.

### Correction to the F08 recount

The 2026-08-02 note recorded **14** `~/notes/...` references "up from the
snapshot's 10" and read that as growth. It is not growth; the two numbers count
different things, so the comparison is invalid. Measured on the current tree,
every variant reported so the denominator is unambiguous:

| Counting rule | Count |
| --- | --- |
| Markdown links `](~/notes/`, non-archive, excluding ideal-base evidence | **10** |
| Same, including `docs/archive/` | 11 |
| Any mention of `~/notes/`, non-archive, excluding ideal-base evidence | **14** |
| Any mention, non-archive, including ideal-base evidence | 18 |
| Any mention anywhere | 26 |

`10` and `14` are the same tree under two rules: the extra four are prose or
backticked references rather than links (`INTERACTION_SURFACES.md:136`,
`proposals/swarm-lifecycle-remediation.md:3`, `REFACTORING.md:7-8`), and
`10 + 4 = 14` exactly. The link count has not changed since the snapshot.

F08's disposition is therefore **unchanged in scope but corrected in framing**:
the finding is that these targets are unavailable to repository consumers, which
is true of all 14 regardless of syntax. All seven distinct targets exist on this
machine and none is in the repository, so link-vs-prose does not change what a
consumer can reach. D01-B/D01-F must fix all 14, and the durable rule belongs in
F09's checker, which must match any `~/notes/` reference and not only link
syntax.

### Lane sizing for D01-A

| Surface | Files |
| --- | --- |
| `docs/*.md` (top level) | 56 |
| `docs/architecture/**` | 22 |
| `docs/proposals/**` | 17 |
| `docs/archive/**` (classification only, not policy) | 11 |
| All tracked `*.md` in the repository | 446 |

D01's 66 owned paths resolve cleanly: 59 exist on disk and the other 7 are globs
or artifacts the node is meant to create (`docs/architecture/**`,
`docs/proposals/**`, `docs/README.md`, `.apm/instructions/**`,
`scripts/check_docs*.py`, `tests/test_check_docs*.py`,
`docs/fork/ideal-base/evidence/D01/**`). No owned path has gone missing.

`D01-F09` is the largest item and the only one that prevents recurrence: the
other nine are individual drifts, and three of them (F02, F03, F10) have already
been hand-corrected once. Nothing currently re-checks them, so they can drift
again silently.
