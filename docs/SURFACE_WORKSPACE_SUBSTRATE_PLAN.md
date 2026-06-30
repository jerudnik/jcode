# Surface Workspace Substrate Plan

Status: Draft 2026-06-30

Related docs:

- [`PERSONAL_INTERACTION_SURFACES.md`](./PERSONAL_INTERACTION_SURFACES.md)
- [`INTERACTION_SURFACE_REQUIREMENTS.md`](./INTERACTION_SURFACE_REQUIREMENTS.md)

This plan replaces the earlier idea of adapting Backlog.md for cards. For now, cards, documentation, and annotations should use a jcode-native surface substrate that matches jcode's values: optimized, performant, functional, inspectable, local-first, and bespoke minimal.

The goal is one coherent surface for:

- cards
- documentation notes
- annotations
- artifact references
- intent capture
- future diagram/canvas marks

The implementation should not start from a task-manager format. It should start from the smallest shared object graph that can render multiple views.

## Research summary

### External research

#### W3C Web Annotation

Source: <https://www.w3.org/TR/annotation-model/> and <https://w3c.github.io/web-annotation/selector-note/index-respec.html>

Useful ideas:

- annotations have a **body** and a **target**
- targets can point at whole resources or selected regions
- selectors cover text quote, text position, fragment, CSS, XPath, data position, SVG, and range selections
- applications can use plain JSON object shapes without needing to care about RDF/JSON-LD in practice

What to borrow:

- body plus target mental model
- selector vocabulary for anchoring annotations
- multiple target kinds for files, images, DOM, SVG, and text

What not to borrow in P0:

- JSON-LD context machinery
- RDF identity model
- broad interoperability requirements
- complex selector combinations before we have real annotation workflows

#### Local-first software

Source: <https://www.inkandswitch.com/local-first-software/>

Useful ideas:

- user ownership matters
- work should survive offline use and service failure
- cloud/server sync should improve reach, not own the truth
- local state should be usable and durable before collaboration exists

What to borrow:

- local writes first
- explicit sync/export later
- durable user-owned files
- no cloud requirement for useful work

What not to borrow in P0:

- CRDT infrastructure
- peer-to-peer collaboration
- full multi-device conflict resolution

#### Editor and canvas package footprint

Observed package signals:

- CodeJar advertises a 2 KB embeddable editor and its npm package is small. It is a plausible optional P1 enhancement for code-like editing.
- CodeMirror 6 is modular and mature, but its basic package pulls several editor modules. It is useful later when editing demands justify it.
- Milkdown brings a ProseMirror and remark stack. It is too heavy for P0 operational surfaces.
- tldraw is powerful, but the npm package is large and pulls React/Tiptap/editor dependencies. It is not aligned with P0 featherweight goals.

Default decision:

- P0 uses `textarea`, markdown preview, native selection APIs, CSS, SVG, and Pointer Events.
- P1 may add CodeJar for code-ish fields if textarea ergonomics fail.
- P2 may evaluate heavier editors only after concrete workflows demand them.

### Internal jcode research

Relevant existing patterns:

- `jcode-storage` already has atomic JSON writes, fast JSON writes, corrupt-primary recovery from `.bak`, and JSONL append helpers.
- `side_panel` persists a compact `index.json` plus markdown page files under `~/.jcode/side_panel/<session_id>/`.
- side-panel pages are small serde structs with `Managed`, `LinkedFile`, and `Ephemeral` sources.
- side-panel state already appears in history and `side_panel_state` protocol events.
- the web gateway is a newline-delimited JSON bridge over WebSocket, so new surface events can remain simple request/event objects later.
- `jcode-task-types` has goal and todo types, but those are not a unified card/document/annotation substrate.

What this suggests:

- Build a jcode-native substrate, not an adapter to another task system.
- Use small serde types, compact JSON indexes, markdown bodies, and JSONL events.
- Start client-local in the web prototype, then lift the same object model into a Rust crate and server storage path.
- Keep every object human-readable or trivially exportable.

## Thesis

Cards, docs, and annotations are not three systems. They are one object graph viewed three ways.

- A **card** is a surfaced work object with status, priority, body, acceptance criteria, and links.
- A **doc note** is a markdown body object with links and optional targets.
- An **annotation** is a body object anchored to a target.
- An **artifact reference** is a pointer to a file, diff, image, transcript message, generated output, or external URL.

Views should be derived:

- board view = objects of kind `card`, grouped by `status`
- annotation view = objects of kind `annotation`, grouped by target or status
- docs view = objects of kind `doc`, ordered by workspace and links
- artifact review view = selected artifact plus linked annotations/cards/docs
- intent view = captured intent objects that are not routed or resolved

This keeps the UI fast and minimal because there is one store, one link model, and many simple renderers.

## Name and scope

Working name: **surface workspace**.

A surface workspace is a local-first object graph for one project, session group, or review context. It is not a global project-management tool. It is the substrate behind web/tablet/desktop review surfaces.

Non-goals for P0:

- Backlog.md adapter
- GitHub Issues sync
- PR comment sync
- CRDT collaboration
- full rich-text editor
- full whiteboard
- multi-user permission system

## Core data model

### SurfaceWorkspace

```json
{
  "schema_version": 1,
  "workspace_id": "sw_...",
  "title": "jcode surfaces",
  "scope": {
    "kind": "project",
    "root": "/Users/jrudnik/labs/jcode"
  },
  "created_at": "2026-06-30T17:00:00Z",
  "updated_at": "2026-06-30T17:00:00Z",
  "active_view_id": "view_board_default",
  "views": []
}
```

### SurfaceObject

Every card, doc note, annotation, intent, artifact reference, and canvas mark is a surface object.

```json
{
  "id": "obj_...",
  "kind": "card",
  "title": "Add Key2 lite mode",
  "status": "todo",
  "priority": "high",
  "body_ref": "body/obj_....md",
  "created_at": "2026-06-30T17:00:00Z",
  "updated_at": "2026-06-30T17:00:00Z",
  "created_by": "user",
  "tags": ["surface", "key2"],
  "targets": [],
  "links": [],
  "fields": {}
}
```

Allowed object kinds for P0:

```text
card
doc
annotation
intent
artifact_ref
```

Reserved for later:

```text
canvas_mark
diagram_node
diagram_edge
review
handoff
```

### Body storage

Body content is markdown by default. Keep it outside the object index so object metadata can be loaded quickly.

Rules:

- cards MAY have markdown bodies
- docs MUST have markdown bodies
- annotations SHOULD have short markdown bodies
- artifact refs usually do not need bodies
- intent objects MAY have markdown bodies if longer than a short field

### Target

Targets borrow the W3C selector idea but use compact jcode-specific JSON.

```json
{
  "kind": "file_range",
  "uri": "repo://docs/file.md",
  "selector": {
    "type": "text_position",
    "start": 120,
    "end": 240
  },
  "fallback": {
    "type": "text_quote",
    "exact": "The selected text",
    "prefix": "before ",
    "suffix": " after"
  }
}
```

P0 target kinds:

```text
workspace
session
message
file
file_range
artifact
url
```

P1 target kinds:

```text
image_region
svg_region
diagram_node
diagram_edge
dom_selector
```

P0 selectors:

```text
whole_resource
line_range
text_position
text_quote
message_id
```

P1 selectors:

```text
xywh
css_selector
svg_selector
data_position
```

### Link

Links are typed edges between objects and resources.

```json
{
  "kind": "blocks",
  "to": "obj_..."
}
```

P0 link kinds:

```text
relates_to
blocks
blocked_by
parent
child
implements
annotates
created_from
assigned_to
references
supersedes
```

### View

Views are saved projections over the object graph.

```json
{
  "view_id": "view_board_default",
  "kind": "board",
  "title": "Board",
  "filter": { "kinds": ["card"] },
  "group_by": "status",
  "sort": ["ordinal", "updated_at"]
}
```

P0 view kinds:

```text
board
annotation_list
doc_list
artifact_review
intent_inbox
```

## Storage layout

### P0 web-local layout

Use browser-local storage first. Keep it simple and inspectable.

Keys:

```text
jcode.surfaceWorkspace.active.v1
jcode.surfaceWorkspace.<workspace_id>.snapshot.v1
jcode.surfaceWorkspace.<workspace_id>.events.v1
```

Shape:

- `snapshot` contains workspace metadata, objects, views, and compact short bodies
- `events` is an array or newline-separated string of operation records
- large bodies can remain inline in P0, with an implementation cap

P0 limits:

- warn if snapshot exceeds 1 MB
- refuse automatic image/blob storage in localStorage
- export JSON before clearing or compaction

Why localStorage first:

- already used by `web/jcode-mobile`
- no async IndexedDB wrapper needed
- easy to inspect and export
- enough for a first command-plane prototype

When to move beyond localStorage:

- workspace exceeds 1 MB
- image/screenshot bodies are needed
- many annotations make sync/compaction slow
- multi-surface handoff needs server visibility

### P1 server-local layout

Use the existing jcode storage style.

```text
~/.jcode/surface_workspaces/<workspace_id>/
  workspace.json
  objects.json
  events.jsonl
  bodies/
    <object_id>.md
  blobs/
    sha256-...
  exports/
    surface-pack-<timestamp>.json
```

Files:

- `workspace.json`: metadata, views, active view
- `objects.json`: compact object index without large bodies
- `events.jsonl`: append-only operation log using `append_json_line_fast`
- `bodies/*.md`: markdown bodies
- `blobs/*`: screenshots or binary artifacts only when needed
- `exports/*`: explicit user-triggered exports

Use `jcode-storage` helpers:

- `write_json_fast` for snapshots/indexes
- `append_json_line_fast` for operations
- `.bak` recovery through `read_json`
- owner-only permissions for local state directories

### P2 repo export layout

Do not make repo export the source of truth in P0.

When the user asks to commit surface state, export a **surface pack**:

```text
.jcode-surface/
  surface-pack.json
  bodies/
    <object_id>.md
  blobs/
    sha256-...
```

This is a jcode-native format, not Backlog.md. It can later gain import/export bridges, but those are adapters, not the substrate.

## Operation log

Every mutation should be representable as an operation.

```json
{
  "op_id": "op_...",
  "at": "2026-06-30T17:00:00Z",
  "source_surface_id": "surface_y700",
  "kind": "object.patch",
  "object_id": "obj_...",
  "patch": {
    "status": "in_progress",
    "ordinal": 2000
  }
}
```

P0 operations:

```text
workspace.create
object.create
object.patch
object.archive
body.set
link.add
link.remove
view.set_active
view.patch
```

Why an operation log:

- undo is easier
- sync is easier later
- tests can replay state
- handoff can include a small causal trail
- agents can explain what changed

P0 does not need a full event-sourcing engine. It only needs the ability to append operations and rebuild a snapshot in tests.

## Cards as first-class objects

A card is just a surface object with `kind = card`.

Suggested fields:

```json
{
  "status": "todo",
  "priority": "normal",
  "ordinal": 1000,
  "assignee": null,
  "confidence": null,
  "completion_confidence": null,
  "acceptance": ["Keyboard shortcut works", "State survives reload"],
  "definition_of_done": [],
  "validation": []
}
```

P0 card statuses:

```text
inbox
todo
in_progress
blocked
review
done
archived
```

Card design rules:

- cards are small
- body is markdown
- fields are optional
- ordering uses sparse integer ordinals
- drag/drop is just `object.patch` for `status` and `ordinal`
- a card can annotate or be annotated
- a card can link to sessions, artifacts, docs, and other cards

## Docs as first-class objects

A doc is a markdown body object.

Doc types:

```text
note
plan
decision
review
scratch
```

Rules:

- docs are markdown first
- textarea first, preview second
- do not require WYSIWYG in P0
- can be linked to cards and annotations
- can target a file or artifact if it is a review note
- can be exported as standalone markdown later

Relationship to side-panel pages:

- side-panel pages are session-scoped document surfaces
- surface docs are workspace-scoped document objects
- P1 can bridge them by allowing a side-panel page to be imported as a `doc` object or a `doc` object to be opened in the side panel

## Annotations as first-class objects

An annotation is a body plus target.

P0 annotation fields:

```json
{
  "kind": "annotation",
  "status": "open",
  "annotation_kind": "note",
  "severity": null,
  "targets": [],
  "body_ref": "body/obj_....md"
}
```

Annotation kinds:

```text
note
question
change_request
approval
defect
idea
```

Annotation statuses:

```text
open
accepted
resolved
superseded
archived
```

Anchoring strategy:

1. Prefer stable IDs when available, such as message ID, object ID, artifact ID, or file path plus line range.
2. For text, store both position and quote fallback.
3. For images, start with normalized rectangle coordinates in P1.
4. For diagrams, start with source line range or Mermaid node ID when available in P1/P2.
5. If an anchor breaks, keep the annotation and mark target state as `stale`.

## Artifact references

Do not copy large artifacts into the workspace by default. Reference them.

Artifact ref fields:

```json
{
  "kind": "artifact_ref",
  "title": "rendered diagram",
  "artifact_kind": "image",
  "uri": "file:///.../diagram.png",
  "provenance": {
    "session_id": "session_...",
    "message_id": "msg_...",
    "tool_name": "image_gen"
  }
}
```

P0 artifact kinds:

```text
file
message
tool_output
url
```

P1 artifact kinds:

```text
diff
image
screenshot
rendered_markdown
diagram
benchmark
evidence_pack
```

## UI plan

### One store, several views

Use one store with derived selectors:

```text
objects -> cards view
objects -> docs view
objects -> annotations view
objects + targets -> artifact review view
objects + status -> inbox view
```

This avoids duplicated state and accidental divergence.

### Y700 landscape

Recommended first layout:

```text
left rail: views and object filters
center: current view
right drawer: selected object detail
bottom rail: command input and status
```

Views:

- Board
- Docs
- Annotations
- Artifacts
- Intent

### Card UI

- cards show title, status color, tags, blocked marker, linked annotation count
- drag/drop can wait until after command/button move works
- first implementation can use buttons or select controls for status
- pointer-based drag/drop can be hand-rolled later with Pointer Events

### Doc UI

- textarea editor
- preview toggle or split preview on tablet/desktop
- saved local body after debounce
- no rich editor in P0

### Annotation UI

- list by target or open status
- create from selected transcript text, selected artifact, or active card
- annotation body textarea
- link to card action
- resolve action

## Protocol path

P0 can be entirely client-local.

P1 can add generic surface workspace protocol requests:

```text
surface_workspace_get
surface_workspace_apply_ops
surface_workspace_export
```

Events:

```text
surface_workspace_state
surface_workspace_ops_applied
surface_workspace_exported
```

Do not add separate protocols for cards, docs, and annotations until proven necessary. They should all share object operations.

## Rust crate path

When lifted server-side, add a small crate instead of growing unrelated crates:

```text
crates/jcode-surface-workspace-types
```

Types:

- `SurfaceWorkspace`
- `SurfaceObject`
- `SurfaceTarget`
- `SurfaceSelector`
- `SurfaceLink`
- `SurfaceView`
- `SurfaceOperation`

Implementation can live in app/base storage first:

```text
crates/jcode-base/src/surface_workspace.rs
crates/jcode-app-core/src/tool/surface_workspace.rs
```

Only add protocol support after local/server tool storage works.

## Implementation phases

### Phase 0: web-local object store

Tasks:

- add `web/jcode-mobile/surface-store.js`
- define object, target, link, view, and operation shapes in JS
- persist snapshot/events in localStorage
- add import/export JSON
- add Board, Docs, Annotations, and Intent views backed by the same store
- add validation fixture that replays operations into a snapshot

Done when:

- a card, doc, and annotation can be created locally
- moving a card is recorded as an operation
- a doc body survives reload
- an annotation can target a transcript message or artifact ref
- export JSON includes enough state to restore the workspace

### Phase 1: server-local surface workspace

Tasks:

- add Rust serde types
- add server-local storage layout under `~/.jcode/surface_workspaces/`
- add `surface_workspace` tool with status, create, patch, export, and import actions
- publish workspace state over the existing event path
- let web client sync/apply operations when connected

Done when:

- local browser state can sync to server-local storage
- server restart preserves workspace state
- operation replay tests pass
- no Backlog.md or external adapter is involved

### Phase 2: repo export pack

Tasks:

- implement explicit export to `.jcode-surface/`
- implement import from a surface pack
- add optional markdown export for selected docs/reviews
- add artifact reference validation

Done when:

- user can choose to commit a surface pack to the repo
- generated files are human-inspectable
- import preserves object IDs and links

### Phase 3: richer anchors and previews

Tasks:

- image rectangle annotations
- file text quote fallback repair
- Mermaid/source diagram anchors
- diff preview target anchors
- optional CodeJar editor for code-like markdown/code snippets

Done when:

- an annotation can survive small file edits via quote fallback
- image annotations are normalized and resolution-independent
- richer editor remains optional and lazy-loaded

## Why not Backlog.md now

Backlog.md is a task format. The surface need is broader:

- annotations must target arbitrary artifacts
- docs need to be first-class, not embedded task descriptions
- cards need to link to annotations and docs
- artifact review needs target selectors
- tablet UI needs one object graph, not several imported formats

A Backlog.md adapter could still be useful later, but only as import/export. It should not define the core model.

## Default implementation prompt

```text
Implement Phase 0 from docs/SURFACE_WORKSPACE_SUBSTRATE_PLAN.md in web/jcode-mobile. Build a tiny jcode-native surface object store for cards, docs, annotations, intents, and artifact refs. Use localStorage snapshot plus operation log. Add Board, Docs, Annotations, and Intent views backed by the same store. Do not add Backlog.md integration, IndexedDB, CRDTs, Milkdown, tldraw, or a drag/drop dependency.
```

## Recommendation

Proceed with the **surface workspace** substrate.

The first implementation should be boring and fast:

- one in-memory store
- localStorage snapshot
- appendable operation log
- markdown bodies
- compact object metadata
- derived views
- explicit export/import

This gives jcode a native interaction substrate that can grow into cards, docs, annotations, and artifact review without importing another product's mental model.
