# Session Lifecycle and Remote Client Roadmap

## Motivation

Remote and mobile clients make the current session model more visible. A phone or tablet needs a clear answer to a few questions:

- Which sessions are actually live right now?
- Which sessions are safe to resume from history?
- Which sessions are background/headless workers?
- Which sessions are part of the same project, swarm, or task?
- Which sessions should be archived out of the default picker?

The current implementation mixes several concepts: persisted session status, `active_pids` process ownership markers, swarm member status, connected client state, and restore/reload recovery state. That works for a local TUI, but remote clients need a cleaner source of truth.

## Near-term invariant

`active_pids` should mean live ownership only. It should not be used as durable history, picker membership, or restart intent.

Recommended invariant:

```text
active_pids/<session_id> exists iff a live process/runtime currently owns that session.
```

If a startup/reload path examines a stale marker and decides not to resume or attach a runtime agent, it must remove the marker before returning.

## Proposed domain model

Split session state into explicit layers:

1. **Transcript**
   - Immutable-ish conversation and tool history.
   - Stored under `sessions/`.
   - Can be archived, searched, imported, or rehydrated.

2. **Runtime presence**
   - Live process ownership and streaming state.
   - Backed by `active_pids`, `streaming_pids`, and server in-memory client maps.
   - Ephemeral and aggressively self-healing.

3. **Workspace grouping**
   - Project/worktree/swarm/task group.
   - Drives picker sections and remote client tabs.
   - Should be derived from working dir, git root, swarm id, parent id, and optional user labels.

4. **User-facing identity**
   - Stable display title, memorable short name, emoji/icon, and optional saved label.
   - Should be decoupled from the storage id.
   - Storage id remains globally unique and non-user-facing.

5. **Restore intent**
   - Explicit queued desire to rehydrate after reload/crash/restart.
   - Should be separate from `active_pids`.
   - Examples: reload recovery intent, restart snapshot, scheduled wake, remote reconnect token.

6. **Archive policy**
   - Hide from default picker, keep searchable.
   - Retain important/saved sessions, summarize low-value sessions, optionally prune large transient logs later.

## Naming and display proposal

Keep the current memorable animal ids for human-friendly handles, but introduce a richer display record:

```json
{
  "session_id": "session_crocodile_...",
  "short_name": "crocodile",
  "title": "Build Android gateway client",
  "display_title": "Android gateway client",
  "group": {
    "kind": "project",
    "id": "/Users/jrudnik/labs/jcode/.git",
    "label": "jcode"
  },
  "presence": "running|ready|closed|crashed|archived",
  "activity": "streaming|idle|waiting|detached",
  "client_count": 1,
  "saved": false,
  "archived": false
}
```

Remote clients should render this record directly instead of reconstructing state from raw session files.

## Picker and remote-client UX

Default grouping order:

1. Live attached sessions
2. Running headless/background sessions
3. Recent sessions in current project
4. Saved/bookmarked sessions
5. Recently crashed/recoverable sessions
6. Archived/search results only

For the BlackBerry Key2 UI, expose a text-first list:

```text
● crocodile  running   jcode   current task...
○ blowfish   ready     jcode   last prompt...
◇ otter      saved     jade    design notes...
```

For tablet/web, use grouped cards with status badges, model/provider, project, token usage, and last activity.

## Rehydration rules

Rehydrate only from explicit intent:

- User opens/resumes a session.
- Remote client reconnects to a session it was attached to.
- Reload recovery has a pending directive.
- Restart snapshot requests restore.
- Scheduled/background work wakes a headless worker.

Do not rehydrate merely because a session has an `active_pids` marker. Instead, stale markers should be treated as cleanup candidates.

## Archive rules

- `closed` sessions older than N days disappear from default picker unless saved, recent in current project, or matching search.
- `archived` means hidden by default but not deleted.
- `saved` sessions are pinned above ordinary history.
- Swarm children can auto-archive after reporting completion, while the coordinator/session summary stays visible.

## Implementation milestones

1. **Presence cleanup**
   - Enforce `active_pids` as ephemeral live ownership.
   - Add cleanup on startup, skipped recovery, failed recovery, and server stop.

2. **Session list API**
   - Add one normalized server-side endpoint/debug command for session cards.
   - Include presence, activity, client count, group metadata, and archive flags.

3. **Picker refactor**
   - Make TUI picker consume normalized cards.
   - Stop each client from doing its own status heuristics.

4. **Remote client integration**
   - Reuse the same session card API for web/mobile.
   - Add explicit resume, detach, archive, save, and rename actions.

5. **Archival and compaction**
   - Add archive flag and default filters.
   - Later add summarization/retention policy for large transient worker sessions.
