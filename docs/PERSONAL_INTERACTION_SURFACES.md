# Personal Interaction Surface Design Language

Status: Draft 2026-06-30

Implementation requirements: [`INTERACTION_SURFACE_REQUIREMENTS.md`](./INTERACTION_SURFACE_REQUIREMENTS.md)

This document records a personal design direction for jcode interaction surfaces that are more controllable than the terminal UI. It extends the existing jcode values of minimalism, cypherpunk pragmatism, functional density, and crisp performance without turning the UI into a heavyweight dashboard.

The central idea is that every surface should feel like the same instrument viewed through a device-specific lens:

- **Phone / hardware keyboard:** field terminal and intent capture.
- **Small tablet:** command plane, steering surface, and light project console.
- **Desktop / laptop web:** review, annotation, planning, and control plane alongside the TUI.
- **TUI:** the canonical coding cockpit and fastest direct interaction surface.

## Design principles

1. **Transcript and intent first.** The primary object is still a conversation, command, or captured intent. UI chrome exists to remove work, not to create more interaction.
2. **Surface-local, session-global.** Sessions, messages, tools, tasks, and artifacts are server or repo owned. Draft text, scroll position, open drawers, focus, gesture state, and view density are local to the current surface.
3. **Every rich surface degrades to text.** A kanban card, diagram annotation, or drawer action should always have a text representation that works from the Key2 or TUI.
4. **No gratuitous motion.** Motion should confirm cause and effect. Use short transform-only transitions, disable them in low-power or reduced-motion modes, and never make users wait for animation.
5. **Functional cypherpunk, not retro cosplay.** Dark graphite, terse telemetry, strong contrast, obvious status, and precise glyphs. Avoid skeuomorphic panels, fake terminals, heavy blur, noisy scanlines, and dashboard clutter.
6. **Local-first and recoverable.** Drafts, annotations, board moves, and captured intent should land in local storage or a repo-backed file before they depend on a remote service.
7. **One shared grammar.** The same nouns should appear everywhere: session, agent, task, artifact, annotation, plan, model, workspace, surface.

## Visual language

### Typography

Use richer fonts where the client controls rendering, especially web and future native shells. The TUI can keep terminal-native fonts unless a controlled renderer is in use.

| Role | Preferred family | Use |
| --- | --- | --- |
| Display and headings | **Space Grotesk** | Product title, drawer titles, board names, major mode labels |
| Body/UI sans | **Geist Sans** | Forms, buttons, labels, cards, transcript prose on web surfaces |
| Mono/code | **Geist Mono** | Tool output, code, session IDs, telemetry, shortcuts, command previews |
| Serif | **Source Serif 4** | Long-form reading mode, documentation excerpts, reflective planning notes |
| Fallback serif | Charter, Georgia, serif | System fallback when Source Serif 4 is unavailable |

Serif recommendation: **Source Serif 4**. It is neutral, readable, technically polished, and pairs better with Geist than a more expressive editorial serif. Use it sparingly for long-form markdown and reading/annotation modes, not for operational chrome. If a more literary voice is desired later, evaluate Newsreader as an alternate reading-mode theme, not as the default.

Suggested CSS token direction:

```css
:root {
  --font-display: "Space Grotesk", "Geist", ui-sans-serif, system-ui, sans-serif;
  --font-sans: "Geist", ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
  --font-mono: "Geist Mono", ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  --font-serif: "Source Serif 4", Charter, Georgia, serif;
}
```

Implementation note: keep the current zero-build browser constraint. Either rely on locally installed fonts during prototypes or vendor a small static font subset when the PWA shell is formalized. Avoid runtime font services for offline/local-first use.

### Glyphs and icons

Use **Phosphor Icons** for glyphs instead of emojis. Prefer inline SVG or a tiny vendored subset over a whole icon font so the web client remains deterministic, cacheable, and lightweight.

Initial icon grammar:

| Concept | Phosphor direction |
| --- | --- |
| Session | `terminal-window`, `chat-circle-text` |
| Multi-session / workspace | `squares-four`, `layout` |
| Agent / swarm | `robot`, `tree-structure`, `users-three` |
| Task / project | `kanban`, `check-square`, `list-checks` |
| Artifact | `file-text`, `file-code`, `image`, `package` |
| Annotation | `note-pencil`, `highlighter-circle`, `cursor-click` |
| Diagram / graph | `graph`, `nodes`, `flow-arrow` |
| Pair / connect | `link`, `plugs-connected`, `qr-code` |
| Cancel / danger | `stop-circle`, `warning-diamond`, `x-circle` |
| Status live | `pulse`, `circle`, `broadcast` |

Guidelines:

- Icons are labels and affordance hints, not decoration.
- Prefer regular weight for normal chrome, fill/duotone only for active state or critical attention.
- Pair icon-only controls with `aria-label` and visible text on first-use or low-confidence surfaces.
- Use glyphs consistently across device classes so command muscle memory transfers.

### Color and material

The current green-on-graphite scheme is in a good place. Keep color mostly semantic rather than decorative.

| Token | Meaning |
| --- | --- |
| Graphite / near black | ambient background, focus, low energy draw |
| Deep green panels | jcode identity and cypherpunk continuity |
| Mint | live, primary action, successful connection |
| Blue | linked artifact, navigable reference, network path |
| Purple | reasoning, agentic context, alternate path |
| Orange | pending, needs attention, degraded state |
| Red | stop, failed, destructive, unsafe |

Material rules:

- Prefer solid or lightly translucent panels. Blur is acceptable on tablet/desktop, but should be disabled in lite mode.
- One background gradient is enough. Do not add noise unless it measurably improves perception.
- Borders should do more hierarchy work than shadows. Shadows should be subtle and rare.
- Make focus rings highly visible. Cypherpunk does not mean inaccessible.

### Motion and interaction timing

| Interaction | Target |
| --- | --- |
| Button press | 80 to 120 ms transform or color response |
| Drawer slide | 140 to 180 ms transform-only |
| Panel resize / orientation adaptation | immediate or under 120 ms |
| Session stream updates | no animation, text should feel live |
| Drag/drop card feedback | immediate hover/lift, persist on release |

Respect `prefers-reduced-motion`. On Key2 or lite mode, disable non-essential transitions.

### Density

Density should be device-adaptive, not globally minimal.

- **Key2:** one column, transcript-dominant, sparse controls, hardware-keyboard shortcuts, no canvas, no persistent side rails.
- **Y700 tablet:** compact cockpit, drawers, quick rails, cards, touch/stylus annotation, split orientation-specific layouts.
- **Desktop web:** wide review/planning tables, annotation sidebars, artifact inspectors, control-plane panels. Do not try to out-chat the TUI.

## Shared basics across all surfaces

These are the atomic primitives that should remain coherent across clients.

### Session

A server-owned runtime. A surface attaches to a session, observes it, and can send messages or control actions if authorized.

Required basics:

- title or short ID
- provider/model
- cwd/project
- live/running/idle/error state
- current turn status
- recent tool activity

### Surface

A local presentation and input context for one or more sessions.

Surface-local state:

- draft input
- scroll and selection
- drawer visibility
- viewport density
- open artifact/annotation
- gesture or keyboard focus
- local caches and unsynced edits

### Intent

A captured user desire that may become a chat message, orchestrator instruction, task, plan, or agent assignment.

Intent should support:

- raw text capture
- optional clarifying questions
- target selection: session, project, agent, task, artifact
- confidence and urgency
- conversion to durable task or plan

### Artifact

A thing produced or inspected by agents: file, diff, image, evidence pack, screenshot, rendered doc, diagram, terminal output, benchmark, or web page.

Artifact basics:

- stable ID or path
- type
- provenance
- preview
- open/reveal action
- annotations
- related session and task links

### Annotation

A first-class comment or mark on an artifact, not just a visual overlay.

Minimum schema:

```text
annotation_id
target: file path, URL, message ID, image region, DOM node, line range, or diagram node
body: markdown text
kind: note | change-request | question | approval | defect | idea
status: open | resolved | superseded
created_by: user | agent
created_at
links: task IDs, session IDs, artifact IDs
```

### Task/card

A durable project-management object, ideally repo-backed. Backlog.md tasks are a good local-first target, with GitHub Issues or PR metadata as sync targets rather than the source of truth in early prototypes.

Card basics:

- title
- status/lane
- priority
- assignee/agent
- labels
- references/artifacts
- acceptance criteria
- definition of done
- modified files
- ordinal for manual ordering

### Command

A typed or tapped action that should be invokable from every surface.

Examples:

- send message
- cancel turn
- attach session
- spawn agent
- assign task
- annotate artifact
- create card
- move card
- summarize current state
- hand off to another surface

## Device-specific surfaces

### BlackBerry Key2 / Clicks Communicator: field terminal

Primary role: fast on-the-go capture and imperative orchestration.

What it is good for:

- general chat when away from a workstation
- “tell the orchestrator what I mean” capture
- connecting to an already-running workstation/session
- checking status of agents, tasks, and current turn
- issuing short imperative commands
- reviewing small summaries and deciding next actions

What it should avoid:

- multi-pane overview
- drag/drop project management
- visual diagram editing
- large rendered documents
- continuous telemetry that burns battery or attention

Interface shape:

- one-column transcript
- bottom composer optimized for hardware keyboard
- compact status strip: link, session, turn, model
- command palette reachable by shortcut
- quick actions: attach, summarize, cancel, route, hand off
- optional lite mode that removes gradients, blur, side cards, and non-essential state

Recommended shortcuts:

| Shortcut | Action |
| --- | --- |
| Enter | send if draft is single-line mode |
| Shift+Enter or Ctrl+Enter | newline |
| Esc | cancel current turn or close command palette |
| `/` | command palette |
| `@` | target session/agent/task |
| `#` | target task/card/tag |

Orchestration model:

1. User captures rough intent.
2. Orchestrator asks at most one clarifying question if necessary.
3. Orchestrator selects or spawns the appropriate subagent/session.
4. Phone receives a terse handoff confirmation and later status summary.

### Lenovo Legion Tab Y700 Gen 5: command plane

Primary role: high-performance multi-session supervision, steering, annotation, and lightweight project management.

Why it matters:

- 8.8 inch size is large enough for real spatial context but small enough to demand discipline.
- High refresh and strong hardware make it viable for drawers, split panes, stylus/touch annotation, and live multi-session status.
- Portrait and landscape are both useful, but should emphasize different jobs.

#### Portrait mode

Best for:

- reading and annotating a single transcript, document, or artifact
- steering one session deeply
- capturing intent while holding the tablet
- reviewing a task/card and associated evidence

Layout:

```text
status strip
primary transcript/artifact
composer or annotation input
bottom sheet: actions / references / cards
```

#### Landscape mode

Best for:

- multi-session overview
- command plane
- project board plus selected card detail
- artifact review with side annotations
- diagram/canvas collaboration

Layout:

```text
left rail: sessions / agents / project
center: selected transcript, artifact, board, or canvas
right drawer: details, annotations, plan, commands
bottom rail: composer, quick prompts, status pulses
```

Suggested drawers:

| Drawer | Purpose |
| --- | --- |
| Sessions | attach, switch, watch running turns, pop out/handoff |
| Agents | swarm status, assignments, blockers, role summaries |
| Intent | capture and refine goals before dispatch |
| Project | cards, lanes, priorities, milestones, issue/PR links |
| Artifacts | files, diffs, screenshots, rendered docs, evidence packs |
| Annotations | comments, change requests, approvals, questions |
| Canvas | diagrams, touch marks, screenshot overlays, architecture sketches |
| Models | model/provider selection and capability hints |

Implementation notes:

- Start with CSS drawers and plain ArrowJS state. Avoid a component framework until a measured bottleneck demands one.
- For markdown editing, begin with a durable textarea plus preview. Add CodeJar only for lightweight code editing. Treat Milkdown as an optional richer editor for tablet/desktop after measuring bundle size and offline behavior.
- For canvas, start with SVG or a simple absolute-positioned annotation layer over images/docs. Do not begin with a full whiteboard dependency.
- For project boards, Backlog.md task files can be the local-first substrate. Drag/drop should mutate status and ordinal, then persist to repo-backed task metadata.
- Every drawer state should be serializable so tablet crashes or browser reloads do not lose work.

### Desktop/laptop full-screen web: review table

Primary role: control-plane, review, planning, annotation, and visual coordination. The TUI remains the fastest direct chat/coding environment.

What desktop web should be good for:

- reviewing artifacts, diffs, screenshots, rendered markdown, plans, and diagrams
- annotating code and docs with richer spatial context
- supervising multiple sessions or swarms at a glance
- planning work in boards and linked documents
- summarizing progress for commits, PRs, and roadmap docs

What it should not try to replace first:

- the TUI as the primary coding chat surface
- native IDE/editor workflows
- OS-level window management

Shape:

- full-screen board/review workspace
- session/status strip across top or left
- artifact viewer in the center
- annotation/task drawer on the side
- command palette for routing actions to agents/sessions
- handoff controls to TUI, tablet, or phone

## Architecture direction

### Surface capability handshake

Clients should eventually advertise capabilities so agents and orchestrators can tailor output.

Example capability shape:

```json
{
  "surface": "y700-web",
  "viewport": { "width": 2560, "height": 1600, "orientation": "landscape" },
  "input": ["touch", "stylus", "keyboard"],
  "features": ["multi_session", "drawers", "annotation", "canvas", "drag_drop"],
  "constraints": { "lite_mode": false, "offline_cache": true }
}
```

A Key2 might advertise `keyboard`, `single_column`, and `lite_mode`. The orchestrator can then send summaries and simple choices instead of diagrams or boards.

### Shared design token package

Create one lightweight token file before duplicating CSS across prototypes:

```text
web/shared/jcode-design-tokens.css
web/shared/jcode-icons.js
web/shared/jcode-surface-primitives.js
```

Initial scope:

- font family variables
- semantic color variables
- radii, spacing, shadows, focus rings
- icon registry for the small Phosphor subset
- helpers for status pills, drawers, rails, cards, transcript entries

### Repo-backed synchronization

For now, prefer repo-backed state over app-specific backend state when the object belongs to a project.

| Object | First durable home |
| --- | --- |
| Project cards | Backlog.md tasks |
| Design docs | `docs/` markdown |
| Architecture diagrams | markdown Mermaid or SVG files in repo |
| Code annotations | structured sidecar or task references, later PR comments |
| Screenshots/evidence | evidence pack/artifact storage path |
| Session transcripts | jcode session store, exported summaries in docs/tasks when needed |

### Event model

A future web control plane should not parse only transcript events. It should have explicit events for project and artifact actions.

Candidate event classes:

- `session.*`: attach, detach, renamed, active, idle, running, errored
- `turn.*`: started, token, tool, checkpoint, completed, cancelled
- `agent.*`: spawned, assigned, blocked, reported, stopped
- `artifact.*`: created, updated, opened, annotated
- `task.*`: created, moved, edited, completed
- `surface.*`: handoff requested, capabilities changed, viewport changed

## Near-term implementation slices

1. **Typography/token pass for `web/jcode-mobile`.** Add font tokens, migrate current raw font stacks to Space Grotesk / Geist / Geist Mono fallbacks, and preserve zero-build operation.
2. **Phosphor subset.** Add a tiny inline SVG icon helper and replace any emoji-like affordances or text-only status glyph gaps with named icons.
3. **Key2 lite mode.** Make a true low-resource single-column mode: no blur, minimal metrics, keyboard shortcuts, transcript-first height, reduced CSS effects.
4. **Y700 drawer prototype.** Add one right-side drawer system that can switch between Sessions, Artifacts, Project, and Annotations without adding a framework.
5. **Repo-backed cards spike.** Render Backlog.md tasks as cards, allow lane/order edits, and write changes back through a safe server/tool path.
6. **Annotation primitive.** Support text annotations on a transcript message or artifact preview, with a serializable local schema and export path.
7. **Surface handoff.** Add command verbs for “continue this on tablet”, “send summary to phone”, and “open artifact on desktop”.

## Open questions

- Should the web client vendor fonts immediately, or only define tokens until the PWA installable shell exists?
- Should Source Serif 4 be included in the default bundle, or loaded only for reading mode?
- Should Backlog.md be the canonical project board format for all prototypes, or should the board first read-only mirror existing tasks?
- What is the smallest annotation sidecar format that can later map to PR review comments, Plannotator notes, and Backlog tasks?
- Should the orchestrator live as a distinct server/session role, or as a protocol convention over existing swarm/session primitives?

## Design posture

The right target is not “mobile chat app” or “admin dashboard”. It is a family of precise instruments:

- the **Key2** captures intent and delegates.
- the **Y700** steers and arranges live work.
- the **desktop web surface** reviews and annotates.
- the **TUI** remains the coding cockpit.

If the basics stay coherent, each device can specialize without fragmenting the jcode experience.
