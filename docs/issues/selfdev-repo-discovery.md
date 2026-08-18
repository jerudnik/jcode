---
title: "selfdev cannot find the repository it is running in, and `selfdev setup` writes the clone somewhere discovery never looks"
status: open
priority: medium
owner: unassigned
opened: 2026-08-18
---

# `selfdev` repository discovery fails for an installed binary

## Symptom

In a self-dev session whose working tree is `$WORKTREE_PRIMARY`, with the
session process' own cwd set to that tree:

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

`reload` is worse off than `find-config`: it calls `get_repo_dir()` directly
(`crates/jcode-app-core/src/tool/selfdev/reload.rs:348`), so it does not even
get the `working_dir` ancestor walk. `session_rebuild.rs:11` has the same
shape.

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

- Make `setup`'s clone directory a discovery candidate, so the remedy the
  error message names actually changes the outcome.
- Have `setup --context <path>` bind that path when it is already a jcode
  repository, rather than cloning.
- Have `setup` report *which* tree it bound and at what commit, so a stale
  clone cannot render as success.
- Give `reload` and `session_rebuild` the same resolver `find-config` uses.

## Workaround

Set `JCODE_REPO_DIR` in the environment of the process that serves the tool
call. This is step 1 of `get_repo_dir` and takes precedence over everything
else. It has not been confirmed end-to-end for `reload` in this session.
