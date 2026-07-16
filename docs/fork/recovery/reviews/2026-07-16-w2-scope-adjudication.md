# W2 / R05B Scope & Ownership Adjudication (independent, read-only)

- **Adjudicator role:** verify (adversarial, read-mostly)
- **Repo:** `/Users/jrudnik/labs/jcode-w2-r05b`
- **Branch:** `recovery/fix-r05b-spawn-reclaim-2026-07-15`
- **HEAD adjudicated:** `a342cd5fbe6c0185b486577e59996acc94770b8e`
- **Base authority:** parent `602709895`; `docs/fork/recovery/RECOVERY_PLAN.md`; `docs/fork/recovery/seams/R05B-worker-dispatch-reclaim/ledger.md`
- **Remediation commits in scope:** `2a5beea61` (fix), `6115daa39` (test), `a342cd5fb` (docs)
- **Constraints honored:** read-only; no repo edits; no build/network/live systems; only this file written.

## Top-line verdict

**FAIL (scope/governance) — confidence HIGH.**
Disposition: **current W2 HEAD may NOT proceed to behavioral re-review as an in-scope W2 change; it must PAUSE for R03A protocol-bump governance (or explicit user/coordinator surface-widening authorization).** The change is technically serde-backward-compatible and its constructors/consumers are correct (no correctness defect), so this is not a code-correctness FAIL; it is an unauthorized surface/ownership crossing and a wire change made without the required governance, contrary to the plan's own slice-1 stop condition.

---

## The precise conflict, resolved item by item

### 1. Unauthorized surface/ownership crossing? — YES (HIGH)

- W2 declared **Surface** (`RECOVERY_PLAN.md:100`) is exactly:
  `crates/jcode-app-core/src/server/{comm_control,comm_session,swarm*}.rs`, `crates/jcode-plan` (read-mostly), `tool/communicate.rs` dispatch portions.
  It does **not** include `crates/jcode-protocol`.
- Remediation `2a5beea61` edits `crates/jcode-protocol/src/wire.rs` (adds 3 fields to `ServerEvent::CommSpawnResponse`, `wire.rs:1498-1503`) and widens the durable persistence enum `PersistedSwarmMutationResponse::Spawn` in `crates/jcode-app-core/src/server/swarm_mutation_state.rs:31-36,63-72`.
- Wire schema / compatibility verdict / legacy additivity is **R03A-owned**, not R05B (`RECOVERY_PLAN.md:43`, `:180`; ledger boundary table).
- Verdict: editing `jcode-protocol` under a W2/R05B commit is a surface **and** ownership crossing into R03A, **even though additive and backward-compatible**. Additivity does not confer authority.

### 2. Wire change requiring R03A governance / protocol bump? — YES (HIGH)

- Global gate rule (`RECOVERY_PLAN.md:215`, rule 5): "**no wire change without R03A protocol-bump governance**."
- `ServerEvent` is the serialized wire protocol (`wire.rs:832`, serde `rename` tags). Adding serialized fields to a wire variant is a wire change by definition, regardless of optionality.
- `crate::PROTOCOL_VERSION` remains `1` at both base `602709895` and HEAD (`crates/jcode-protocol/src/lib.rs:26`); no bump, and **no R03A governance record authorizes this change** (grep found no R03A ledger/decision referencing `CommSpawnResponse`/`resolved_spawn_mode`/`spawn_fallback_detail`).
- Verdict: this triggers rule 5 and was performed without the mandated R03A governance.

### 3. Could full response observability have been achieved inside W2 surface? — PARTIALLY; the *response* leg specifically could NOT without protocol change (HIGH)

- Fixture 1 (`ledger.md`, "Required exact fixtures", item 1) requires the Auto fallback to be recorded in **event / detail / response**.
- **Event leg** (in-scope, already satisfied): the swarm event history + broadcast `SwarmEventType::MemberChange{action:"joined"}` for the headless-fallback member existed at base (`602709895:comm_session.rs` headless-fallback block ~L754-800) and remains inside `comm_session.rs`.
- **Detail leg** (in-scope, already satisfied and hardened here): `member.detail = Some(fallback_detail)` is set inside `comm_session.rs`; the remediation's `auto_fallback_status_detail` keeps the fallback text from being overwritten by the initial-prompt "running" status update. All within W2 surface.
- **Response leg**: `CommSpawnResponse` at base carried only `id`, `session_id`, `new_session_id`, `initial_prompt_delivered` — **no free-form/string field** capable of carrying `requested -> resolved` mode or fallback detail. There is **no existing in-surface response/event/detail field** that could carry the *response*-leg observability without editing `jcode-protocol`.
- Therefore: a fully in-scope alternative that satisfies the **response** leg does **not** exist. The event and detail legs alone (both in-scope) already give operator/agent-visible fallback observability; the extra response-leg surfacing is what forced the protocol edit.

### 4. Is the plan internally inconsistent? — NO; the plan pre-anticipated this and prescribed STOP (HIGH)

The plan is **not** internally inconsistent. Slice 1's explicit stop condition (`ledger.md`, "Bounded implementation slices", slice 1) reads:

> "**Stop if API consumers cannot distinguish requested from resolved mode without protocol change; isolate a compatibility proposal.**"

This is exactly the situation reached. The plan foresaw that surfacing requested-vs-resolved mode on the response could need a protocol change and directed the implementer to **stop and isolate a compatibility proposal**, i.e. route it through R03A governance — not to implement it inside W2. Fixture 1's "response" wording and the protocol-freeze rule are reconciled by this stop clause.

**Smallest explicit authorization needed:** an **R03A protocol-bump governance decision** (a compatibility proposal reviewed/approved by R03A, i.e. the "isolate a compatibility proposal" the slice demands), or an equivalent explicit user/coordinator authorization to widen W2's surface to include `crates/jcode-protocol` under R03A governance. Absent that, the response-leg change must not ship under W2.

### 5. Independent serde / backward-compat and correctness assessment — technically compatible; constructors/consumers correct (MEDIUM-HIGH)

- **Field attributes:** all three added fields on both `ServerEvent::CommSpawnResponse` (`wire.rs:1498-1503`) and `PersistedSwarmMutationResponse::Spawn` (`swarm_mutation_state.rs:31-36`) use `#[serde(default, skip_serializing_if = "Option::is_none")]`. `initial_prompt_delivered` keeps `#[serde(default)]`.
- **No `deny_unknown_fields`** on `ServerEvent` (`wire.rs:832`) or `PersistedSwarmMutationResponse` (`swarm_mutation_state.rs:16`). So: old payloads missing the fields deserialize to `None` (default); newer payloads with the fields are tolerated by older binaries (unknown fields ignored). Bidirectionally serde-compatible.
- **Constructors (exhaustive):** only two production constructors of `CommSpawnResponse` exist — `swarm_mutation_state.rs:66` (`into_server_event`) and, upstream of it, `comm_session.rs:1072` building `PersistedSwarmMutationResponse::Spawn` from `SwarmSpawnOutcome`. Both are updated consistently. `spawn_swarm_agent` return type changed `Result<String>` -> `Result<SwarmSpawnOutcome>`; its one non-test caller, `handle_comm_assign_next` (`comm_control.rs:2065`), was updated to `spawned_session.new_session_id`. No other production constructor exists.
- **Consumers:** `tui .../server_events.rs:2842` and `communicate.rs:1654` use `{ new_session_id, .. }` rest patterns (unaffected); `communicate.rs:2833` explicitly binds and renders the new fields. All remain correct.
- **Persistence/replay:** `PersistedSwarmMutationResponse` is durably written to disk (`save_json_state` -> `jcode-swarm-mutations/{key}.json`, `durable_state.rs:63-71`) and replayed via `begin_or_replay`/`into_server_event`. Old on-disk records lacking the fields load as `None`; stale records are TTL-cleaned. Functionally sound.
- **Caveat (severity: important):** this is nonetheless a **durable on-disk serialized schema widening** of the mutation-dedup/replay cache, and a wire-visible change while `PROTOCOL_VERSION` stays pinned at `1`. That is precisely the version/compatibility token R03A governs. Compatible-in-practice is not the same as authorized. The ledger's claim "No durable task-progress schema widening was introduced" is true only for *task-progress*; the *mutation replay cache* schema was in fact widened.

### 6. May current W2 HEAD proceed? — NO; PAUSE for R03A/user approval (HIGH)

- It may **not** proceed to behavioral re-review as an in-scope W2 change.
- Two acceptable remediations:
  1. **Preferred / minimal:** revert the `jcode-protocol` + `PersistedSwarmMutationResponse` widening and re-land the fallback observability using only the in-scope **event** + **detail** legs (both already implemented in `comm_session.rs`), then either (a) reinterpret fixture 1's "response" leg as satisfied by event+detail and record that decision, or (b) hold the response-leg surfacing as an isolated R03A compatibility proposal per slice-1's stop clause.
  2. **Otherwise:** obtain explicit **R03A protocol-bump governance** (or user/coordinator surface-widening authorization) before this HEAD advances; then the wire fields may remain.

---

## Severity findings

| # | Severity | Finding | Evidence |
|---|---|---|---|
| F1 | HIGH | Wire change to R03A-owned `jcode-protocol` shipped under a W2/R05B commit without R03A protocol-bump governance; violates gate rule 5. | `wire.rs:1498-1503`; `RECOVERY_PLAN.md:215`; `:43`,`:180`; no R03A record (grep) |
| F2 | HIGH | Surface crossing: `crates/jcode-protocol` is outside W2's declared surface. | `RECOVERY_PLAN.md:100`; `2a5beea61` touches `wire.rs` |
| F3 | HIGH | Slice-1 stop condition ("isolate a compatibility proposal" on needing a protocol change) was not honored; work continued instead of pausing. | `ledger.md` slice 1 stop; `2a5beea61` |
| F4 | IMPORTANT | Durable on-disk schema of the swarm mutation replay cache widened while `PROTOCOL_VERSION` stays `1`; ledger's "no durable schema widening" claim is scoped only to task-progress. | `swarm_mutation_state.rs:31-36`; `durable_state.rs:63-71`; `lib.rs:26`; `ledger.md` amendment |
| F5 | LOW (mitigant) | Added fields are serde-additive/backward-compatible; all constructors and consumers correct; no code-correctness defect found. | serde attrs; constructor/consumer census above |

## Exact evidence / commands used

- `git log --oneline -12`; `git rev-parse HEAD` -> `a342cd5fb…`
- `git show --stat 2a5beea61 6115daa39 a342cd5fb`
- `git show 2a5beea61 -- crates/jcode-protocol/src/wire.rs` (the 6 added lines)
- `git show 2a5beea61 -- .../comm_session.rs .../comm_control.rs .../tool/communicate.rs .../swarm_mutation_state.rs .../comm_session_tests.rs`
- `sed -n '1492,1504p' crates/jcode-protocol/src/wire.rs`
- `grep -rn 'CommSpawnResponse' crates/` (8 hits; constructor/consumer census)
- `grep -n 'PROTOCOL_VERSION' crates/jcode-protocol/src/lib.rs` -> `= 1`; `git show 602709895:.../lib.rs | grep PROTOCOL_VERSION` -> `= 1`
- `grep -rn 'deny_unknown_fields' wire.rs swarm_mutation_state.rs` -> none
- `RECOVERY_PLAN.md` lines 97-104 (W2), 215 (gate rules), 43/180 (R03A ownership)
- `ledger.md` fixture 1, slice 1 stop, 2026-07-16 remediation amendment
- `durable_state.rs:41-71` (on-disk persistence of mutation state)

## What was NOT checked (limits)

- **No build/test execution** (offline/no-network/no-live constraint): serde compatibility and constructor/consumer correctness were assessed by static source inspection only, not by `cargo test`. The ledger's cited green test runs were not independently re-executed.
- **No live daemon/replay** exercised; on-disk backward-compat reasoning is static.
- Did not adjudicate HIGH-gap-2 (churn-to-abort/residue) beyond confirming it does not touch `jcode-protocol`; this report is scoped to the protocol/surface conflict only.
- Did not evaluate whether R03A would in fact approve the field (that is R03A's governance call, not this adjudication's).
- R09/panic/size budget deltas were read from the ledger, not re-run.

## Confidence

- Items 1, 2, 4, 6 and F1-F3: **HIGH** (direct plan text + direct diff).
- Item 3: **HIGH** for "no in-scope response-leg field exists"; the reinterpretation option is a judgment call.
- Item 5 / F5: **MEDIUM-HIGH** (static-only, no compile/test run).

## Report file hash

SHA-256 of this file (computed after final content, so excludes this trailing line): `4cb002602b843810f780180b134c5bde4c061759c74764d730e0fad3feb6b6cd`
