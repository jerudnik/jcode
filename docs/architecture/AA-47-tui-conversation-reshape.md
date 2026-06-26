# AA-47: Reshape TUI for Conversation (Typesetting, Theming, Collaboration Handles)

Date: 2026-06-26
Status: design (decompose into child items; name the smallest safe prototype)
Pad: AA-47 (milestone). Relates to AA-46 (mode), AA-43 (evidence panel), AA-51 (editable surface).

## Goal

Make an assistant session read as a **conversation**, not a command/ack/praise loop. The biggest, most open-ended UX track: typesetting, information display, and new "handles" for human-agent collaboration.

## Framework reality (verified, sets the boundaries)

- Stack is **ratatui 0.30 + crossterm 0.29**, split across many `jcode-tui-*` crates (markdown, messages, render, style, tool-display, anim, mermaid, session-picker, visual-debug, ...).
- It is a **cell-grid TUI**: "fonts" are the terminal emulator's font, not app-controlled. We control **typesetting** (layout, spacing, glyphs, color via `jcode-tui-style`) and markdown rendering (`jcode-tui-markdown`). True typeface control would need user terminal config or a non-TUI surface (desktop crate is out of session scope).
- **Theming today is hardcoded** functions in `jcode-tui-style/src/theme.rs` (`user_color`/`ai_color`/...) + `color.rs` truecolor-vs-256 quantization. **No user-facing theme config exists** yet; adding one is itself a sub-task.
- Some levers already exist: `MarkdownSpacingMode`, `ReasoningDisplayMode` (config-types). These are the seams to extend, not rebuild.

## The reframe: render density follows mode

AA-46 shipped a real `AssistantMode` (execute | converse). AA-47's spine is: **converse-mode sessions render differently from execute-mode** (lighter chrome, prose-forward layout, distinct treatment for "thinking out loud"/proposal turns vs tool-execution turns). Mode is the existing, inspectable signal that drives the visual difference, so the milestone has a concrete acceptance test (converse vs execute frames differ).

## Candidate sub-tracks (each becomes a child item)

1. **Conversational turn shape**: lighter chrome for chat vs work; distinct visual treatment for proposal/think-out-loud turns vs tool-execution turns. Driven by `AssistantMode`. (This is the load-bearing track and the smallest prototype.)
2. **Typesetting/markdown**: a "document"/prose-forward layout for converse mode, extending the existing `MarkdownSpacingMode`/`ReasoningDisplayMode` seams rather than a new renderer.
3. **Theme config surface**: promote `theme.rs` hardcoded colors into a configurable theme (config-types + `DisplayConfig`), enabling per-profile palettes (infra vs jcode visually distinct). Respect existing truecolor/256 quantization in `color.rs`.
4. **Collaboration handles**: inline proposals the human can accept/amend, clarifying-question prompts, side-by-side plan/diff panels. Overlaps AA-43 (evidence detail panel) and AA-51 (editable surface); share panel infra, do not duplicate.

## Smallest safe prototype

**Mode-driven turn density (sub-track 1), one visible difference.** Concretely: in converse mode, render proposal/conversational assistant turns with lighter chrome than execute-mode tool-execution turns, proven by a `TestBackend` full-frame render that differs between a converse-mode and execute-mode session. No theme config, no new panels. This anchors the whole milestone in a measurable visual delta with the least surface area, and it directly exercises the AA-46 `mode` field end to end into rendering.

## Child items (to create under AA-47)

1. **AA-47a Mode-driven turn density**: converse vs execute render difference for conversational vs tool turns; TestBackend frame-diff test. (Smallest prototype, ship first.)
2. **AA-47b Prose-forward typesetting for converse mode**: extend `MarkdownSpacingMode`/reasoning display for a document layout; render test.
3. **AA-47c Theme config surface**: `theme.rs` colors -> configurable theme in config-types/`DisplayConfig`; per-profile palette; render test honoring color quantization.
4. **AA-47d Collaboration handles**: inline accept/amend proposals + clarifying-question affordance; share panel infra with AA-43/AA-51; render + interaction test.

## Validation

- Evidence: per child item — `TestBackend` full-frame renders per sub-track, `debug_socket` visual frames, `cargo test` tui.
  Proves: converse-mode sessions visibly differ in layout/density; a theme config surface exists and changes rendered colors; collaboration affordances render and accept input.
  Limit: cell-grid renders prove layout/glyph/color, not true typeface; visual "feels like a conversation" is a human judgment the frame-diffs approximate but do not fully capture.
