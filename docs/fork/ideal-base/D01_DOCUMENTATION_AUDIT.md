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
| `D01-F01` | `confirmed` | `src/cli/login.rs:353` still runs post-login validation by default. `OAUTH.md` mentions the opt-out flags only twice and still does not disclose the default network/spend behavior. |
| `D01-F04` | `confirmed` | `docs/MEMORY_ARCHITECTURE.md:3` still says "Planned (Graph-Based Hybrid)" while its Phase 4 section marks every graph item `[x]`. Self-contradictory in one file. |
| `D01-F05` | `confirmed` | `docs/AMBIENT_MODE.md:3` and `docs/SAFETY_SYSTEM.md:3` both still say `Status: Design` over shipped code. Needs an implemented/planned matrix and a D01-A lane assignment. |
| `D01-F06` | `confirmed` | `docs/PROVIDER_DOCTOR.md:12-13` still hard-codes an OpenAI-compatible list while `provider_doctor.rs` selects native drivers. |
| `D01-F07` | `confirmed` | `docs/CLOUDFLARE_EXPERIMENT_STRATEGY.md:4` still calls `distro/nix` the current packaging rail, contradicting the single-rail model. Needs an archive/reclassify decision. |
| `D01-F08` | `confirmed` | Recounted post-merge as required: **14** non-archive Markdown references to `~/notes/...`, up from the snapshot's 10. Each needs importing or relabelling as private context. |
| `D01-F09` | `partially delivered` | Both halves now exist. The map: `docs/README.md` classifies every document into four authority tiers (binding contract, current subsystem, proposal, frozen record), with all nine of its counts verified against the tree, and root `README.md:717` now points at it instead of offering an unclassified list. The checker: `scripts/check_docs_references.py`, 23 tests, a mutation control per rule. What is still missing is ENFORCEMENT - the checker is not wired into `.github/workflows/fork-ci.yml` because that is a governance-protected path (Governance Root failed on PR #94), so nothing re-runs it automatically and it prevents nothing today. Stays open until it gates. |
| `D01-F12` | `confirmed` | Reproduced by `scripts/measure_code_path_drift.py`: **84** stale code-path citations across **13** live files, plus **306** in two dated audit snapshots that are frozen by nature and not drift. Worst offenders `docs/WINDOWS.md` (18), `docs/MODULAR_ARCHITECTURE_RFC.md` (14), `docs/CRATE_OWNERSHIP_BOUNDARIES.md` (13). Not yet repaired and not yet gated: the measurement script is advisory only. Needs a ratcheted rule in `check_docs_references.py` starting at 84. |

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
