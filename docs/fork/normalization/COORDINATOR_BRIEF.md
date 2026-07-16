# Coordinator brief: post-recovery fork normalization

Use the following as the first user message in a fresh coordinator session.

```markdown
You are the coordinator and final engineering owner for post-recovery
normalization of `/Users/jrudnik/labs/jcode`. The six-phase evidence-driven
recovery is complete. Your job is to convert the signed recovery branch and the
local host into one canonical, clean, safe, well-organized, fully
runtime-validated fork without losing forensic history or user data.

Persist through implementation, validation, host normalization, independent
review, and final handoff. Do not stop at a plan. This program may span sessions.
At the start of every session, re-run the reproduction block in `BASELINE.md`,
append the observation, and reconcile drift before resuming mutation.

## Durable authority

Read these files completely before mutation:

- `docs/fork/normalization/README.md`
- `docs/fork/normalization/BASELINE.md`
- `docs/fork/normalization/COMPLETION_STANDARD.md`
- `docs/fork/recovery/PROGRESS.md`
- `docs/fork/recovery/RECOVERY_PLAN.md`
- `docs/fork/recovery/reviews/2026-07-16-w7-review.md`
- `docs/fork/SYNC_MODEL.md`

Track durable progress in initiative `post-recovery-fork-normalization`. The old
initiative `fork-recovery-2026-07-15` is complete and must not be reopened.

## Starting facts to revalidate, not assume

- Primary checkout: `/Users/jrudnik/labs/jcode`.
- Expected branch: `recovery/2026-07-15`.
- Pre-normalization-infrastructure head:
  `cdc2cc2b4cea51c185de330c8e15e08615acc46c`.
- The current authority commit cannot self-identify inside its own contents.
  Derive it with:
  `git log -1 --format='%H' -- docs/fork/normalization/COMPLETION_STANDARD.md`.
  At session start, all four normalization authority files must be tracked and
  this authority commit must be reachable from branch HEAD.
- Phase 6 closure: `ba45f20aa61fdf597bbe4a1d11e94d1dd43c8c38`.
- Accepted recovery source: `51168d16e9c708ae4afff09a6fc6402642d17782`.
- `main`: `6ca1fcf2ec2366c7abc99664a485c40d60cec80e`, an ancestor
  166 commits behind recovery at the snapshot.
- Exactly 29 registered worktrees: 25 under `/Users/jrudnik/labs`, 4 under
  `/private/tmp`. All auxiliary worktrees were clean when inventoried.
- Expected post-infrastructure dirty state: only
  `docs/fork/recovery/ORCHESTRATOR_PROMPT.md`, with diff SHA-256
  `8e8e6a92dad180b3925bc0b2a3b7b951bc6a6f5c9e4f8a57c9f522d03ad85c00`.
- 40 local branches, 138 tags, and 916 commits reachable from local refs but not
  recovery HEAD.
- Exactly four stashes with commit and index-parent identities recorded in
  `BASELINE.md`. `stash@{1..3}` are reflog-only and are not protected by a normal
  `git bundle --all`.
- `vendor/upstream` remained pinned at
  `631935dd1d3b2e31e167e2b12ad463e54bcf4b8d`.
- Phase 6 audit passed 62/62 real checks and all 17 evidence manifests.
- W7 is reviewed and must be split into W7a-W7d. It is not a seventh recovery
  phase.
- Two jcode binaries are on PATH. The `/etc/profiles/per-user/...` binary is
  home-manager/Nix-managed and must not be edited or removed directly.
- Menubar, hotkey, shared-server, and `com.jcode.hotkey` are pre-existing user
  integrations to retain or gracefully restart, not cleanup targets.
- `com.jcode.lesson-library-shadow` and `.wrangler/` are unrelated project state,
  out of scope, and must not be swept by name or path matching.

If a starting fact differs, stop mutation, preserve the observation, explain the
drift, and update the normalization baseline append-only. The committed authority
files and commits made by this normalization program are expected evolution, not
unexplained drift.

## Completion standard

`docs/fork/normalization/COMPLETION_STANDARD.md` is binding. Completion requires
all D0-D9 gates at one fixed committed head and one recorded host state. Do not
claim “clean,” “fully functional,” or “fully runtime validated” while any gate is
missing. Without an explicitly authorized successful real-provider turn, the
strongest permitted label is “core-runtime validated,” and final D9 remains open.

## Execution milestones

### N0: freeze safety and capture truth

1. Reproduce Git refs, prompt hash, all stash and stash-index commit IDs,
   worktrees, branch/tag tips, remotes, current processes, binaries, services,
   sockets, configs, caches, build queues, application bundles, package/profile
   entries, and every jcode-related filesystem path. Credential-bearing files
   are inspected by metadata only, never printed.
2. Produce a path-by-path and ref-by-ref classification: canonical, archive,
   integrate, retain, remove, unrelated/out-of-scope, unknown, or user decision
   required.
3. Record exact current targets and SHA-256 values for
   `~/.jcode/builds/current/jcode` and
   `~/.jcode/builds/shared-server/jcode`.
4. Create rollback artifacts before cleanup:
   - verify the four current stash commits and index parents against
     `BASELINE.md`;
   - create explicit archive refs for every stash commit and index parent;
   - create and verify a stash-object bundle from those archive refs;
   - create and verify an all-ref bundle containing all branch/tag/archive refs,
     including the 916 commits not reachable from recovery;
   - restore both bundles into a disposable repository and prove every archived
     tip, all four stash commits, and the `stash@{3}` untracked-payload parent
     `7c68ef5f59359ed89e0979b99bba143c74d926aa` are present;
   - record that creating stash archive refs is expected to change the initial
     916-object reachability count, so it is not misclassified as unexplained
     drift.
5. Record which actions require explicit approval. Never combine inventory and
   deletion in one step. Preserve all failed or superseded attempts.

### N1: curated integration line, no `main` movement

1. Preserve the exact recovery line under a named immutable archive ref. Do not
   rewrite it.
2. Prepare a bounded commit-stack plan before integration. Each commit has a
   single stated purpose; each product commit builds and passes focused tests, or
   an inseparable group has an explicit validation boundary.
3. Create a curated integration branch from current `main` and carry the approved
   recovered product tree in logical commits. Keep evidence in bounded dedicated
   documentation commits, not interleaved with product history.
4. Prove tree equivalence against the approved recovered product tree, with every
   intentional normalization difference enumerated.
5. Reconcile every auxiliary branch, tag, worktree, and stash. A non-ancestor
   branch is not disposable merely because recovery closed.
6. N1 ends with a reviewed tree-equivalent curated branch. **Do not move `main`
   in N1.** Branch/ref/worktree/stash deletion remains approval-gated and cannot
   begin until both rollback bundles and the dry-run disposition manifest pass.

### N2: W7, quality, and `main` promotion

Execute W7 as separate, serially reviewed slices on the curated branch:

- W7a: `ClassifiedEvidenceError::source` plus typed interruption predicate and
  positive, chained, and lookalike-negative fixtures.
- W7b: deterministic interrupted-turn `stopped`/`cancelled` normalization in all
  consumers.
- W7c: narrow provider-evidence helper extraction only, with all 11 R12 fixtures
  and exact event-count/correlation parity unchanged.
- W7d: propose the exact UTF-8-safe bound and retention algorithm first. Obtain
  user or independent-reviewer policy approval before merging the implementation.
  Preserve the original checkpoint, newest history, and an omission marker.

Adjudicate the original R03A verdict-centralization and R02 file-splitting ideas.
Implement them only if evidence supports them. Otherwise close them explicitly.
Reconcile R09 debt without `--update` or baseline laundering. The exact Phase 6
starting reds are panic `31 -> 48`, swallowed-error `2987 -> 3074`, production-
size red, and test-size red.

Re-run tree equivalence and the complete trusted matrix. Only after W7, quality,
independent review, explicit approval, and rollback readiness may `main` be
fast-forwarded to the curated branch. `main` promotion is the exit criterion of
N2.

### N3: documentation and task normalization

1. Make current docs describe the canonical fork rather than the recovery
   process.
2. Archive recovery-only instructions and add operator runbooks for build, test,
   daemon, reload, attach, diagnostics, sync, rollback, and cleanup.
3. Close, migrate, or explicitly own every recovery/W7/TODO/FIXME/task/initiative
   item. No ambiguous recovery task may remain active.
4. Refresh monitored-curation policy and eliminate the stale
   `vendor/upstream` footgun without broad replay. A read-only `git fetch
   upstream` is permitted after remote verification, but it may not delete refs,
   merge, rebase, replay, or push.
5. Reconcile duplicate `origin`/`github` remotes with an explicit rationale.

### N4: hermetic and live runtime validation

1. Run the complete trusted command set from
   `docs/fork/recovery/QUALITY_GATES.md` and the Phase 6 accepted driver, minus
   only explicitly retired recovery checks, from a clean canonical checkout.
   Do not update baselines.
2. Before starting any sandbox daemon, prove its `JCODE_HOME`, runtime directory,
   socket, port, pid, and marker paths are disjoint from all pre-existing user
   integrations.
3. Use those disposable paths for real binary/daemon start, attach, reload,
   resume, cancel, persistence, swarm, tool, and MCP tests. Leave no sandbox
   orphan, but do not kill or repoint the menubar, hotkey, shared-server, or
   `com.jcode.hotkey` processes.
4. Verify source, binary, daemon, build hash, runtime identity, and protocol
   version agreement.
5. Real provider credentials, installer/update execution, account mutation,
   signing, tagging, release, publication, and remote push require separate
   explicit authorization. Redact secrets from all logs.
6. Obtain approval for and execute at least one real provider turn to reach
   “fully runtime validated.” If authorization is declined, record
   “core-runtime validated” and leave final completion open.

### N5: local-host normalization

1. Present a complete dry-run cleanup manifest before removing or stopping
   anything. Preserve user sessions, credentials, configuration, and logs by
   default.
2. Exclude all home-manager/Nix-managed paths from direct mutation. Any desired
   change must be made through its declarative source and requires explicit user
   approval.
3. Preserve or gracefully restart the intended menubar, hotkey, shared-server,
   and `com.jcode.hotkey` integrations. Treat
   `com.jcode.lesson-library-shadow` and `.wrangler/` as unrelated and out of
   scope. Never classify by name/path glob alone.
4. Before any live binary repoint, record the current and shared-server link
   targets and hashes. Produce a dry-run restore script that uses those exact
   prior targets with `ln -sfn <recorded-target> <link>`, verifies the two
   recorded SHA-256 values, and gracefully restarts intended integrations. The
   exact pre-normalization commands are recorded in `BASELINE.md`. Do not execute
   the live repoint without explicit approval.
5. After explicit approval, remove only items classified `remove`: stale recovery
   worktrees, `/private/tmp` worktrees, selected refs, duplicate unmanaged
   binaries, stale runtime processes/agents, sockets, pid files, aliases,
   symlinks, caches, queues, and temp paths. Every removal must have passed the
   all-ref/stash archive and item-specific rollback gates.
6. Leave one documented canonical checkout and one documented runtime precedence.
   From a new shell, verify the resolved binary and smoke-test the normalized
   runtime without hidden session state.

### N6: final sign-off

1. Re-run every D0-D9 check and package byte-hashed evidence under
   `docs/fork/normalization/evidence/` in bounded dedicated documentation commits.
2. Obtain an independent architecture review and an independent operations/live
   validation review at the same fixed commit and host manifest. Reviewers must
   not have authored, steered, or approved an implementation lane they review.
3. Resolve every IMPORTANT/CRITICAL finding before closure.
4. State the remote disposition exactly: either an explicitly authorized push of
   canonical `main` and selected archive refs, or
   `local-canonical, remote pending` with the risk recorded. Do not push merely to
   satisfy the wording.
5. Publish a concise final handoff: canonical repo/branch/binary, archive and
   rollback anchors, validated runtime surfaces, normal debt, upstream-curation
   workflow, and exact remaining host state.

## Safety and coordination rules

- Use direct, non-interactive tools. Do not use `scripts/dev_cargo.sh`.
- Commit coherent changes as you go. Keep source, tests, refactors, sync, and
  docs separable.
- Use dedicated worktrees only when needed, and record their cleanup owner at
  creation. Do not increase worktree sprawl without a removal plan.
- Serialize overlapping source changes and all canonical-history integration.
- Use GPT-5.5 for implementation/live test lanes, Opus for independent review,
  and Fable for architecture/investigation when available. Final D9 reviewers
  must be fresh and independent of implementation and steering.
- Preserve every failed, interrupted, invalid, superseded, unsafe, or
  contradictory attempt append-only.
- Do not force push, rewrite the recovery archive, expose credentials, publish,
  sign, release, pay, delete user data, or mutate accounts.
- Do not edit or discard the preserved recovery prompt until its disposition is
  explicitly approved.
- Moving `main`, deleting refs, removing worktrees, dropping stashes, changing
  declarative Nix/home-manager state, repointing live binaries, stopping user
  integrations, using real credentials, or taking external write actions requires
  an explicit approval packet with dry run, backup, rollback, and validation.
- Request user input only when such a gate is actually ready. Otherwise continue
  autonomously through the next reversible measurable gate.

Start now with N0 read-only inventory. Update the durable initiative and visible
todo list, then proceed through every reversible prerequisite. Present explicit
approval packets only when a destructive, credentialed, live-profile, `main`, or
external action is ready.
```
