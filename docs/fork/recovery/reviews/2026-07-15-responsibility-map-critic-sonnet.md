# Phase 1 map critic: independent challenge of seed taxonomy R00-R11

Reviewer: independent map critic (read-only), swarm role `verify`
Baseline: fork `d756d6a2c` / `6ca1fcf2e` (recovery HEAD); upstream `802f690982`;
merge base `631935dd1`
Scope checked: `docs/fork/recovery/{BASELINES,PRESCREEN,QUALITY_GATES,
RESPONSIBILITIES,SEAM_LEDGER_TEMPLATE,PROGRESS}.md`; symbol-grep across
`crates/` for reload/selfdev, run-plan/DAG, config hot-path, ambient/backoff,
MCP/consent, TUI surface counts; `.fork.toml`; git log for hotpath stash
subject.
Research budget: ~8 evidence checkpoints, used all 8. Time-boxed, stopping now.
Confidence: medium-high on taxonomy critique, low on anything requiring a
full symbol diff against upstream (not attempted here; that is Mapper/seam
work).

I did not read `/tmp/jcode-recovery-mapper.md` and did not consult any Mapper
conclusion. This is an independent read of Phase 0 evidence plus fresh source
grep.

## Top-line verdict

The seed R00-R11 taxonomy is a reasonable *starting* partition of file
surface, but it conflates at least three distinct kinds of things under one
umbrella ("responsibility"): (a) actual runtime behavioral contracts with
invariants (R01 reload identity, R02 config resolution, R04 supervision), (b)
process/governance overlays that are not owned by any runtime code path at all
(R00 sync governance, R09 quality gates, R11 docs), and (c) a residual bucket
that is really 3-4 unrelated responsibilities glued together by keyword
overlap (R07, and to a lesser extent R08). Treating all twelve as
commensurable "seams" competing for the same six full-review slots is itself
a methodological risk: R00/R09/R11 will always score high on the mechanical
divergence signal (huge doc/script diff volume) without corresponding runtime
risk, which is exactly the "top-six bias from raw path volume" this task asks
me to hunt for.

## Findings, ranked

### 1. R00 is not a responsibility, it is a governance overlay — separate it explicitly

Evidence: `PRESCREEN.md` R00 row is "Fork-only governance machinery is large,
including 199 rerere paths, while the visibility ref remains stale" (215 files,
30 commits, 0 upstream overlap by construction). Independently confirmed:
`find . -path '*.rerere-cache*' | wc -l` → 299 paths today, and `.fork.toml`
exists at repo root (4nix fork-sync/fork-status/fork-doctor consumer) with a
`vendor-branch = "vendor/upstream"` that `BASELINES.md` confirms is pinned to
the merge base, not a live upstream mirror.

R00 has **zero upstream overlap by definition** (F/U/O = 215/0/0) and no
"authority" question in the sense R01-R08 have: nothing in the running binary
executes rerere cache entries or reads `.fort.toml` fields at runtime. Its
"invariant" is procedural (does the next sync attempt correctly reproduce
resolved conflicts, is the visibility ref honest about staleness) not
behavioral. Scoring it 16/16 alongside R01/R02 (which both gate live runtime
correctness) is a category error: R00 governs *how you review*, not *what the
software does*. It should be reclassified as a cross-cutting governance
constraint that every full-review seam must satisfy (state your sync
provenance, don't treat rerere/curated-sync as upstream authority), not a
seventh candidate for a full-review slot. Downgrade to "light,
always-in-scope policy check", freeing a full-review slot.

### 2. R09 has the same shape as R00: necessary precondition, not a peer seam

`PRESCREEN.md` already flags this partially ("truth-gate dependency"), but the
provisional top six still includes it as a full seam. Evidence: Phase 0 (the
P0_truth dependency) already fully adjudicated the two quarantined parsers and
produced a trusted gate truth table (`QUALITY_GATES.md`). What remains open in
R09 is *not* an ownership/authority question between fork and upstream, it is
"which size-ratchet violations are fork debt vs curated-sync debt" — a
bookkeeping/attribution task, already 87-88% attributed to the curated slice
per `QUALITY_GATES.md`'s historical debt attribution table. Running a full
Opus/Grok/Terra adjudication on this is disproportionate: there is no
competing upstream behavior to reconcile, only a debt paydown plan. Recommend
**light** ledger for R09 (define which owning seam pays down which violating
files) rather than full review, freeing a second full-review slot.

### 3. R11 is R00's sibling — same overlay pattern, already downgraded correctly in the pre-screen but worth stating structurally

`PRESCREEN.md` ranks R11 8th at total 10 and already suggests "light
governance review". I concur, and note the mechanism is identical to R00:
116 fork-only files, 0 upstream overlap by construction (docs cannot conflict
with upstream code). Confirms the taxonomy's real axis should be "does this
seam adjudicate a fork-vs-upstream behavioral dispute" (R01-R08, R10) vs "does
this seam certify the review process itself" (R00, R09, R11). The current
index conflates these two axes into one review-depth column.

### 4. R03 "protocol compatibility" is not a standalone seam — it is R01's wire format, and treating it separately risks validating the two halves against different assumptions

Evidence: `crates/jcode-protocol/Cargo.toml` depends directly on
`jcode-selfdev-types` (`jcode-selfdev-types = { path = "../jcode-selfdev-types" }`),
and `crates/jcode-protocol/src/lib.rs:185` defines
`pub type ReloadRecoverySnapshot = jcode_selfdev_types::ReloadRecoveryDirective;`.
`wire.rs`'s `Request::Subscribe` carries a `selfdev: Option<bool>` field
directly (`crates/jcode-protocol/src/wire.rs:207`), and this same field is
threaded through `client_lifecycle.rs`'s 2700-line `handle_client` (12 distinct
`client_selfdev` mutation sites), `client_session.rs`'s `handle_subscribe`/
`handle_resume_session`, and `jcode-tui/src/tui/mod.rs`'s
`subscribe_metadata`/`resolve_subscribe_metadata`. This is exactly the
incident class named in the task ("reload identity" and "protocol"): the wire
protocol *is* the vehicle for runtime/reload identity, not an adjacent
concern. `PRESCREEN.md`'s own explicit-unknown #5 asks "Is protocol/runtime
identity in R03 a standalone seam or part of R01?" — the symbol evidence above
answers this: it should be **merged into R01** as "R01: live runtime identity,
reload authority, and the wire fields that carry it," with R03 narrowed to the
remaining wire concerns that do *not* touch selfdev/build/version identity
(general handshake compatibility, general reconnect semantics unrelated to
build identity, iOS client compatibility). Reviewing R01 and R03 as two
independent full seams risks exactly the seam-pair failure mode named in the
task ("seams that cannot be validated together"): a reviewer could approve
R01's reload authority model and a separate reviewer could approve R03's wire
schema, while the composition of the two (a `Subscribe{selfdev}` message whose
receiver-side mutation is scattered across 12+ sites in one 2700-line function)
is never validated as a unit.

### 5. Circular/mutually-referential dependency: R01 (selfdev tool) is defined inside `jcode-app-core::tool::selfdev`, but R04 (session lifecycle) and R08 (TUI) both directly call into its types (`ReloadContext`, `ReloadRecoveryDirective`) rather than going through a narrow interface

Evidence: `crates/jcode-tui/src/tui/app.rs:19` does
`use crate::tool::selfdev::ReloadContext;` directly from the TUI crate-internal
module path (not an exported API), and `jcode-tui/src/tui/app/remote/
reconnect.rs`, `server_events.rs`, and multiple `tests/remote_events_reload_*`
files reach into the same internal path. `restart_snapshot.rs` in
`jcode-app-core` also directly branches on `session.is_selfdev` to decide which
of two session-launch code paths to use. This means R01's "authority" is not
encapsulated: R04 (session lifecycle/supervision, since `restart_snapshot.rs`
is squarely session-recovery) and R08 (TUI reconnect/remote handling) both
read and, in some code paths, write reload/selfdev state directly. A reviewer
assigned only R04 could bless the session-recovery `is_selfdev` branch without
ever seeing R01's `ReloadContext` semantics, and vice versa. This is the
"conflated authority boundary" and "hidden operational invariant" the task
asks me to find: the *real* invariant ("a session's selfdev/canary flag must
agree between reload-context persistence, restart-snapshot, subscribe wire
field, and telemetry channel labeling") has at least four independent
call sites (`restart_snapshot.rs`, `client_session.rs::handle_subscribe`,
`wire.rs::Request::Subscribe`, `telemetry_state.rs::build_channel`) and no
single owning module. Recommend the R01 ledger explicitly enumerate every
`is_selfdev`/`client_selfdev`/`selfdev_requested` write site as part of its
"owns" boundary, and record R04/R08 as *cross-seam dependents that must be
re-checked*, not independently reviewable in isolation.

### 6. R05 (swarm/DAG) claim of a standalone crate does not match evidence — `jcode-plan` is small and the operational incident lives mostly in `jcode-app-core`, not in the DAG crate itself

Evidence: `crates/jcode-plan` is 5,813 lines across ~9 files including a
`dag/` module (`sim.rs`, `schedule.rs`, `ops.rs`) and a `bridge.rs`/
`mermaid.rs`. The cited run-plan spawn-storm incident
(`bug-run-plan-spawn-storm.md`, summarized in `PRESCREEN.md`) describes
terminal-backed workers dying before prompts, explicit headless spawns
succeeding, and missing failure backoff/spawn-mode authority. Grepping for the
actual spawn code (`spawn_swarm_agent`, `spawn_visible_session_window_with_
context`, `prepare_visible_spawn_session`) shows these live in
`jcode-app-core/src/server/comm_session.rs` (836 lines, `spawn_swarm_agent` at
line 544), not in `jcode-plan`. `jcode-plan` supplies the DAG/scheduling
*data model* (`SwarmExecutionState`, `TaskControlAction`, `is_runnable_status`)
but the *spawn mechanism and failure backoff* that caused the incident is a
`comm_session.rs`/`jcode-app-core::server` concern. This means "R05: swarm,
comm, DAG, and scheduling" as currently scoped bundles a small, coherent,
mostly-pure DAG state machine (testable in isolation, in `jcode-plan`) with a
much larger, stateful, incident-prone spawn/supervision surface
(`comm_session.rs`, `client_lifecycle.rs`) that behaves more like R04
(session lifecycle/supervision) than like graph scheduling. Recommend
splitting R05 into R05a "DAG/task-graph state model" (pure, `jcode-plan`
crate, low operational risk, light review) and folding the spawn-storm
incident surface into R04 as "swarm agent spawn supervision" (high
operational risk, full review, this is where the actual incident evidence
lives). This directly targets the "run-plan spawn storms" and "supervision"
stress areas named in the task: as currently drawn, neither R04 nor R05 alone
owns the spawn-storm failure mode end to end.

### 7. R02 config hot-path claim needs a narrower "owns" than "configuration, auth, providers, and routing" — the stash evidence names a specific hot path, not the whole config surface

Evidence: `git log --oneline --all --grep=hotpath` surfaces
`1f54abc9f On main: WIP fix-config-hotpath-spam part 3 (scorpion):
account_failover hot path`, matching the three preserved stashes recorded in
`BASELINES.md` (`account_failover hot path`, `Config::load->config() TUI
callers`, `config warn-once + sidecar log dedup`). `grep -rl "Config::load"
crates` shows the read sites concentrated in `jcode-base/src/config.rs`
(`pub fn config()` at line 265, 914 lines total),
`jcode-base/src/mcp/{manager,pool,protocol_tests}.rs`, `provider_catalog.rs`,
and several TUI call sites
(`tui/app/tests/{remote_startup_input_02,commands_accounts_01,
state_model_poke_02,commands_accounts_02}` and
`tui/app/inline_interactive/helpers.rs`). The maintenance note (audit-2026-07-
14) explicitly warns these stashes "overlap fixes already integrated" — Phase
0 confirmed via ancestry that `fix-config-hotpath-spam`/`fix-marker-sweep` are
not open work. So R02's genuinely open, high-risk surface is narrower than
"configuration, auth, providers, and routing" as a whole: it is specifically
(a) how often `config()`/`Config::load` gets re-invoked on TUI hot paths, (b)
account-failover's config re-read behavior, and (c) sidecar log dedup. The
rest of R02 (auth token flows, provider catalog entries, model
selection/routing tables) is comparatively low-incident. Recommend the R02
ledger split its "owns" section explicitly into "config hot-path re-read
behavior" (full review, incident-backed) vs "provider/auth/routing catalog
entries" (still full review given the 90/58/55 two-sided divergence, but
tracked as a separable sub-scope so the hot-path fix doesn't get lost inside a
much larger provider-catalog diff review).

### 8. R07 is at least three responsibilities wearing one row, and R08 is likely four — both should be split *before* any full-review slot is spent on them as single rows

Evidence for R07: `jcode-base/src/mcp` alone is 3,186 lines across many files
(`manager.rs`, `pool.rs`, `protocol.rs`, `protocol_tests.rs`); tool execution
(`jcode-app-core/src/tool/mod.rs`, 1,096 lines) is a separate registry
concern from MCP lifecycle; telemetry lives in its own crate
(`jcode-telemetry-core`, 4 files, clearly separable); network/consent policy
touches `external_auth.rs` and the individual `auth/{gemini,google,claude,
copilot}.rs` files plus onboarding-flow consent UI in the TUI crate. These are
four different ownership questions (does the tool registry decide what an
agent may call; does the MCP manager decide what a server exposes and how it
recovers; does telemetry decide what gets reported; does auth/consent decide
what network access is permitted) with different stakeholders and different
incident profiles. `PRESCREEN.md`'s own explicit-unknown #4 already asks this;
I confirm via file/line evidence that the split is warranted, not merely
plausible.

Evidence for R08: 242 `.rs` files under `crates/jcode-tui` versus 13 across
the three already-split TUI helper crates
(`jcode-tui-session-picker`, `jcode-tui-render`, `jcode-tui-account-picker`),
plus 83 files under `jcode-desktop` that overlap TUI concepts (reload,
session_launch, single_session/tool_lines) but are a distinct binary target.
`PRESCREEN.md` already flags R08 as "too broad to review coherently without
Mapper splits" (rank 8, pilot dependency only 1). I concur and add: the
desktop crate should not silently ride along inside "R08 TUI and CLI" — desktop
has its own reload/session-launch duplication (`desktop_reload.rs`,
`session_launch.rs`) that partially re-implements R01/R04 concerns for a
second binary target, which is itself a finding (duplicated reload-identity
logic across TUI and desktop binaries is a maintenance and correctness risk
the current taxonomy doesn't surface because R08's "key surfaces" line
mentions "desktop" as one bullet among five).

### 9. Hidden operational invariant not named anywhere in the taxonomy: "reload/build identity must stay consistent across four independent persistence points" — no single R0x row owns this

This restates and generalizes finding 5. The four points are: (1) filesystem
build-manifest/symlink state (`jcode-build-support/src/paths.rs`,
`client_update_candidate`, `shared_server_update_candidate`,
`preferred_reload_candidate`, `nix_managed_launcher_override`), (2) the daemon's
in-memory `ReloadSignal`/`reload_state.rs` (`prefer_selfdev_binary` field), (3)
the wire-level `Subscribe{selfdev}` flag and its per-connection
`client_selfdev` mutable, and (4) the session-level `RestartSnapshotSession
{is_selfdev}` persisted to disk for crash recovery. The maintenance incident
(`bug-server-reload-stale-daemon-version-check.md`, summarized in
`PRESCREEN.md`: "reload reported already newest while the daemon still mapped
an old executable") is a direct symptom of these four sources of truth
disagreeing. No seed row's "Must preserve" column (they're all still `pending`)
currently names this cross-cutting invariant. Recommend it become an explicit,
named invariant in the R01 ledger's "Owns"/"Must preserve" section, with R03,
R04, and R08 listed as cross-seam dependents that must each demonstrate they
read a single canonical source rather than re-deriving the flag locally.

## Alternative or corrected concise responsibility table

| ID | Responsibility (revised) | Owns | Excludes | Review |
|---|---|---|---|---|
| R01 | Live runtime identity, reload authority, and its wire carriage | build hash/manifest/symlink state, daemon reload signal, `Subscribe{selfdev}` field semantics, `RestartSnapshotSession.is_selfdev`, the single cross-seam invariant that all four agree | general handshake/version-skew logic unrelated to build identity (stays R03) | full |
| R02a | Config hot-path re-read and dedup behavior | `config()`/`Config::load` call frequency on TUI/agent hot paths, account-failover re-read, sidecar log dedup (the three preserved stashes' exact scope) | provider catalog contents, auth token flows | full |
| R02b | Provider/auth/routing catalog and credential resolution | provider selection, model routing tables, credential/auth flows, sidecar processes | hot-path re-read cadence (R02a) | full |
| R03 | Client/server protocol compatibility minus build identity | wire types, handshake version-skew policy, reconnect semantics, iOS client compatibility | build/reload identity fields (owned by R01, merely carried on the wire) | full or light depending on Mapper's symbol diff against upstream |
| R04 | Session lifecycle, supervision, and swarm-agent spawn/backoff | create/resume/cancel/shutdown/recovery, `comm_session.rs` spawn mechanism and failure backoff (absorbs the spawn-storm incident surface from R05) | pure DAG/task-graph state (R05a) | full |
| R05a | DAG/task-graph state model | `jcode-plan` crate: node state, status transitions, mermaid/bridge rendering | spawn mechanism, backoff, terminal-vs-headless spawn-mode selection (R04) | light (small, mostly pure, testable in isolation) |
| R06 | Persistence, evidence, memory, and replay | session store, journals, snapshots, backups, provenance | — | light initially per pre-screen, escalate on coupling found |
| R07a | Tool execution registry and authority | which tools an agent may call, tool definition schema/gating | MCP lifecycle, telemetry, consent | light-to-full pending Mapper symbol diff |
| R07b | MCP lifecycle and discovery | `jcode-base/src/mcp` manager/pool/protocol | tool registry, telemetry | full (3,186 lines, real complexity) |
| R07c | Telemetry and reporting | `jcode-telemetry-core` | consent/network policy | light (small, self-contained crate) |
| R07d | Network/auth consent policy | `external_auth.rs`, per-provider `auth/*.rs`, onboarding consent UI | credential storage/routing (R02b) | light-to-full pending overlap with R02b |
| R08a | TUI input/command/render core | `jcode-tui` core loop, keymap, render | session picker, account picker, desktop | full |
| R08b | TUI session/account picker surfaces | already-split crates | core render loop | light (already modularized) |
| R08c | Desktop adaptation, including its own reload/session-launch duplication | `jcode-desktop` | TUI core | full — specifically to compare against R01's canonical reload logic and flag duplication |
| R09 | Quality gates and test debt attribution | which seam pays down which ratchet violation | parser correctness (already adjudicated in Phase 0) | light |
| R10 | Packaging, release, update, and distribution | Nix, wrappers, channels, updater | — | light or defer, per pre-screen |
| R00 | Sync/governance policy overlay | provenance discipline every full seam must apply (don't treat rerere/curated-sync as upstream authority) | not a competing full-review seam | policy constraint applied to all full seams, not a row |
| R11 | Documentation/backlog governance | active docs, stale-instruction cleanup | not a competing full-review seam | light |

This yields six full seams if you count R01, R02a, R02b, R03, R04, R08a as the
top six (R08c strongly tempts a seventh slot because of the reload-duplication
finding — see open questions). R00, R09, R11 are deliberately *not* full-review
rows in this alternative map; they are always-on constraints or light ledgers.

## Smallest safe pilot question and what would block it

Smallest safe pilot: **"Does a `selfdev` reload/subscribe round-trip preserve
identical `is_selfdev`/`client_selfdev` state across all four persistence
points (build-manifest, daemon reload signal, wire `Subscribe` flag,
restart-snapshot) for a single session, before and after a forced reload?"**
This is narrow (one session, one reload cycle), directly exercises the R01
invariant named in finding 9, and is cheap to falsify with an existing test
harness (`jcode-app-core/src/server/util.rs` already has
`selfdev_daemon_reloads_into_fresh_release_after_update` and
`selfdev_pin_is_preserved_when_it_is_the_freshest_build` as a starting point;
`jcode-app-core/src/server/client_session_tests/reload.rs` has
`handle_reload_queues_signal_for_canary_session`).

What would block it:
1. If R01's ledger cannot enumerate all write sites of `is_selfdev`/
   `client_selfdev`/`selfdev_requested` with confidence (the four-way
   duplication in finding 5/9 means a fifth undiscovered site is plausible;
   I did not exhaustively grep every crate, only `jcode-app-core`,
   `jcode-tui`, `jcode-build-support`, `jcode-protocol`, `jcode-desktop`).
2. If the maintenance incident's live daemon still reproduces the stale-build
   symptom today (PRESCREEN.md's explicit unknown #7) — running the pilot
   against a currently-broken invariant would conflate pilot noise with
   real regression.
3. If R09's size-ratchet debt touches the exact files the pilot needs to
   change (would need to confirm the pilot's target files are not among the
   60/31 violating files before treating a size increase there as pilot
   scope-creep vs pre-existing debt).

## Strongest objections to this critic's own map

1. **The R02a/R02b split may be premature.** The three preserved stashes are
   explicitly *not* open work (Phase 0 ancestry-verified), so the "hot-path"
   sub-scope I'm proposing may have zero remaining fork-vs-upstream dispute
   once someone checks whether the integrated fixes already landed under a
   different commit subject. If so, R02a collapses to a light ledger
   ("verify integrated, close") and R02 should stay one row after all.
2. **Folding R03's build-identity field into R01 could under-review general
   protocol compatibility.** `wire.rs` has substantial handshake/reconnect
   logic (`HandshakeCompatibility`, `TaskGraphNodeSpec`) untouched by my
   review; if that logic has its own large fork/upstream divergence
   independent of selfdev, merging R03 into R01 risks starving it of a full
   review slot. The mapper should re-run the mechanical divergence signal
   with the selfdev-tagged hunks excluded from R03's count to see if the
   remainder still justifies "full."
3. **My R04/R05 split assumes `comm_session.rs`'s spawn code is the dominant
   incident surface, but I did not read `jcode-plan/src/dag/sim.rs` or
   `schedule.rs` in full.** If the DAG scheduler itself has a bug that causes
   nodes to never be marked runnable (a pure-logic bug, not a spawn-mechanism
   bug), that would belong in R05a and contradict my claim that R05a is
   low-risk/light. I did not open those files, only counted lines and greped
   struct names — this is an explicit gap.
4. **Downgrading R00 to a policy overlay could hide a real seam.** If the
   4nix fork-sync/fork-status tooling itself has a bug (e.g., misidentifies
   the vendor branch, silently drops conflict resolutions), that is a
   behavioral bug, not merely governance, and deserves its own review. I have
   not audited the 4nix tool's actual behavior, only confirmed `.fork.toml`'s
   existence and schema.
5. **I did not examine the 127/64 unclassified paths from the pre-screen at
   all** (explicit unknown #1). My alternative map could be missing an
   entire responsibility hiding in that unclassified set; six checkpoints
   were spent on symbol-level confirmation of the *existing* taxonomy's
   cracks, none on discovering entirely new candidate seams from the
   unclassified file list.

## What was not checked (explicit gaps)

- Did not diff any fork commit against its corresponding upstream commit at
  the symbol level; all findings here are internal-consistency checks of the
  current fork tree against Phase 0 docs, not fork-vs-upstream semantic
  comparisons (that is the seam teams' job).
- Did not open `jcode-plan/src/dag/{sim,schedule,ops}.rs` contents, only
  counted lines/structs.
- Did not examine the 127 fork-side / 64 upstream-side unclassified paths from
  `PRESCREEN.md` at all.
- Did not verify whether the three hot-path stashes' exact functions are
  already covered by integrated commits (only confirmed the stash subjects
  and that they are marked non-authoritative in `BASELINES.md`).
- Did not run any test or build; this was a read-only source/doc review.
- Did not examine `crates/jcode-desktop`'s reload duplication in enough depth
  to say whether it is a real behavioral divergence from R01 or a
  necessary, justified adaptation for a second binary target.
- Did not check R06 (persistence/evidence/replay) or R10 (packaging/release)
  in any depth beyond reading the pre-screen row; no independent evidence
  gathered for those two.
