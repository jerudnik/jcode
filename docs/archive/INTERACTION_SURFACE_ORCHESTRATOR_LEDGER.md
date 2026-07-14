# Interaction Surface Orchestrator Ledger and Launch Prompt

Status: **Archived - superseded.** The work surfaces this run scaffolds largely landed: browser + server surface workspace stores, command palette/verb log, and the rendered smoke harness under `web/jcode-mobile/` and `crates/jcode-base/src/surface_workspace.rs`. Durable design lives in `INTERACTION_SURFACES.md`, `INTERACTION_SURFACE_REQUIREMENTS.md`, and `SURFACE_WORKSPACE_SUBSTRATE_PLAN.md`. Residual WS7 (Kanidm OIDC) is deferred; access policy in `INTERACTION_SURFACE_SECURE_ACCESS.md`.

Original status: next-session orchestration scaffold, 2026-06-30

This document governs the next full interaction-surface implementation session. The next agent should act as an orchestrator over swarms of subagents and must not stop until the strict definition of done is met.

Related docs:

- [`INTERACTION_SURFACES.md`](./INTERACTION_SURFACES.md)
- [`INTERACTION_SURFACE_REQUIREMENTS.md`](./INTERACTION_SURFACE_REQUIREMENTS.md)
- [`SURFACE_WORKSPACE_SUBSTRATE_PLAN.md`](./SURFACE_WORKSPACE_SUBSTRATE_PLAN.md)
- `~/notes/projects/jcode/proposals/mobile-interface/web-mobile-mvp.md`

## Non-negotiable stop condition

The only successful stop condition is: **all seven work surfaces below are implemented, validated, documented, committed, pushed, and operational in the repo.**

Do not stop after a research phase, a partial implementation, a passing subset of tests, or a promising prototype. If an external dependency blocks a surface, continue by building a deterministic local harness, a disabled-by-default integration path, or a safety-gated dry-run until the work surface has a concrete operational path and tests. Ask the user only before irreversible or security-sensitive actions such as public DNS exposure, production auth changes, payments, deleting data, or sending email.

## The seven work surfaces

| ID | Work surface | Goal | Primary artifacts | Required validation |
| --- | --- | --- | --- | --- |
| WS1 | Rendered viewport smoke harness | Commit reusable Chrome CDP rendered checks for Key2, Y700, and laptop. | `scripts/check_web_mobile_rendered.mjs` or equivalent, screenshots/report under ignored temp/output, integrated check docs. | No horizontal overflow, no runtime errors, app renders, command queues, pending command visible/persisted across all three viewports. |
| WS2 | Gateway E2E fixtures | Exercise real or faithful gateway pairing, WebSocket subscribe/history/send/reconnect behavior. | Protocol/gateway fixtures and tests, preferably in existing Rust test layout plus web fixture docs. | Pairing succeeds, send/cancel/history work, disconnect/reconnect/resubscribe is deterministic, stale ack safety is proven. |
| WS3 | Typed command palette and verb log | Add text-first typed verbs with a recoverable command/operation log. | Web UI palette, parser, verb registry, command log persistence, tests. | Keyboard/touch usable, text fallback for each rich action, invalid verbs are safe, pending/acked/failed states survive reload. |
| WS4 | Browser-local surface workspace store | Implement the P0 object graph locally for cards, docs, annotations, intents, artifact refs, views, bodies, snapshots, and ops. | `surface_store` JS module(s), fixtures, tests, docs. | CRUD, operation append/replay, snapshot compaction, body persistence, recovery from corrupt localStorage, 500 card and 1,000 annotation fixture performance. |
| WS5 | Board/docs/annotations/intents/artifact review UI | Render usable projections from the one object graph. | Web views and CSS for board, docs, annotations, intent inbox, artifact review, meta-agent prompt builder. | Key2 stays text-first, Y700/laptop layouts are responsive, derived views update from shared store, CDP smoke covers core flows. |
| WS6 | Server-local surface workspace store | Lift the same model to `~/.jcode/surface_workspaces/` with Rust types, atomic file storage, backup recovery, and protocol hooks. | Rust crate/module, protocol requests/events, gateway integration, docs. | Atomic JSON writes, JSONL ops, `.bak` recovery, open/apply/get snapshot/export requests, Rust tests, no repo export unless explicit. |
| WS7 | Secure access/auth/mesh operational path | Provide a safe path beyond localhost using existing auth/mesh direction without unsafe public exposure. | Security review notes, config docs, tests/harnesses, Kanidm OIDC + PKCE/mesh implementation if approved or a disabled-by-default safety-gated path. | Local/mesh access works, public exposure is blocked unless review passes, auth failure/re-auth paths are tested, no bespoke refresh layer unless Kanidm is proven unsuitable. |

## Orchestrator operating model

The orchestrator owns final design decisions, repo state, conflict resolution, commits, pushes, and the ledger. Subagents can research, draft patches, test, and review, but they should not push. If concurrent edits risk conflict, the orchestrator serializes implementation or uses separate worktrees.

Recommended swarm roles per surface:

1. **Researcher:** map current docs/code/tests and produce gaps plus risks.
2. **Planner:** convert gaps into the smallest coherent implementation slice with acceptance criteria.
3. **Implementer:** edit code behind the agreed slice.
4. **Tester:** create automated unit, integration, rendered, and failure-mode checks.
5. **Reviewer:** review maintainability, UX, security, and docs. For WS7, include a security-focused review.
6. **Orchestrator:** merge, test, commit, push, update ledger, and decide next recursion.

Use `swarm` channels such as `surface-orchestrator` and `surface-ws<N>`. Each subagent report must include changed files, tests run, remaining risks, and whether it believes its acceptance criteria are met.

## Recursive loop for every work surface

For each `WS<N>`, repeat this loop until the surface passes all acceptance criteria:

```text
research -> plan -> implement -> test -> review -> orchestrate -> recurse if incomplete
```

Detailed rules:

1. **Research**
   - Read relevant existing docs and code.
   - Search for prior implementations and tests.
   - Identify safety, UX, data model, protocol, and test risks.
2. **Plan**
   - Write a small per-surface plan into the Serena ledger.
   - Define exact acceptance criteria and test commands.
   - Decide file ownership to avoid subagent conflicts.
3. **Implement**
   - Keep the zero-build web path where possible.
   - Preserve TUI as the primary coding cockpit.
   - Prefer plain JS/CSS for `web/jcode-mobile` until a dependency is justified.
   - Use Nix for cargo if normal `cargo` is not on PATH.
4. **Test**
   - Add tests before declaring complete.
   - Validate failure modes, not only happy paths.
   - Use rendered browser/CDP checks for layout-affecting changes.
5. **Review**
   - Run at least one subagent review for the surface.
   - Fix review findings or record why they are intentionally deferred.
6. **Orchestrate**
   - Run the agreed validation commands.
   - Commit and push focused changes.
   - Update the ledger, todo list, and user-facing progress.
   - Start the next recursion or the next work surface.

## Heartbeat and pulse protocol

The orchestrator must emit a regular pulse at least every 20 minutes of active work and at every phase boundary. The pulse is a governance action, not a pause.

Every pulse must do all of the following:

1. Read the Serena ledger memory.
2. Check active swarm tasks and blockers.
3. Compare repo state against the current ledger phase.
4. Update the `todo` tool with current statuses.
5. Update the Serena ledger memory with:
   - timestamp,
   - current surface,
   - current recursive phase,
   - active subagents,
   - tests run since last pulse,
   - commits pushed since last pulse,
   - blockers and mitigation,
   - remaining DoD items.
6. Send the user a concise progress update.
7. If running a long command, use background progress output such as `JCODE_PROGRESS` or wait with progress-aware tools.

Suggested pulse text:

```text
Pulse YYYY-MM-DDTHH:MMZ: WS<N> <phase>. Done: <facts>. Running: <agents/tests>. Blockers: <none/item>. Next: <next action>. Stop condition remaining: <count/list>.
```

## Self-destructive Serena ledger memory

The next session should create exactly one temporary Serena memory for this orchestration run and delete it only after the strict definition of done is met.

Memory name pattern:

```text
temp/interaction_surface_orchestrator_ledger_<YYYYMMDD_HHMMSS>
```

Rules:

- Create it at session start with the scaffold below.
- Treat it as the authoritative in-session ledger.
- Update it after every pulse and every subagent report.
- Do not delete it while blocked, interrupted, or partially complete.
- At final completion, write a permanent handoff summary to repo docs or commit history, then call `mcp__serena__delete_memory` for this temporary memory only.

### Ledger scaffold

```markdown
# Temporary Interaction Surface Orchestrator Ledger

self_destruct: true
created_at: <ISO8601>
repo: jcode
branch: <branch>
stop_condition: all 7 work surfaces implemented, validated, documented, committed, pushed, and operational
current_surface: WS1
current_phase: research
last_pulse_at: <ISO8601>

## Global constraints

- TUI remains primary coding cockpit.
- Web path remains zero-build unless explicitly justified.
- Use plain JS/CSS for mobile where possible.
- Mobile browser WebSocket disconnects are normal and must recover safely.
- Persist drafts/intents/pending commands before or while editing.
- Prefer Kanidm OIDC Authorization Code + PKCE and WebAuthn/passkeys for future auth.
- Use mesh/DNS for exposure unless public exposure has a low-risk security review.
- Do not build Backlog/GitHub sync, Milkdown, tldraw, heavy drag/drop, or heavy rich-text editor in this program unless all seven surfaces are otherwise complete and the user explicitly asks.
- Preserve unrelated worktree state, especially `docs/cloudflare-roadmap/`, unless the user explicitly includes it.

## Work surfaces

### WS1 Rendered viewport smoke harness
Status: not_started
Owner agents: []
Acceptance criteria:
- [ ] Script committed and documented.
- [ ] Key2 viewport passes.
- [ ] Y700 viewport passes.
- [ ] Laptop viewport passes.
- [ ] No runtime errors or horizontal overflow.
- [ ] Pending command flow is verified.
Research notes:
Plan:
Implementation notes:
Tests:
Review findings:
Commits:

### WS2 Gateway E2E fixtures
Status: not_started
Owner agents: []
Acceptance criteria:
- [ ] Pairing fixture works.
- [ ] Subscribe/history fixture works.
- [ ] Send/cancel fixture works.
- [ ] Reconnect/resync fixture works.
- [ ] Stale ack safety is tested.
Research notes:
Plan:
Implementation notes:
Tests:
Review findings:
Commits:

### WS3 Typed command palette and verb log
Status: not_started
Owner agents: []
Acceptance criteria:
- [ ] Verb parser and registry exist.
- [ ] Command log persists and recovers.
- [ ] Keyboard and touch paths work.
- [ ] Invalid verbs fail safely.
- [ ] Tests cover pending/acked/failed/reload states.
Research notes:
Plan:
Implementation notes:
Tests:
Review findings:
Commits:

### WS4 Browser-local surface workspace store
Status: not_started
Owner agents: []
Acceptance criteria:
- [ ] Cards/docs/annotations/intents/artifact refs implemented.
- [ ] Views/bodies/snapshots/ops implemented.
- [ ] Operation replay and compaction tested.
- [ ] Corrupt recovery tested.
- [ ] 500 card and 1,000 annotation fixtures pass.
Research notes:
Plan:
Implementation notes:
Tests:
Review findings:
Commits:

### WS5 Board/docs/annotations/intents/artifact review UI
Status: not_started
Owner agents: []
Acceptance criteria:
- [ ] Board projection usable.
- [ ] Docs projection usable.
- [ ] Annotation projection usable.
- [ ] Intent inbox usable.
- [ ] Artifact review and meta-agent prompt builder usable.
- [ ] Responsive CDP smoke passes for Key2/Y700/laptop.
Research notes:
Plan:
Implementation notes:
Tests:
Review findings:
Commits:

### WS6 Server-local surface workspace store
Status: not_started
Owner agents: []
Acceptance criteria:
- [ ] Rust model types exist.
- [ ] `~/.jcode/surface_workspaces/` storage works.
- [ ] Atomic writes and `.bak` recovery tested.
- [ ] JSONL ops tested.
- [ ] Protocol open/apply/get_snapshot/export implemented and tested.
Research notes:
Plan:
Implementation notes:
Tests:
Review findings:
Commits:

### WS7 Secure access/auth/mesh operational path
Status: not_started
Owner agents: []
Acceptance criteria:
- [ ] Security review documented.
- [ ] Mesh/local safe path works.
- [ ] Kanidm OIDC + PKCE/WebAuthn path is implemented if approved, or a disabled-by-default testable integration path exists if not safe to enable.
- [ ] Public exposure is blocked unless low-risk review passes.
- [ ] Auth failure/re-auth paths tested.
Research notes:
Plan:
Implementation notes:
Tests:
Review findings:
Commits:

## Pulse log

- <ISO8601> initialized ledger.

## Global validation ledger

Commands that must pass before final stop:

- [ ] `./scripts/check_web_mobile.sh`
- [ ] rendered viewport smoke script for Key2/Y700/laptop
- [ ] `nix develop --command cargo test -p jcode-protocol protocol_tests --lib`
- [ ] `nix develop --command cargo test -p jcode-base gateway_tests --lib`
- [ ] new surface workspace JS tests
- [ ] new gateway/surface Rust tests
- [ ] `nix develop --command cargo check -p jcode --bin jcode`
- [ ] `nix develop --command scripts/dev_cargo.sh build --profile selfdev -p jcode --bin jcode`
- [ ] `git diff --check`
- [ ] final `git status --short` shows only intentional unrelated files

## Final completion checklist

- [ ] All WS1-WS7 acceptance criteria checked.
- [ ] Docs updated.
- [ ] Tests passed.
- [ ] Commits pushed.
- [ ] User-facing final summary prepared.
- [ ] Temporary Serena memory deleted after permanent handoff exists.
```

## Baseline commands

Use these as a baseline, expanding them as new tests are added:

```bash
./scripts/check_web_mobile.sh
# Add/run the committed rendered smoke script once WS1 lands.
nix develop --command cargo test -p jcode-protocol protocol_tests --lib
nix develop --command cargo test -p jcode-base gateway_tests --lib
nix develop --command cargo check -p jcode --bin jcode
nix develop --command scripts/dev_cargo.sh build --profile selfdev -p jcode --bin jcode
git diff --check
git status --short
```

Use `selfdev build target=tui` or `selfdev build-reload target=tui` for coordinated self-development builds when appropriate. If `cargo` is missing from PATH, use `nix develop --command ...`.

## Pasteable launch prompt for the next session

```text
You are the orchestrator for the jcode interaction-surface completion program.

Your only successful stop condition is: all seven work surfaces in docs/INTERACTION_SURFACE_ORCHESTRATOR_LEDGER.md are implemented, validated, documented, committed, pushed, and operational. Do not stop after partial completion.

Start by activating the jcode project and reading:
- docs/INTERACTION_SURFACES.md
- docs/INTERACTION_SURFACE_REQUIREMENTS.md
- ~/notes/projects/jcode/proposals/surface-workspace-substrate-plan.md
- ~/notes/projects/jcode/proposals/mobile-interface/web-mobile-mvp.md
- docs/INTERACTION_SURFACE_ORCHESTRATOR_LEDGER.md

Create a temporary self-destructive Serena MCP Server memory named temp/interaction_surface_orchestrator_ledger_<YYYYMMDD_HHMMSS> using the ledger scaffold from docs/INTERACTION_SURFACE_ORCHESTRATOR_LEDGER.md. This memory is authoritative for the run. Update it after every pulse, phase boundary, subagent report, test pass/failure, commit, and blocker. Delete that temporary memory only after the final definition of done is met and a permanent repo/commit handoff exists.

Operate as an orchestrator. Use swarms of subagents for each work surface. For every WS1 through WS7, recursively execute:
research -> plan -> implement -> test -> review -> orchestrate -> recurse if incomplete.

Use a heartbeat pulse at least every 20 minutes of active work and at every phase boundary. Each pulse must update the Serena ledger, update the todo tool, inspect swarm/repo/test state, send a concise user progress update, and name the next action. Keep going until the only stop condition is satisfied.

Seven work surfaces:
1. Commit a reusable Chrome CDP rendered viewport smoke harness for Key2, Y700, and laptop.
2. Add real or faithful gateway E2E pairing/send/history/reconnect fixtures.
3. Build the typed command palette and durable verb log.
4. Implement the browser-local surface workspace store.
5. Render board/docs/annotations/intents/artifact review UI from that object graph.
6. Lift the workspace store to server-local `~/.jcode/surface_workspaces/` with Rust storage and protocol hooks.
7. Implement a safe secure access/auth/mesh operational path, using Kanidm OIDC + PKCE/WebAuthn only when security review approves, otherwise a disabled-by-default tested path plus working mesh/local access.

Constraints:
- Preserve the TUI as the primary coding cockpit.
- Keep the web path zero-build where possible.
- Treat mobile browser background WebSocket disconnects as normal.
- Persist drafts/intents/pending commands/session/local state before or while editing.
- On foreground/network return, reconnect with capped backoff, resubscribe, get_history, then reconcile pending commands.
- Avoid Backlog/GitHub sync, Milkdown, tldraw, heavy drag/drop, and heavy rich-text editor work in this program.
- Preserve unrelated worktree files, especially `docs/cloudflare-roadmap/`, unless explicitly told otherwise.
- Ask before irreversible or security-sensitive external actions such as public DNS changes or production auth modifications.

Baseline validation before final completion:
- `./scripts/check_web_mobile.sh`
- committed rendered viewport smoke script for Key2/Y700/laptop
- `nix develop --command cargo test -p jcode-protocol protocol_tests --lib`
- `nix develop --command cargo test -p jcode-base gateway_tests --lib`
- all new JS/Rust tests created for the seven work surfaces
- `nix develop --command cargo check -p jcode --bin jcode`
- `nix develop --command scripts/dev_cargo.sh build --profile selfdev -p jcode --bin jcode`
- `git diff --check`
- final `git status --short` with only intentional unrelated files

Commit and push focused changes as each surface or coherent sub-slice completes. Keep the ledger current. Continue recursively until all WS1-WS7 acceptance criteria are checked and operational.
```
