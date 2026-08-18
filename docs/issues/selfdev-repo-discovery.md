---
title: "selfdev cannot find the repository it is running in, and `selfdev setup` writes the clone somewhere discovery never looks"
status: open
priority: medium
owner: unassigned
opened: 2026-08-18
---

# `selfdev` repository discovery fails for an installed binary

## Symptom

In a self-dev session started from `$WORKTREE_PRIMARY` but whose *session*
working directory is `$HOME` (see the correction below):

    $ selfdev find-config
    **Repository:** not found (run `selfdev setup`)

    $ selfdev reload
    Could not find jcode repository directory

The repository is right there. `build::is_jcode_repo($WORKTREE_PRIMARY)` is
true: the root `Cargo.toml` carries `name = "jcode"` and `.git` exists.

## What was verified

Checked directly, on this host, at the time of writing:

- `find-config` reports `Repository: not found` while a `jcode` process with
  cwd `$WORKTREE_PRIMARY` is live. (`lsof -a -d cwd -p <pid>`.)
- Following the advice in the message does not fix it. `selfdev setup` had
  already been run with an explicit `--context` naming `$WORKTREE_PRIMARY`;
  it reported success, and `find-config` still reports `not found`.
- `selfdev setup` cloned a *different* tree into
  `$JCODE_HOME/source/jcode`. That clone exists and its HEAD is
  `c4e82de2d` (a PR #145-era merge), months behind `github/main`.

So the tool did not bind to the working tree it was given, and the tree it
did create is not visible to the tool that asked for it.

## Mechanism

Two resolvers exist and they disagree about where a repository can live.

`SelfDevTool::resolve_repo_dir` (`crates/jcode-app-core/src/tool/selfdev/mod.rs:650`)
walks the ancestors of the tool call's `working_dir`, then delegates to
`build::get_repo_dir`.

`build::get_repo_dir` (`crates/jcode-build-support/src/paths.rs`) tries, in
order:

1. `$JCODE_REPO_DIR`;
2. ancestors of the compile-time `CARGO_MANIFEST_DIR` — for a binary built
   on the remote fleet this is a path on the *build host*, absent locally;
3. exe-relative `repo/target/<profile>/<binary>` — for an installed binary at
   `$JCODE_HOME/current/jcode` three parents up is `$HOME`, not a repo;
4. ancestors of `std::env::current_dir()`.

Step 4 carries a comment saying it exists precisely for "self-dev sessions
launched from the repo but running from an installed canary/stable binary".
That is this configuration, and it does not fire — the cwd that matters is
the one belonging to whichever process evaluates the call, and that is not
guaranteed to be the cwd the session was launched from. A second live `jcode`
process on this host has cwd `$HOME`.

Neither resolver consults `Self::selfdev_clone_dir()`
(`crates/jcode-app-core/src/tool/selfdev/setup.rs:304`), which is the
directory `setup` clones into. Its only callers are inside `setup.rs`
(lines 121, 310). So `setup` writes a repository to a location that
`find-config` and `reload` do not search — and then `find-config` tells the
operator to run `setup`.

Not every caller resolves alike. `status`
(`crates/jcode-app-core/src/tool/selfdev/status.rs:14`) and `session_rebuild`
(`crates/jcode-app-core/src/session_rebuild.rs:11`, `:96`) call
`build::get_repo_dir()` directly, so they never get the `working_dir` ancestor
walk. `reload` does not have this problem — see the correction below.

## Why it matters

`selfdev reload` is the step that turns a built binary into an observation.
Without it, a change can be written, built, reviewed and merged without ever
being run by the agent that wrote it. Work stops at "the diff looks right",
which is exactly the class of claim this repo does not accept elsewhere.

## Absence read as success

`selfdev setup --context <tree>` reported success. What it had done was clone
an unrelated, stale tree. Nothing in the output distinguished "bound to the
tree you named" from "created something else, somewhere else, at an older
commit". The operator's next command still failed, but the failure appeared
to be a *new* problem rather than the same one.

## Suggested direction

Not prescriptive; the shape that would close it:

- Say where it looked. `not found (run selfdev setup)` names a remedy that
  does not apply to the common case and hides the one fact that identifies it:
  the directory whose ancestors were searched. `not found (searched ancestors
  of $HOME)` would have ended this in one command instead of several.
- Have `setup --context <path>` bind that path when it is already a jcode
  repository, rather than cloning.
- Have `setup` report *which* tree it bound and at what commit, so a stale
  clone cannot render as success.
- Give `status` and `session_rebuild` the same resolver `reload` and
  `find-config` already use.

## Workaround

Work from a session whose working directory *is* the repository. Over the
debug socket:

    create_session:selfdev:$WORKTREE_PRIMARY

`selfdev build` issued in that session resolves the repository and queues
normally. `JCODE_REPO_DIR` in the serving process' environment is step 1 of
`get_repo_dir` and would also work, but it requires restarting that process,
whereas a new session does not.

## Correction (2026-08-18)

Two claims above were wrong when first written. Both are corrected in place;
this section records what changed and how the replacement was checked, so the
entry cannot be read as if it had always said this.

**The premise was wrong.** The original text asserted the session process' cwd
was the working tree. It was not. `pwd` in the reporting session returned
`$HOME`. Every resolver walks ancestors of that directory, and `$HOME` has no
jcode repository above it, so the failure is fully explained without any
resolver disagreement.

How the session came to be rooted at `$HOME` is a separate, still-open
question: being launched from there is the obvious explanation, but a resume
can also rewrite a session's recorded working directory, and that has not been
ruled out here. Only the *observation* — cwd was `$HOME` — is established
below; the cause is not.

Verified by A/B against a control, same host, same binary, seconds apart:

| session working dir | `selfdev find-config` → `Repository:` |
| --- | --- |
| `$HOME` (control) | `not found (run selfdev setup)` |
| `$WORKTREE_PRIMARY` (treatment) | `$WORKTREE_PRIMARY` |

The treatment session was made with `create_session:selfdev:$WORKTREE_PRIMARY`
over the debug socket. `selfdev build` in it queued a build and returned a task
id; the same call in the control session returned `Could not find the jcode
repository directory for selfdev build`.

**The `reload` claim was wrong.** The entry said `reload` calls
`get_repo_dir()` directly and cited `reload.rs:348`. It does not.
`resolve_selfdev_reload_repo_dir` (`reload.rs:232`, called from `:335`) treats
an explicit `working_dir` as authoritative — resolving through that directory's
ancestors *or not at all*, so a caller pointed at a non-repository never
silently reloads the ambient one. That is stricter than `find-config`, not
looser. The entry also cited `session_rebuild.rs:11` as living under
`tool/selfdev/`; the file is at `crates/jcode-app-core/src/session_rebuild.rs`.
`status.rs:14` and that file are the two call sites that really do bypass the
`working_dir` walk.

The claim was written from a reading of neighbouring code rather than from the
file it cited. It went into a merged document without anyone re-opening
`reload.rs`.

## Absence read as success, again

The error text is `not found (run selfdev setup)`. It is not wrong about
the absence — no repository was found. It is wrong about *why*, and it names a
remedy that cannot help, because `setup` clones into a directory discovery
never searches. Following that advice produced a second stale artifact and left
the original cause untouched. A message that had printed the directory it
searched would have shown `$HOME` immediately.

The residual gap is therefore narrower than first filed, and still real:

- the failure message points away from the cause;
- `setup`'s clone directory is not a discovery candidate, so its own advice is
  inert;
- `status` and `session_rebuild` resolve differently from `reload` and
  `find-config`, so the same session can be simultaneously able to build and
  unable to report where it would build.
