# Sessions stall behind a permanent "server binary differs" notice because reload staleness was measured against process start time

Reported by human (2026-08-01): jcode repeatedly printed "Connected server
binary differs from the installed client channel" and "Newer server binary
detected. Auto-reload is disabled", in a loop, while sessions sat on
`Loading session…`.

**Root-caused and fixed** (unlike the other documents here). Recorded because
it is a human-noticed product defect in the W6 family, and because the defect
was a *false invariant written down as a safety comment*, which is worth
keeping.

## Symptom

A pair of notices repeats indefinitely and the session never loads:

```
Connected server binary differs from the installed client channel
Newer server binary detected. Auto-reload is disabled
```

Reloading does not clear it. Neither does restarting the client. The daemon
advertises an update that no action can satisfy.

## Root cause

The canonical publish flow overwrites `~/.jcode/current/jcode` **in place**, so
a running daemon's executable and its reload candidate are the *same canonical
file*. An mtime-vs-mtime comparison is then the file against itself and can
never be strictly newer, so `newer_binary_available` fell back to comparing the
candidate's mtime against the **process start time**.

That fallback was documented as loop-safe on the grounds that process start
time "advances with every exec". It does not. Reload re-execs via
`Command::exec` (`crates/jcode-base/src/platform.rs`), and `exec` **preserves**
the process start time: same pid, same start time, new image.

```
publish overwrites ~/.jcode/current/jcode   (mtime advances)
  →  daemon: candidate_mtime > process_start  →  "update available"
  →  client defers history on every bootstrap (runtime-identity path)
  →  auto_server_reload disabled  →  notice printed, nothing changes
  →  watchdog re-requests history  →  repeat forever
```

Even with auto-reload *enabled* the signal could not clear, because the reload
preserves the very baseline the predicate is measured against.

Verified empirically rather than by reading: a probe calling
`proc_pidinfo(PROC_PIDTBSDINFO)` — the same API the check used — reports a
byte-identical start time before and after `exec`.

```
gen=1 pid=59578 process_start=316675 image_baseline=1785588612
   on-disk mtime after republish = 1785588615
-- republished in place, now exec()ing --
gen=2 pid=59578 process_start=316675 image_baseline=1785588615
```

`process_start` is unchanged across the exec; the image mtime advances. Linux
degrades the same way, since `/proc/self` btime is per-task, not per-image.

## Fix

Baseline on the mtime of the binary image *currently executing*, sampled once
into a `OnceLock` and seeded at server boot. `exec` replaces the process image
and reinitializes statics, so each reloaded image re-samples, the baseline
genuinely advances, and the signal terminates. This is the same pattern the
client already used (`construction.rs::current_binary_mtime`).

Seeding at boot matters: it freezes the value describing the image we booted,
rather than whatever is on disk at the first freshness query, which a
concurrent republish could otherwise race.

Regression test `same_path_republish_clears_once_the_baseline_advances` covers
both halves of the transition — a stale image reports an update, and the
reloaded image does not — because a test that only asserts the first half is
exactly the test that passed while this bug shipped.

## Verification

Unit tests alone cannot exercise `exec` semantics, so the fix was also proven
end to end against a live daemon with a sandboxed `HOME` (making the running
exe genuinely the exec candidate):

| step | `has_update` |
|---|---|
| fresh image | `false` |
| republished in place | `true` |
| after forced reload (**same pid, same start time**) | `false` |

The last row is the configuration that previously latched forever.

## Note

`display.auto_server_reload = false` in the reporter's config made the loop
*visible* rather than silently self-resolving. It is not the bug, and the
underlying defect would still have advertised an unclearable update with
auto-reload enabled.

## Not inspected

* Whether the client-side runtime-identity defer path
  (`server_events.rs`) should itself bound how many times it re-defers, which
  would have converted this infinite loop into a bounded one regardless of the
  server-side cause.
* Whether any other caller depends on the old process-start baseline
  semantics.
* Whether the "differs from the installed client channel" wording is accurate
  when both sides resolve to the same published file.
