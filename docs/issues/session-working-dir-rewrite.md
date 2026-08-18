---
title: "Resuming a session rewrites that session's recorded working directory to the resumer's, permanently and silently"
status: open
priority: medium
owner: unassigned
opened: 2026-08-18
---

# A session's working directory has no owner

## Symptom

Sessions that were created in a working tree are later found rooted somewhere
else. On this host, three persisted sessions read:

| session | recorded working directory |
| --- | --- |
| `owl_…` | `$WORKTREE_PRIMARY` |
| `piglet_…` | `$HOME` |
| `rat_…` | `$HOME` |

The `.json` and its `.bak` agree in every case, so this is not a partial write:
the recorded value *is* `$HOME`, and was written that way.

A session rooted at `$HOME` has no jcode repository above it, so every
repository-relative feature in it fails in a way that looks like a resolver
bug. That is how this was first mistaken for one — see
`selfdev-repo-discovery.md`.

## What is verified

Switching sessions with `/resume` rewrites the *target* session's recorded
working directory to the working directory of the session you were in when you
switched. The change is written to the session file and persists.

The path, end to end:

1. `/resume` opens the session picker (`inline_interactive.rs`), and choosing an
   entry calls `queue_resume_session`.
2. That drains into `Backend::resume_session`
   (`crates/jcode-tui/src/tui/backend.rs:594`), which sends
   `Request::ResumeSession`.
3. The server handles it in
   `crates/jcode-app-core/src/server/client_lifecycle.rs:1773`. It binds

       let resume_working_dir = {
           let agent_guard = agent.lock().await;
           agent_guard.working_dir().map(str::to_string)
       };

   from `agent` — the agent the client is *currently* attached to — and passes
   it as the working-directory override for the session being resumed.
4. `handle_resume_session` forwards it to `restore_session_with_working_dir`
   (`crates/jcode-app-core/src/agent/turn_execution.rs:634`), which, when the
   override is `Some`, overwrites `session.working_dir` and refreshes the
   initial session context.

So the override describes the resumer, but is applied to the resumed. A
session that is live short-circuits earlier through `claim_live_target_agent`
and is unaffected; a dormant one is rewritten.

Reproduced deterministically in
`crates/jcode-app-core/src/server/client_session_tests/resume/dormant_working_dir.rs`.
Two sentinel paths that cannot coincide — `/sentinel/created/here` for where
the session was made, `/sentinel/attached/from` for where the resume comes
from — and two directions:

| resume carries | recorded directory afterwards |
| --- | --- |
| `Some("/sentinel/attached/from")` | `/sentinel/attached/from` — rewritten |
| `None` | `/sentinel/created/here` — preserved |

Both assertions hold, so the first is not satisfied by a resolver that always
writes. Before trusting either, a `compile_error!` was planted in the new file
and the build was confirmed to fail on it: an earlier run of this same test had
reported `0 passed` purely because the name filter did not match, and a green
`0 passed` is indistinguishable from a green pass at a glance.

## What is not verified

That this is how the three sessions above came to be rooted at `$HOME`. The
mechanism is confirmed; this instance of it is not. A plausible propagation —
one session rooted at `$HOME` re-roots every session resumed from it, which
would explain why two sessions share `$HOME` while a third kept its tree — is
consistent with the evidence but was not demonstrated.

An earlier attempt to demonstrate it live, by driving a spawned tester through
`/resume` with injected keys, produced no change in the target. That result is
worthless in both directions: `client:frame` returned `no frames captured`
throughout, so there was never any evidence the picker opened or that anything
was selected. A probe that may never have fired cannot refute anything.

## Why it matters

The working directory is not incidental state. It decides which repository
self-dev resolves, which files relative paths reach, and what the initial
session context describes. Rewriting it changes what a session *is*.

Three properties make this hard to catch:

- **It is silent.** Nothing is written to the transcript, and no notice is
  shown, at the moment a session is re-rooted.
- **It is persistent.** The value is saved. Switching back does not restore it;
  it re-rewrites it in the other direction.
- **It is invisible at the point of failure.** The symptom appears later, in a
  different session, as a feature that cannot find a repository — with nothing
  connecting it to the switch that caused it.

There is a related asymmetry worth noting: `subscribe` *does* warn when a
reconnect changes an established working directory
(`subscribe_warns_when_reconnect_changes_established_working_dir`). The
resume path performs the same kind of change with no equivalent notice.

## Root cause

Not a failure of a mechanism — the absence of one. A session's working
directory has no owner and no provenance. It is a mutable field that any
resume may write, with no record of who wrote it, no signal at the time, and
no way to distinguish "this session was created here" from "the last client to
touch it happened to be somewhere else".

## Possible directions

Not prescriptive; shapes that would close it:

- Treat a session's recorded working directory as owned by the session and set
  at creation, so resuming attaches to it rather than overwriting it.
- If re-rooting is deliberately supported, make it explicit — a distinct
  gesture, not a side effect of switching — and say so at the time.
- Give the resume path the notice the subscribe path already has, so a change
  of this kind is at minimum visible in the session it happens to.
- Record provenance with the value, so a directory that was rewritten can be
  told from one that was chosen.

## Reproduction

    cargo test -p jcode-app-core --lib working_dir

Both `resume_rewrites_a_dormant_sessions_recorded_working_dir` and
`resume_without_an_override_keeps_the_recorded_working_dir` must appear in the
output. If either is missing, the filter did not match and the result means
nothing.
