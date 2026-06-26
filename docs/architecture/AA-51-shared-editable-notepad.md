# AA-51: Shared Editable Plan/Notepad Surface with Per-Block Metadata

Date: 2026-06-26
Status: design (editable-surface design may proceed; feedback-into-turn gated on AA-49)
Pad: AA-51 (milestone). Blocked-by AA-49 for the feedback path only.

## Motivation (the real asymmetry)

The human cannot hold as much context as the agent. The agent produces enormous dense markdown that is hard to navigate, often buries the point, and is sometimes wrong, but correcting it after the fact feels like too much work. The fix is a **real-time, mutually-writable surface** where a human-in-the-loop reads and amends as content is created: strike false info, cut tangents, redirect, before it ossifies into a 2000-line file.

## Design principle (anti-anthropomorphizing)

Do not pretend the agent "understands" the doc like a human. Make the notepad a **shared, typed artifact**: the human's edits/annotations become structured context the agent ingests; the agent's structure becomes something the human can read and amend. Map human-understanding <-> agent-understanding through a shared typed surface, not assumed shared cognition.

## Cousin-project thesis (Vellotype block metadata)

Wiki/bullet-journal tools over-focus on **interlinking** and under-invest in **rigorously recording metadata about the minimum viable unit of thought (the block)**. The associative work (linking) is the fun part; the **analytical work (the disciplined per-block log) is what enables good associative thinking later.** So: **type the block-level invariants; free-form the prose.** (Sibling of bespoke-pad's "type the invariants, free-form the story.")

## What exists today (verified; gap is small)

- **Side-panel surface is general and persisted:** `SidePanelPage { id, title, file_path, format: Markdown, source: Managed|LinkedFile|Ephemeral, content, updated_at_ms }` (crates/jcode-side-panel-types/src/lib.rs). Multi-page, persisted (`PersistedSidePanelState`). But **display-only**: build markdown -> render, no in-place edit.
- **An editable-text primitive exists:** the input box (`ui_input.rs`, ~2.6k lines) is homegrown cursor/wrap/edit handling to borrow from (not tui-textarea).
- **Markdown render exists:** `jcode-tui-markdown`. Plan/todo display: `todos_view.rs`, `info_widget_todos.rs`.

So surface + render + persistence + an editable primitive **all exist; they have never been combined into an editable, agent-and-human-writable page.** That is the whole milestone.

## Scope (phase into child items)

1. **Editable side-panel page mode:** human edits/annotates markdown in place; persisted. Reuse the `ui_input.rs` edit primitive against a `SidePanelPage.content`. Pure local UX, no agent dependency.
2. **Agent read/write tool over the same document:** one canonical artifact, both parties write. A tool that reads and patches the focused editable page.
3. **Annotation primitives:** accept/reject/strike a block, "this is wrong", "cut this", "do X first", captured as **structured marks**, not free text.
4. **Per-block metadata (Vellotype thesis):** a minimal typed log per block (status, confidence, source, keep/cut), distinct from interlinking. This is the load-bearing typed layer; prose stays free-form.
5. **Verbosity/communication guardrails:** budgets + prompts constraining how much/what/why gets recorded. The surface discourages sprawl and requires load-bearing facts captured tersely; a budget, not a blank canvas.
6. **Feedback into the turn loop (GATED on AA-49):** human annotations become next-turn dynamic context. This is exactly the outcome-reactive injection AA-49 researches; it ships only behind the AA-49 §6 replay/eval harness and the damping discipline (§3.2), because feeding human marks back per turn is a reactive-steering channel.

## Ordering and the AA-49 gate

Items 1-5 are **not** outcome-reactive and may proceed now: an editable, mutually-writable, per-block-typed surface is plain UX + persistence + a tool, validated by render/round-trip/tool tests. Item 6 (marks feeding the turn) is the only reactive piece and is gated. Per AA-49 §2, human corrections are an **external verifiable signal** (the safe kind), but the *mechanism* (per-turn injection of human marks) still needs the §6 measurement and §3.2 damping before it ships, so it does not silently degrade.

## Block model (typed invariants, free-form prose)

A block carries free-form markdown plus a small typed record:

```
Block:
  id            stable
  text          free-form markdown (the prose)
  status        draft | kept | struck | cut | needs-rework
  confidence    optional 0..1 (who/what asserted it)
  source        human | agent | tool:<name>
  marks         [ accept | reject | strike | "do X first" | ... ]  (structured)
```

Marks and per-block metadata serialize alongside the page content, so a human's
"this is wrong / cut this / do X first" becomes structured context (and a
candidate human-labeled good/bad signal for AA-42 TaskOutcome / AA-48 evidence),
not lossy prose.

## Smallest safe prototype

**An editable, persisted single side-panel page (item 1) with a block-status mark (subset of item 3): human can strike/keep a block in place, and it persists.** No agent write, no per-turn feedback. This proves the surface is genuinely mutually-editable and that structured marks round-trip, with zero reactive risk. Agent write (item 2), full per-block metadata (item 4), guardrails (item 5), and the gated feedback path (item 6) follow as separate child items.

## Child items (to create under AA-51)

1. **AA-51a Editable side-panel page**: in-place markdown edit on a `SidePanelPage`, persisted; TestBackend render + round-trip persist test. (Smallest prototype.)
2. **AA-51b Agent read/write tool** over the focused editable page; tool test.
3. **AA-51c Annotation marks + per-block metadata**: typed block record (status/confidence/source/marks), serialize; round-trip test.
4. **AA-51d Verbosity guardrails**: budget + prompts constraining recorded content; render/behavior test.
5. **AA-51e Feedback into the turn (GATED on AA-49 §6)**: human marks -> next-turn dynamic context, damped per AA-49 §3.2, measured by the replay harness. Do not start before the AA-49 harness exists.

## Validation

- Evidence: per child item — TestBackend full-frame renders of the editable page, round-trip persist test, agent read/write tool test, annotation-marks serde test, `debug_socket` visual frames.
  Proves: the page is mutually editable and persisted; marks/per-block metadata round-trip; the agent and human write one canonical artifact.
  Limit: does not prove the feedback-into-turn path improves outcomes (gated on the AA-49 harness); does not prove the verbosity guardrails actually reduce sprawl in practice (needs real-session observation).
