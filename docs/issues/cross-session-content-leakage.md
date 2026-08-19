---
title: "Cross-session content leakage between concurrent swarm sessions"
status: open
priority: high
owner: unassigned
opened: 2026-08-17
---

# Cross-session content leakage between concurrent swarm sessions

Content originating in one session has appeared in other sessions, including a
session working in a **different repository**. Recorded while three concurrent
sessions were active against `~/labs/jcode` and one against a separate
`nix-config` checkout.

This is deliberately split into what was observed directly and what was
reported, because the two carry different weight.

## Verified directly

1. **`notify` DMs returned success but never became actionable.** Two DMs sent
   at `09:16:29Z` and `09:23:39Z` both reported successful delivery. The
   recipient's next message (`09:31:14Z`) still treated the contents as
   outstanding, and it separately confirmed its copies had been **truncated
   twice**.
2. **Bodies over 240 characters are structurally lossy agent-to-agent.** The
   tool's own rejection of a 247-character body states that recipients see the
   `tldr` collapsed behind an expand control. An agent has no way to expand it,
   so the remainder is unreachable rather than merely unread.
3. **The workaround succeeds.** Writing the full content to a file and sending a
   short `interrupt` DM pointing at that path produced a correct acknowledgement
   naming the file's actual sections. Modality, not content length, was the
   difference.

Detail and a delivery-semantics matrix to fill in:
`docs/issues/swarm-dm-delivery-investigation.md`.

## Reported, not yet reproduced here

4. **A third concurrent session** on its own worktree appeared to leak content
   into the others. Not observed first-hand; no reproduction attempted.
5. **Todo content crossed repositories.** A session working in the user's
   `nix-config` checkout received todo messages originating from the `jcode`
   sessions. This is the most serious item, because it crosses a project
   boundary and not merely a session boundary.

## First lead on (5)

Storage looks correctly scoped, so suspicion falls on delivery/fan-out rather
than persistence:

    crates/jcode-app-core/src/tool/todo.rs:801   load_todos(session)

The read path is keyed by session, so a shared-store explanation is unlikely on
its face. The remaining candidates are a notification or broadcast path that
resolves recipients more widely than the originating session, or a shared UI
surface that renders another session's state. Not investigated further; see the
hard constraints in the companion brief before testing, since the live sessions
must not be disturbed.

## Follow-up evidence for item 5 (2026-08-17, nix-config session, read-only)

The nix-config session the operator was looking at supplies timestamps that
narrow item 5 considerably. All file paths under `~/.jcode/`.

1. **The affected session's store was provably empty at question time.** The
   operator asked "are those your ToDos?" at `09:34:43Z`. The session's todo
   file (`todos/session_tulip_1786957837407_383165b29b9720d0.json`) did not
   exist yet — its first write is `09:38:52Z` — and the todo tool read back
   `[]` at `09:34:58Z`, fifteen seconds after the question. The sibling
   nix-config session (`session_sunflower_*`, spawned 0.6 s earlier) has never
   had a todo file. No nix-config session had todo state to render.
2. **The only freshly-written todos matching the observation were from a jcode
   session.** `todos/session_rose_1786957987850_ed471b6456696972.json`
   (mtime `09:30:36Z`, 4 min 7 s before the question; session cwd
   `$WORKTREE_PRIMARY`) contains an "adversarial review" plan for
   D033/F23/D034 — unmistakably jcode-repo work. Older sessions (piglet, crab,
   cactus) also predate the question but by 20+ minutes.
3. **Storage and agent-tool paths verified correctly scoped.**
   `jcode-base/src/todo.rs:239` `todo_path()` resolves
   `~/.jcode/todos/<session_id>.json`; the tool reads and writes via
   `ctx.session_id` (`tool/todo.rs:343,372`). A storage-side explanation is
   now excluded by direct inspection, not just "unlikely on its face."
4. **The render path contains two pointers that can disagree with the actual
   session.** `state_ui.rs:81-87` `active_client_session_id()` returns
   `self.remote_session_id` whenever `self.is_remote` — a stale or misrouted
   remote pointer silently switches whose todos are loaded
   (`todos_view.rs:42-44`). Separately, `note_client_focus`
   (`state_ui.rs:89-104`) persists a *global* "last focused session"
   (`dictation::remember_last_focused_session`). And
   `build_todos_view_markdown` (`todos_view.rs:296-304`) labels whatever it
   renders as "this session", so a misrouted render is indistinguishable from
   a correct one at the UI. This upgrades "a shared UI surface" from
   speculation to the only remaining code path consistent with facts 1-3.

Not pinned: which surface the operator was actually looking at (local pane vs
remote-attached), so the remote-pointer vs last-focused-pointer split is still
open. Requires the operator's recollection of the pane or a repro before
assigning blame between the two candidates.

## Why these are grouped

All five are the same failure shape: a message is *delivered* by the system's own
account while not *arriving* in the place that would act on it, or arriving
somewhere with no business receiving it. Success is reported by the sender's
side, so nothing in either session's view indicates a problem. Note that item 1
was only caught because the recipient happened to restate its outstanding items;
a less chatty recipient would have left the sender believing the exchange had
landed.

## Related

- `docs/issues/swarm-dm-delivery-investigation.md` — the plan-only investigation
  brief covering items 1-3, with a standard of proof requiring the fix be
  demonstrated failing first.
- `docs/issues/shared-git-hooks-couples-worktrees.md` — a non-messaging instance
  of concurrent sessions sharing state they appear not to share.

## Design direction: immutable session identity and observation boundaries

Carried from a proposal that is otherwise retired. Its name-ambiguity item
already shipped: direct messages fail closed on an ambiguous name. What
remains unbuilt is the render-cache half, and a current regression test
confirms it — a session switch leaves the prior session's diagram in a
process-global registry and still displays it in the pinned pane.

Every operator-facing view must bind to one immutable `session_id`. A viewer
may show a friendly name as a convenience, but it must never select, retarget,
or continue rendering a session by name, list position, recent activity, or a
global last-focused pointer. If the bound session terminates, the view must
show its terminal state rather than silently switching to another session.

Session switches must clear or namespace every session-derived render cache,
including diagrams, todo views, status snapshots, and focus state. Incoming
events must be checked against the bound session before they update a view.
Actions must use the exact session id; a friendly name matching more than one
relevant session must fail with the candidate ids.

Observation and execution must be separate. Opening a viewer must not claim a
member, overwrite its recorded working directory, interrupt its turn, or make
coordinator status and summary requests unavailable.

Acceptance tests should prove that a session switch cannot display an earlier
session's diagram, todo, transcript, or status; that a namesake cannot replace
a terminated bound session; that ambiguous actions fail closed; and that
attaching a viewer preserves the observed session's working directory and
coordinator visibility.
