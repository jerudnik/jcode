# Interaction Surface Product Requirements

Status: Draft 2026-06-30

Companion design language: [`PERSONAL_INTERACTION_SURFACES.md`](./PERSONAL_INTERACTION_SURFACES.md)

Surface workspace substrate: [`SURFACE_WORKSPACE_SUBSTRATE_PLAN.md`](./SURFACE_WORKSPACE_SUBSTRATE_PLAN.md)

This document specifies what the personal jcode interaction surfaces must do so later sessions can focus on implementation. The design-language document explains the aesthetic and philosophy. This document is the product and implementation contract.

The surfaces are not competing clients. They are specialized controls over the same runtime:

- **Key2 / hardware-keyboard phone:** capture intent, route work, and check status.
- **Y700 tablet:** steer sessions, coordinate agents, review artifacts, and manipulate lightweight project state.
- **Desktop web:** review, annotate, plan, and supervise. The TUI remains the primary coding cockpit.

## Canonical decisions

These decisions should be treated as defaults unless explicitly revisited.

1. **The TUI remains the primary coding/chat cockpit.** Web and mobile surfaces add orchestration, review, and touch-native control.
2. **The first web implementation stays zero-build.** ArrowJS plus plain CSS/JS is the baseline. Any dependency must justify its weight.
3. **Surface-local state stays local.** Drafts, drawer state, scroll positions, selected cards, and open annotations belong to the surface until explicitly persisted.
4. **Session state stays server-owned.** Messages, tools, models, turn lifecycle, and session metadata are owned by jcode server/runtime.
5. **Cards, docs, and annotations should use the jcode-native surface workspace substrate first.** Repo export should be explicit and user-directed, not a Backlog.md adapter or task-manager default.
6. **Every rich action must have a text fallback.** If a touch board can move a card, a command can move that card too.
7. **Phosphor glyphs are preferred over emojis.** Use a vendored inline SVG subset rather than a full icon runtime.
8. **Space Grotesk, Geist Sans, Geist Mono, and Source Serif 4 are the target controlled-surface fonts.** Use fallbacks until fonts are vendored.
9. **The orchestrator is a role, not necessarily a new process.** It may start as a session convention over existing swarm/session primitives.
10. **No implementation should require cloud hosting for the local-first path.** LAN and Tailscale are enough for prototypes.

## Glossary

| Term | Meaning |
| --- | --- |
| Session | Server-owned jcode runtime with conversation, tools, provider/model state, and persistence. |
| Surface | Local UI attachment to one or more sessions. A surface can be interactive or passive. |
| Client | A process or browser app hosting one or more surfaces. |
| Intent | A captured user goal before it has become a message, task, plan, or agent assignment. |
| Orchestrator | A session or role that clarifies intent and routes work to sessions, agents, tasks, or surfaces. |
| Artifact | File, diff, image, screenshot, rendered document, diagram, evidence pack, benchmark, or tool output. |
| Annotation | Structured note against an artifact, transcript, card, diagram node, image region, or code range. |
| Card | Project-management object representing work to do, in progress, blocked, or done. |
| Handoff | Explicit transfer of context from one surface to another. |

## Global surface requirements

### SURF-G-001: pairing and connection

All browser/mobile surfaces MUST support the existing local gateway path:

- `POST /pair` with host, port, code, device ID, and device name.
- `WS /ws?token=...` for browser-compatible WebSocket auth.
- Saved credentials in local storage for web prototypes.
- Explicit disconnect and reconnect.
- Clear state for offline, connecting, live, error, and stale-token cases.

Acceptance criteria:

- A user can pair from a fresh browser without developer tools.
- A saved workstation can reconnect after reload.
- Mixed-content, unreachable-host, bad-code, and bad-token failures produce actionable text.
- Credentials can be forgotten from the UI.

### SURF-G-002: session basics

Every surface MUST be able to display, when available:

- current session ID or display title
- provider and model
- current working directory or project label
- turn lifecycle state
- latest tool activity
- token summary
- live, idle, interrupted, or error state

Acceptance criteria:

- The user can tell whether a session is safe to interrupt.
- The user can distinguish disconnected UI from idle session.
- The user can identify which model/session will receive an input before sending.

### SURF-G-003: transcript basics

Every interactive surface MUST support:

- sending a message
- canceling a running turn
- syncing history
- rendering streamed assistant text
- rendering reasoning as visually secondary
- rendering tool calls with name, status, input, output, and error
- rendering system notifications distinctly from assistant text

Acceptance criteria:

- Current `web/jcode-mobile` event types still render correctly.
- Unknown event types are ignored safely with optional debug/status feedback.
- A long streamed response does not block typing or cancel.

Current event baseline:

```text
ack
history
session
session_renamed
state
available_models_updated
model_changed
tokens
status_detail
notification
reasoning_delta
reasoning_done
text_delta
text_replace
tool_start
tool_input
tool_exec
tool_done
message_end
done
interrupted
error
```

Current outbound baseline:

```text
subscribe
get_history
message
cancel
resume_session
set_model
```

### SURF-G-004: command palette and command verbs

Every surface SHOULD expose a command palette, even if the first version is just typed slash commands. The palette is the bridge between touch, hardware keyboard, and text fallback.

Required command verbs for the first coherent system:

| Verb | Meaning | Minimum payload |
| --- | --- | --- |
| `message.send` | Send text to active session | `session_id`, `content` |
| `turn.cancel` | Cancel active turn | `session_id` |
| `history.sync` | Refresh session history | `session_id` |
| `session.attach` | Attach surface to session | `session_id` |
| `session.switch` | Change active session | `session_id` |
| `model.set` | Change model | `model` |
| `intent.capture` | Save rough intent | `body`, optional `target` |
| `intent.route` | Ask orchestrator to route captured intent | `intent_id` or `body` |
| `agent.spawn` | Spawn or request agent | `prompt`, optional `role` |
| `agent.assign` | Assign task to agent | `task_id`, `agent_id` or `role` |
| `artifact.open` | Open or focus artifact | `artifact_id` or `path` |
| `annotation.create` | Create annotation | `target`, `body`, `kind` |
| `card.create` | Create project card | `title`, optional `body` |
| `card.move` | Move card to lane/order | `card_id`, `status`, optional `ordinal` |
| `surface.handoff` | Continue context elsewhere | `target_surface`, `context` |
| `summary.request` | Ask for terse current-state summary | `scope` |

Acceptance criteria:

- A Key2 user can run core actions without touch targets.
- A tablet user can execute the same actions from buttons or palette.
- Commands can be logged and replayed in tests.

### SURF-G-005: local persistence and recovery

Surfaces MUST avoid losing user work on reload.

Persist locally:

- draft composer text by surface and session
- captured intents not yet routed
- unsynced annotations
- selected server/session/model
- open drawer and focus mode
- local card edits not yet written back

Do not store locally as canonical state:

- complete session transcript when server history is authoritative
- provider credentials beyond gateway token
- final project board state once server-local surface workspace storage exists

Acceptance criteria:

- Reloading the web app preserves unsent text and open context.
- Failed project writes remain visible as unsynced state.
- Clearing local state is possible from settings or debug action.

### SURF-G-006: safety and reversibility

Surfaces MUST gate destructive or externally visible actions.

Actions requiring confirmation:

- deleting credentials
- deleting cards or annotations
- stopping agents outside the user's current subtree
- pushing commits
- sending email or posting external comments
- destructive file operations

Actions that do not require confirmation:

- saving local drafts
- creating local annotations
- creating local cards
- moving a card between lanes if the move is reversible
- requesting a summary
- canceling a running turn

Acceptance criteria:

- A user cannot accidentally trigger destructive repo or external effects from touch misfires.
- Reversible local edits can be undone or restored from a persisted local state record.

### SURF-G-007: performance budgets

Performance is a product feature.

Baseline budgets for the zero-build web client:

| Surface class | Initial JS target | Interaction target | Notes |
| --- | ---: | ---: | --- |
| Key2 lite | under 150 KB app JS excluding ArrowJS CDN | input response under 50 ms | no blur, minimal metrics |
| Y700 tablet | under 250 KB app JS excluding ArrowJS CDN | drawer open under 120 ms | richer layout allowed |
| Desktop web | under 350 KB app JS excluding ArrowJS CDN | view switch under 120 ms | artifact views may lazy-load |

Rules:

- No charting runtime in early versions.
- No large editor dependency until a textarea plus preview fails a concrete workflow.
- Lazy-load artifact viewers and optional editors.
- Prefer CSS transforms and opacity for motion.
- Respect `prefers-reduced-motion`.

Acceptance criteria:

- Key2 lite mode works with side panels disabled.
- Typing remains responsive while text streams.
- The app passes a lightweight static validation script before merge.

## Surface capability contract

Clients SHOULD eventually send a capability announcement after subscribe. This can start as local-only metadata before becoming protocol-visible.

```json
{
  "type": "surface_capabilities",
  "surface_id": "device-local-uuid",
  "surface_kind": "key2-web",
  "device_label": "BlackBerry Key2",
  "viewport": {
    "width_css_px": 360,
    "height_css_px": 640,
    "orientation": "portrait",
    "device_pixel_ratio": 3
  },
  "input": ["hardware_keyboard", "touch"],
  "features": ["single_session", "command_palette", "lite_mode"],
  "constraints": {
    "prefer_terse": true,
    "prefer_text_fallbacks": true,
    "can_render_canvas": false,
    "can_drag_drop": false,
    "network": "lan_or_tailnet"
  }
}
```

Initial `surface_kind` values:

| Surface kind | Meaning |
| --- | --- |
| `key2-web` | hardware-keyboard narrow browser surface |
| `phone-web` | generic narrow phone browser surface |
| `y700-web-portrait` | tablet portrait command/review surface |
| `y700-web-landscape` | tablet landscape command plane |
| `desktop-web-review` | full-screen desktop review/planning surface |
| `tui` | terminal cockpit attachment |

Acceptance criteria:

- The surface can compute and display its own mode.
- The orchestrator can later use this to format output.
- Tests can simulate each capability profile without real hardware.

## Shared object schemas

These are working schemas for implementation. They are not final server contracts yet, but UI state should map cleanly to them.

### IntentCapture

```json
{
  "intent_id": "intent_...",
  "body": "Find the failing build issue and route it to an agent.",
  "source_surface_id": "surface_...",
  "target": {
    "type": "project",
    "id": "jcode"
  },
  "urgency": "normal",
  "confidence": 0.74,
  "status": "captured",
  "created_at": "2026-06-30T16:00:00Z",
  "updated_at": "2026-06-30T16:00:00Z"
}
```

Allowed statuses:

```text
captured
clarifying
routed
accepted
blocked
done
superseded
```

### CommandEnvelope

```json
{
  "command_id": "cmd_...",
  "verb": "intent.route",
  "source_surface_id": "surface_...",
  "session_id": "session_...",
  "payload": {},
  "created_at": "2026-06-30T16:00:00Z",
  "idempotency_key": "surface_...:local-counter"
}
```

Rules:

- Every command has a stable `command_id`.
- Mutating commands SHOULD have an `idempotency_key`.
- Reversible commands SHOULD include enough state to undo or restore locally.

### ArtifactSummary

```json
{
  "artifact_id": "artifact_...",
  "kind": "file",
  "title": "docs/PERSONAL_INTERACTION_SURFACES.md",
  "uri": "repo://docs/PERSONAL_INTERACTION_SURFACES.md",
  "preview": "Personal Interaction Surface Design Language",
  "provenance": {
    "session_id": "session_...",
    "task_id": "task_...",
    "created_by": "agent"
  },
  "status": "available",
  "annotations": ["annotation_..."]
}
```

Allowed artifact kinds:

```text
file
diff
image
screenshot
rendered_markdown
diagram
web_page
terminal_output
benchmark
evidence_pack
message
```

### Annotation

```json
{
  "annotation_id": "annotation_...",
  "target": {
    "kind": "file_range",
    "uri": "repo://docs/file.md",
    "range": { "start_line": 10, "end_line": 18 }
  },
  "body": "Clarify this acceptance criterion.",
  "kind": "change_request",
  "status": "open",
  "created_by": "user",
  "created_at": "2026-06-30T16:00:00Z",
  "links": {
    "task_ids": [],
    "session_ids": [],
    "artifact_ids": []
  }
}
```

Allowed annotation targets:

```text
file_range
message
image_region
diagram_node
diagram_edge
card
url
dom_selector
freeform_canvas_region
```

Allowed annotation kinds:

```text
note
change_request
question
approval
defect
idea
```

### TaskCard

```json
{
  "card_id": "card_...",
  "title": "Add Key2 lite mode",
  "body": "Single-column mode with keyboard shortcuts and no blur.",
  "status": "todo",
  "priority": "high",
  "ordinal": 1000,
  "assignee": "unassigned",
  "labels": ["surface", "key2"],
  "acceptance": [
    "No side rail is visible below 430 CSS px.",
    "Esc cancels a running turn."
  ],
  "links": {
    "session_ids": [],
    "artifact_ids": [],
    "annotation_ids": [],
    "issue_urls": [],
    "pr_urls": []
  },
  "storage": {
    "kind": "surface_workspace",
    "workspace_id": "sw_...",
    "object_id": "obj_..."
  }
}
```

Allowed statuses:

```text
todo
ready
in_progress
blocked
review
done
archived
```

### SurfaceHandoff

```json
{
  "handoff_id": "handoff_...",
  "from_surface_id": "surface_key2",
  "to_surface_kind": "y700-web-landscape",
  "context": {
    "session_id": "session_...",
    "intent_id": "intent_...",
    "artifact_id": "artifact_...",
    "view": "artifact_review"
  },
  "summary": "Continue reviewing the tablet drawer spec.",
  "created_at": "2026-06-30T16:00:00Z",
  "status": "requested"
}
```

Allowed statuses:

```text
requested
available
accepted
expired
cancelled
```

## Key2 / Clicks field terminal requirements

Surface ID prefix: `KEY2`

### Primary jobs

1. Capture rough intent while away from the main machine.
2. Route that intent to an orchestrator or active session.
3. Check whether work is idle, running, blocked, or done.
4. Send short imperative commands.
5. Receive terse summaries and explicit choices.

### Non-goals

- Editing project boards by drag/drop.
- Diagram or canvas editing.
- Full artifact review.
- Multi-pane dashboards.
- High-frequency telemetry.

### KEY2-P0 requirements

| ID | Requirement | Acceptance criteria |
| --- | --- | --- |
| KEY2-P0-001 | True lite mode | At `max-width: 430px`, side rails, blur-heavy panels, and nonessential metrics are hidden. Transcript and composer dominate. |
| KEY2-P0-002 | Hardware keyboard send flow | Enter sends in send mode. Ctrl+Enter or Shift+Enter inserts newline. Esc cancels turn or closes palette. |
| KEY2-P0-003 | Command palette | `/` opens command input. At least cancel, sync, attach, summarize, route, and handoff are available. |
| KEY2-P0-004 | Intent capture | User can save rough text locally without selecting a session. It survives reload. |
| KEY2-P0-005 | Orchestrator route | User can submit captured intent to active session or orchestrator with one action. |
| KEY2-P0-006 | Terse status | Shows connection, active session, turn state, and model in one compact strip. |
| KEY2-P0-007 | Low attention summaries | Summary responses are formatted as short bullets and choices when surface kind is `key2-web`. |

### KEY2-P1 requirements

| ID | Requirement | Acceptance criteria |
| --- | --- | --- |
| KEY2-P1-001 | Target shortcuts | `@` opens session/agent target picker. `#` opens task/tag picker. |
| KEY2-P1-002 | Offline outbox | Captured intents queue when disconnected and prompt before sending after reconnect. |
| KEY2-P1-003 | Handoff to tablet/desktop | Current intent/session can be made available to another surface. |
| KEY2-P1-004 | Battery-friendly mode | Disables gradients, backdrop blur, live telemetry polling, and nonessential animations. |

### KEY2 happy paths

#### Capture and route intent

1. User opens phone browser.
2. Surface reconnects to saved workstation.
3. User types rough intent.
4. User presses `/route` or a route button.
5. Orchestrator either accepts or asks one clarifying question.
6. Phone shows a terse confirmation with target session/agent.

#### Check work while away

1. User opens status view.
2. Surface shows active sessions and running agents in a compact list.
3. User requests summary.
4. Orchestrator returns bullets: done, running, blocked, needs decision.
5. User chooses cancel, continue, assign, or ignore.

## Y700 tablet command-plane requirements

Surface ID prefix: `Y700`

### Primary jobs

1. Steer one or more live sessions.
2. Coordinate agents and swarm work.
3. Review artifacts with enough context for decisions.
4. Create and move task cards.
5. Capture, refine, and route intent.
6. Annotate transcripts, files, screenshots, diagrams, and rendered docs.

### Non-goals

- Replacing the TUI for long coding conversations.
- Full IDE editing.
- Heavy whiteboard implementation as the first canvas.
- Depending on cloud sync for project state.

### Orientation behavior

#### Portrait

Portrait optimizes deep focus.

Required layout:

```text
status strip
primary transcript or artifact
composer or annotation input
bottom sheet for actions, references, cards, or details
```

Acceptance criteria:

- A single transcript, artifact, or card detail can occupy most of the height.
- Bottom sheet can be dismissed quickly.
- Composer is reachable without losing the selected artifact or card.

#### Landscape

Landscape optimizes command-plane work.

Required layout:

```text
left rail: sessions, agents, project
center: transcript, artifact, board, or canvas
right drawer: details, annotations, plan, commands
bottom rail: composer, quick prompts, status pulses
```

Acceptance criteria:

- Left rail and right drawer can collapse independently.
- Center remains usable when either side is collapsed.
- Drawer switches do not reset selected center content.

### Y700-P0 requirements

| ID | Requirement | Acceptance criteria |
| --- | --- | --- |
| Y700-P0-001 | Drawer shell | Sessions, Intent, Project, Artifacts, Annotations, and Models drawers exist as switchable panes, even if some are initially read-only. |
| Y700-P0-002 | Session cockpit | Session list shows title/ID, state, model, and whether a turn is running. |
| Y700-P0-003 | Intent drawer | Captured intents can be edited, routed, marked done, or converted to cards. |
| Y700-P0-004 | Artifact list | Files, transcript messages, tool outputs, screenshots, or docs can be represented as artifacts with type and provenance. |
| Y700-P0-005 | Annotation primitive | User can create a text annotation on at least transcript messages and artifact summaries. |
| Y700-P0-006 | Local project cards | User can create cards, move them between lanes, and persist locally. |
| Y700-P0-007 | Handoff accept | Tablet can accept a Key2 or desktop handoff and focus the relevant session/artifact/intent. |
| Y700-P0-008 | Orientation mode | CSS and state respond to portrait vs landscape without losing drafts or selected items. |

### Y700-P1 requirements

| ID | Requirement | Acceptance criteria |
| --- | --- | --- |
| Y700-P1-001 | Server-local surface workspace | Card, doc, annotation, intent, and artifact-ref objects can sync to a jcode-native server-local surface workspace store. |
| Y700-P1-002 | Artifact preview | Markdown, image, text, and diff previews render in the center panel. |
| Y700-P1-003 | Image annotation | User can draw or place a rectangular note over an image or screenshot. |
| Y700-P1-004 | Agent drawer | Swarm/agent status shows spawned, running, blocked, completed, failed, and stopped states. |
| Y700-P1-005 | Command chips | Context-aware chips appear for common next actions, but every chip maps to a command verb. |
| Y700-P1-006 | Split review | Artifact center plus annotation drawer supports open, resolve, and link-to-card flows. |

### Y700-P2 requirements

| ID | Requirement | Acceptance criteria |
| --- | --- | --- |
| Y700-P2-001 | Lightweight canvas | SVG or simple layered canvas supports boxes, arrows, freehand marks, and annotations. |
| Y700-P2-002 | Diagram round-trip | Mermaid or SVG diagrams can be viewed, annotated, and linked back to source files. |
| Y700-P2-003 | Stylus ergonomics | Touch and stylus affordances do not conflict with scroll, text selection, or drawer gestures. |
| Y700-P2-004 | Multi-session watch | Tablet can watch multiple sessions passively while one remains active for input. |

### Y700 happy paths

#### Steer a running swarm

1. Tablet opens in landscape.
2. Sessions rail shows active coordinator and worker sessions.
3. Agent drawer shows running, blocked, and completed workers.
4. User taps a blocked worker.
5. Right drawer shows blocker summary and suggested commands.
6. User assigns a follow-up or asks coordinator to resolve.

#### Review and annotate artifact

1. User opens artifact drawer.
2. User selects a diff, markdown doc, screenshot, or generated image.
3. Center panel shows preview.
4. User creates annotation in right drawer.
5. Annotation can be linked to a card or sent to an agent.
6. Local state persists even if the browser reloads.

#### Project board update

1. User opens Project drawer.
2. Cards render by status lane.
3. User drags a card from `todo` to `in_progress` or uses `card.move` command.
4. UI records local change immediately.
5. If server-local surface workspace sync is available, the change is persisted there.
6. If sync fails, unsynced state remains visible and retryable.

## Desktop web review surface requirements

Surface ID prefix: `DWEB`

### Primary jobs

1. Review artifacts, plans, diffs, diagrams, docs, and screenshots.
2. Annotate with rich context.
3. Supervise sessions and swarms at a glance.
4. Convert observations into cards, tasks, or agent instructions.
5. Handoff context to TUI, tablet, or phone.

### Non-goals

- Replacing the TUI for coding chat.
- Replacing an IDE for source editing.
- Replacing OS-level window management.
- Becoming a general admin dashboard.

### DWEB-P0 requirements

| ID | Requirement | Acceptance criteria |
| --- | --- | --- |
| DWEB-P0-001 | Full-screen review shell | Center artifact/review pane with session/status rail and annotation/task drawer. |
| DWEB-P0-002 | Artifact focus | Opening an artifact makes it the primary object, not a secondary transcript detail. |
| DWEB-P0-003 | Annotation drawer | User can list, create, edit, resolve, and link annotations. |
| DWEB-P0-004 | Plan/task conversion | Annotation or selected text can become a card or agent instruction. |
| DWEB-P0-005 | Session supervision | Shows active sessions and can attach, switch, summarize, or cancel. |
| DWEB-P0-006 | Handoff to TUI/tablet/phone | Current review context can be summarized and made available to another surface. |

### DWEB-P1 requirements

| ID | Requirement | Acceptance criteria |
| --- | --- | --- |
| DWEB-P1-001 | Code/doc line anchors | File annotations can target line ranges. |
| DWEB-P1-002 | PR-style review mode | Open annotations can be grouped into a review summary. |
| DWEB-P1-003 | Diagram review | Mermaid/SVG diagrams render with selectable nodes or line anchors. |
| DWEB-P1-004 | Evidence pack view | Generated artifacts can be browsed as a coherent evidence bundle. |
| DWEB-P1-005 | Cross-session timeline | Shows high-level event timeline across selected sessions or swarm. |

### Desktop happy paths

#### Review implementation output

1. User opens desktop review surface from a session or artifact link.
2. Center pane shows diff, doc, screenshot, or plan.
3. Right drawer shows existing annotations and task links.
4. User marks issues as change requests.
5. User converts selected requests into cards or sends them to an agent.
6. Surface records review summary and links it to the source session.

#### Plan next work

1. User opens project board or roadmap doc.
2. User annotates missing decisions.
3. User converts decisions into cards with acceptance criteria.
4. User assigns cards to coordinator or future implementation session.
5. Handoff summary can be sent to tablet or TUI.

## Orchestrator behavior requirements

The orchestrator can begin as a convention in an ordinary jcode session. It does not need a special runtime for P0.

### ORCH-P0 requirements

| ID | Requirement | Acceptance criteria |
| --- | --- | --- |
| ORCH-P0-001 | One-question clarification | If intent is ambiguous, ask at most one focused question before routing. |
| ORCH-P0-002 | Surface-aware response | Responses honor surface capabilities, especially terse Key2 output. |
| ORCH-P0-003 | Routing summary | Every routed intent returns target, action, and expected next update. |
| ORCH-P0-004 | Task conversion | Orchestrator can turn intent into a card or implementation prompt. |
| ORCH-P0-005 | Agent/session routing | Orchestrator can choose active session, spawn agent, assign task, or defer. |

### Routing policy

Default decision tree:

```text
Is this a quick answer? -> answer directly.
Is this about current session? -> send to current session or coordinator.
Is this implementation work? -> create/assign task or spawn agent.
Is this review/annotation work? -> open or create artifact/annotation flow.
Is this too ambiguous? -> ask one clarifying question.
Is this risky/destructive/external? -> require explicit confirmation.
```

Orchestrator confirmations should be compact:

```text
Routed: Y700 drawer prototype
Target: session_crocodile coordinator
Action: spawn UI agent with drawer-shell task
Next: I will report blocker/done summary here
```

## Storage and synchronization requirements

### Local storage keys

Use namespaced keys. Do not reuse current credential keys for unrelated state.

Suggested web keys:

```text
jcode.mobileWeb.credentials.v1
jcode.mobileWeb.deviceId.v1
jcode.surfaceWorkspace.active.v1
jcode.surfaceWorkspace.<workspace_id>.snapshot.v1
jcode.surfaceWorkspace.<workspace_id>.events.v1
jcode.handoffs.v1
```

### Surface workspace durable homes

| State | Preferred early home | Notes |
| --- | --- | --- |
| Cards | surface workspace `card` objects | Must preserve ordering and status. |
| Design decisions | surface workspace `doc` objects | Export to `docs/` markdown explicitly when wanted. |
| Annotations | surface workspace `annotation` objects | Need target URI, selector, and stale-anchor handling. |
| Diagrams | artifact refs plus later SVG/canvas marks | Link rather than duplicate. |
| Evidence packs | artifact refs to existing evidence directories | Link rather than duplicate. |
| Handoffs | server/session state first, local fallback | Needs expiry or acknowledgement. |

### Sync conflict policy

P0 policy:

1. Local edits apply immediately.
2. Server-local surface workspace sync is attempted when available.
3. If write fails, mark item `unsynced` with retry action.
4. If server/export state changed since local edit, preserve both and ask user or orchestrator to reconcile.
5. Never silently discard local drafts or annotations.

## Event extensions for future protocol work

These are protocol candidates. They should not block UI-local prototypes.

### Surface events

```text
surface.capabilities
surface.focus_changed
surface.handoff_requested
surface.handoff_available
surface.handoff_accepted
surface.state_saved
```

### Intent events

```text
intent.captured
intent.clarification_requested
intent.routed
intent.accepted
intent.blocked
intent.done
```

### Artifact events

```text
artifact.created
artifact.updated
artifact.opened
artifact.preview_available
artifact.annotation_created
artifact.annotation_resolved
```

### Card events

```text
card.created
card.updated
card.moved
card.linked
card.completed
card.sync_failed
```

### Agent events

Map to the swarm architecture states where possible:

```text
agent.spawned
agent.ready
agent.running
agent.blocked
agent.completed
agent.failed
agent.stopped
agent.crashed
```

## Implementation phases

### Phase 0: clarify current web client boundaries

Goal: make the current ArrowJS app a stable host for future surface work.

Tasks:

- Add shared design tokens for fonts, colors, spacing, radii, and focus rings.
- Add surface mode detection for Key2, tablet portrait, tablet landscape, and desktop review.
- Add local surface state persistence for draft, focus mode, selected drawer, and selected session.
- Add command registry with current outbound messages mapped to command verbs.
- Add a validation fixture for each surface mode.

Done when:

- Static check script validates the command registry and required selectors.
- Reload preserves local surface state.
- Current chat, history, cancel, model, and session switch behavior still works.

### Phase 1: Key2 lite and intent capture

Goal: make the phone/hardware keyboard path useful away from the desk.

Tasks:

- Implement true lite mode.
- Add keyboard shortcuts.
- Add command palette.
- Add intent outbox.
- Add route-to-orchestrator action.
- Add terse status summary.

Done when:

- A narrow viewport can capture and route intent with keyboard only.
- Local outbox survives reload.
- User can cancel a running turn with Esc.

### Phase 2: Y700 drawer shell

Goal: make tablet landscape useful as a command plane.

Tasks:

- Add left rail, center pane, right drawer, and bottom rail layout.
- Add drawer registry for Sessions, Intent, Project, Artifacts, Annotations, and Models.
- Add local cards and annotations.
- Add handoff accept state.
- Add portrait layout adaptation.

Done when:

- Tablet can switch drawers without losing center context.
- Cards and annotations persist locally.
- A handoff can focus a session, artifact, or intent.

### Phase 3: server-local surface workspace primitives

Goal: make planning and review durable beyond browser storage without adopting an external task format.

Tasks:

- Add jcode-native surface workspace serde types.
- Add safe read/write path through server/tool layer.
- Store object metadata as compact JSON, bodies as markdown, and operations as JSONL.
- Link annotations to artifacts, cards, docs, and sessions.
- Add sync-failure UI.

Done when:

- Card moves can be written to server-local surface workspace storage.
- Annotation can target a transcript message and a file range.
- Failed sync remains retryable and visible.

### Phase 4: desktop review and artifact workflow

Goal: make desktop web valuable alongside the TUI.

Tasks:

- Add review shell mode.
- Add artifact preview registry.
- Add annotation drawer with resolve/link flows.
- Add convert-to-card and send-to-agent actions.
- Add review summary export.

Done when:

- A doc, diff, screenshot, or transcript artifact can be opened as primary content.
- User can create annotations and convert them into tasks or agent prompts.
- TUI remains the recommended direct coding cockpit.

### Phase 5: richer canvas and diagram collaboration

Goal: enable touch/stylus visual thinking without adding a heavy whiteboard too early.

Tasks:

- Add SVG-backed annotation layer.
- Add boxes, arrows, and freehand marks.
- Add Mermaid/SVG node linking when possible.
- Add export to markdown-linked artifact.

Done when:

- A diagram or screenshot can receive spatial annotations.
- Those annotations can be linked to cards or agent instructions.

## Test matrix

### Viewport fixtures

| Fixture | CSS viewport | Expected mode |
| --- | --- | --- |
| `key2_portrait` | 360 x 640 | `key2-web`, lite eligible |
| `phone_portrait` | 390 x 844 | `phone-web`, single column |
| `y700_portrait` | 820 x 1180 | `y700-web-portrait` |
| `y700_landscape` | 1180 x 820 | `y700-web-landscape` |
| `desktop_review` | 1440 x 960 | `desktop-web-review` |

### Protocol fixtures

Each implementation phase SHOULD add replay fixtures for:

- connection lifecycle
- history load
- streaming text and reasoning
- tool start/input/exec/done
- model list and model change
- notification
- error
- interrupted
- unknown event
- command palette action
- local persistence reload

### Accessibility fixtures

Minimum checks:

- All icon-only controls have labels.
- Keyboard focus is visible.
- Key2 core path can be operated with keyboard only.
- Touch targets are at least 44 CSS px where touch is primary.
- Reduced-motion mode disables drawer animation and button transforms.

## Implementation backlog by stable ID

Use these IDs in commits, issues, or future agent prompts.

### P0 backlog

```text
SURF-P0-001 Add shared design tokens and font stacks.
SURF-P0-002 Add surface mode detection and capability object.
SURF-P0-003 Add command registry mapping current outbound protocol.
SURF-P0-004 Persist draft, selected session, selected model, focus mode, and drawer state.
KEY2-P0-001 Implement true lite mode.
KEY2-P0-002 Add hardware keyboard send/newline/cancel shortcuts.
KEY2-P0-003 Add slash command palette.
KEY2-P0-004 Add local intent capture and outbox.
KEY2-P0-005 Add route-to-orchestrator action.
Y700-P0-001 Add drawer shell and drawer registry.
Y700-P0-003 Add intent drawer.
Y700-P0-005 Add annotation primitive for transcript/artifact summaries.
Y700-P0-006 Add local project cards.
DWEB-P0-001 Add desktop review shell mode.
```

### P1 backlog

```text
SURF-P1-001 Add handoff object and local handoff inbox.
SURF-P1-002 Add server-local surface workspace sync spike.
SURF-P1-003 Add surface workspace export pack for annotations, cards, docs, and artifact refs.
Y700-P1-002 Add markdown/image/text/diff artifact previews.
Y700-P1-004 Add agent drawer from swarm status events.
DWEB-P1-001 Add file line-range annotations.
DWEB-P1-002 Add PR-style review summary grouping.
```

### P2 backlog

```text
SURF-P2-001 Add protocol-visible surface capabilities.
SURF-P2-002 Add server-side handoff events.
Y700-P2-001 Add lightweight SVG canvas.
Y700-P2-002 Add diagram round-trip annotations.
DWEB-P2-005 Add cross-session timeline.
```

## Things not to build yet

Avoid these until P0/P1 prove they are needed:

- full whiteboard runtime
- full Milkdown integration
- large drag/drop framework
- charting package
- hosted cloud relay UI
- full native Android rewrite
- complete IDE source editor
- generative UI that changes layout opaquely

## Default prompts for future implementation sessions

### Key2 implementation prompt

```text
Implement KEY2-P0-001 through KEY2-P0-005 from docs/INTERACTION_SURFACE_REQUIREMENTS.md in web/jcode-mobile. Preserve zero-build ArrowJS. Add tests or static checks to scripts/check_web_mobile.sh. Do not add dependencies unless unavoidable.
```

### Y700 implementation prompt

```text
Implement Y700-P0-001, Y700-P0-003, Y700-P0-005, and Y700-P0-006 from docs/INTERACTION_SURFACE_REQUIREMENTS.md. Build a drawer registry and local persistence first. Keep artifact previews simple and defer heavy editors.
```

### Desktop review implementation prompt

```text
Implement DWEB-P0-001 through DWEB-P0-004 from docs/INTERACTION_SURFACE_REQUIREMENTS.md as a desktop review mode. The center pane should make artifacts primary, with annotations and card conversion in a side drawer. Do not replace the TUI chat workflow.
```

## Success definition

The surface program is working when:

1. The Key2 can capture intent and route work in under one minute while away from the desk.
2. The Y700 can steer sessions, annotate artifacts, and move project cards without needing the TUI for every coordination action.
3. Desktop web can review and annotate outputs more effectively than a chat transcript alone.
4. Every rich action has a text command fallback.
5. Local work survives reloads and failed sync.
6. The implementation remains light enough to feel like jcode, not a dashboard product glued onto jcode.
