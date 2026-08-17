---
status: open
priority: high
owner: maintainers
opened: 2026-07-31
---

# Remote sessions stall on "loading session…" because a 3s cache TTL is shorter than the build it caches

Reported by human (2026-07-30/31) during remote-session use: the client sat on
`loading session…`, then advised `/restart`, while the server was still working
normally and would have answered.

Full working notes: `~/.jcode/pending/remote-loading-session-rootcause.md`.
This document is the durable summary. **Root-caused, not fixed** — no code
changed, pending a decision on which of the fixes below to take.

## Symptom

A remote client shows `loading session…` for ~20s and then reports the session
as unavailable, suggesting `/restart`. The server is healthy throughout; the
request it is blocked behind eventually completes.

## Root cause

```
3s ROUTES_MEMO_TTL expires  →  cold model_routes rebuild (8-17s) runs INSIDE Subscribe
   →  the sequential request queue blocks a 27ms GetHistory behind it
   →  the client watchdog's 21s budget expires
   →  the user is told to /restart while the server is still working
```

The memo that is supposed to prevent the expensive rebuild expires faster than
the rebuild takes, even in the warm case:

```rust
        const ROUTES_MEMO_TTL: std::time::Duration = std::time::Duration::from_secs(3);
```

`crates/jcode-base/src/provider/mod.rs:529`, consumed at line 536:

```rust
                && entry.built_at.elapsed() < ROUTES_MEMO_TTL
```

Measured build times against that 3-second TTL:

| condition | observed |
|---|---|
| warm rebuild | 2393 ms, 3801 ms |
| cold rebuild | 8275 ms, 14699 ms, 17039 ms |

A warm rebuild at 3801 ms already exceeds its own 3000 ms TTL, so the memo can
be stale before it is ever reused. The cache is therefore near-useless under
exactly the conditions it exists to protect, and the cost lands on the
interactive path because the rebuild happens inside `Subscribe`, ahead of a
sequential queue.

The 21s client watchdog is not the bug but sets the deadline: a 17s cold build
plus normal work crosses it, so the failure is presented to the user as a dead
session rather than a slow one.

## Suggested fixes (independent; 1 is the minimum)

1. **Raise `ROUTES_MEMO_TTL` above the p99 build time.** A cache whose TTL is
   shorter than the value it caches cannot amortize anything. Any value that
   exceeds the observed 3801 ms warm case restores the intended behaviour;
   sizing it against cold builds (17s) is better still.
2. **Do not rebuild routes inside `Subscribe`.** Serve the stale entry and
   refresh in the background, so an expiry never blocks an interactive
   subscribe.
3. **Do not let one slow build block unrelated cheap requests.** A 27ms
   `GetHistory` queued behind a 17s rebuild is the proximate cause of the
   user-visible stall.
4. **Report honestly when the watchdog fires.** "Still starting up" is
   different from "session unavailable, /restart"; the current advice is wrong
   and encourages an action that discards a working session.

## Not inspected

* Why cold builds vary from 8s to 17s, and whether that is network-bound
  (provider metadata fetches) or local.
* Whether the 21s watchdog budget is itself well chosen.
* Whether any other call path depends on `ROUTES_MEMO_TTL` being short.
