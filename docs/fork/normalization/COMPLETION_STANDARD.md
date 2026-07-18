# Completion standard: normalized jcode fork

> **Historical normalization standard.** The active ideal-base exit gates are in
> [`../ideal-base/ACCEPTANCE_STANDARD.md`](../ideal-base/ACCEPTANCE_STANDARD.md).
> This document remains evidence for how the earlier normalization claim was
> bounded.

The fork is done only when every applicable gate below passes at one fixed
committed head and one recorded local-host state. “Clean” means understandable,
reproducible, owned, and free of unexplained state. It does not mean erasing
recovery history or pretending normal technical debt does not exist.

The target is **fully runtime-validated**. If authorization for a real provider
turn is declined, the program may reach an honest intermediate state named
**core-runtime validated**, but D9 and the unqualified definition of done remain
open.

## D0. Safety, evidence, and rollback

- Must: The exact recovery branch, commits, reports, evidence, prompt edit, all
      local refs, worktree inventory, and relevant host state have immutable
      hashes and at least one verified rollback archive.
- Must: Before any ref deletion, a verified all-ref bundle physically contains the
      current branch and tag tips, including commits not reachable from recovery.
- Must: Before any stash drop, reflog expiration, garbage collection, branch
      deletion, or worktree removal, all four stash commit objects and their index
      parents are explicitly included in a separate verified stash-object bundle.
      `git bundle --all` alone is insufficient because `stash@{1..3}` are
      reflog-only.
- Must: Both bundles pass `git bundle verify` and are restoration-tested in a
      disposable repository. All four stash commits can be re-listed from the
      restored object set, and the `stash@{3}` untracked-payload parent
      `7c68ef5f59359ed89e0979b99bba143c74d926aa` is present.
- Must: Every destructive category has a dry-run manifest, explicit approval,
      pre-state backup, post-state check, and tested restoration command.
- Must: Failed, interrupted, invalid, superseded, or contradictory attempts remain
      visible and are not counted as passing evidence.
- Must: No secret, credential, token, private payload, or user content is copied
      into committed evidence or logs. Credential inventories are metadata-only.

## D1. One canonical repository and Git topology

- Must: `/Users/jrudnik/labs/jcode` is the documented canonical source checkout.
- Must: The canonical branch is `main`, and `main` points to the independently
      validated recovery-plus-W7 normalized tree.
- Must: `git status --short` is empty in the canonical checkout.
- Must: The preserved prompt edit has an explicit disposition: committed to an
      appropriate history line, archived as a hashed patch, or discarded only
      by explicit user instruction.
- Must: The four stashes are individually identified and resolved by keep/archive,
      integrate, or explicitly approved drop. No anonymous stash remains.
- Must: Every local branch and tag has a documented keep/archive/delete decision.
- Must: Exactly the intentionally documented worktrees remain. The default target
      is one canonical checkout and zero recovery worktrees.
- Must: No independent duplicate clone, stale `.git/worktrees` entry, orphaned
      worktree directory, misleading symlink, or ambiguous source path remains.
- Must: Remotes and the monitored-curation sync model are current. The duplicate
      `origin`/`github` relationship has an explicit disposition, and a stale
      `vendor/upstream` ref cannot masquerade as current upstream truth.

## D2. Clean and reviewable history

- Must: The exact recovery line remains under an explicit immutable archive ref.
      The all-ref and stash-object rollback bundles from D0 exist before canonical
      history is curated or any ref is removed.
- Must: Canonical history is constructed from `main` as a bounded logical stack
      with source, tests, refactors, sync changes, and documentation separated
      where useful. A stack plan names every commit and its purpose before
      integration; independent review confirms there is no accidental merge,
      conflict residue, fixup/WIP commit, or obscuring evidence churn.
- Must: Each product commit builds and passes its focused tests, or an explicitly
      documented inseparable commit group is validated at its group boundary.
- Must: Curating canonical history does not rewrite or delete the recovery archive.
- Must: N1 ends when the curated integration branch carries the tree-equivalent
      recovered product tree. W7a-W7d then land as reviewed commits on that branch.
      `main` promotion is the exit criterion of N2, not N1.
- Must: Tree equivalence is proven between the approved recovery-plus-W7 tree and
      the curated integration branch, excluding only enumerated normalization
      changes.
- Must: `main` moves only by reviewed fast-forward to that final curated branch.
- Must: Normalization evidence lives under
      `docs/fork/normalization/evidence/` in a bounded number of dedicated
      `docs(evidence)` commits at the top of the curated stack. This is the
      explicit exception to the evidence-churn prohibition.
- Must: No force push or remote push occurs without a separate explicit user
      instruction.

## D3. W7 and code normalization

- Must: W7a preserves `ClassifiedEvidenceError` source traversal and provides a
      typed interruption predicate with positive, chained, and lookalike-negative
      fixtures.
- Must: W7b maps interrupted turns consistently to `stopped`/`cancelled` in every
      server consumer, with deterministic detached-cancel ordering coverage.
- Must: W7c performs only narrow provider-evidence helper consolidation. All 11
      R12 fixtures and explicit event-count/correlation parity remain unchanged.
- Must: Before W7d merges, the coordinator proposes the exact UTF-8-safe byte bound
      and retention algorithm, and the user or an independent reviewer approves
      it as a product policy. The implementation preserves the original
      checkpoint, newest provenance, and a visible omission marker.
- Must: The original R03A verdict-centralization and R02 file-splitting candidates
      are either implemented with evidence or explicitly closed as unwarranted.
      No vague “W7 later” item remains.
- Must: Protocol, replay, identity, liveness, consent, acquisition, and authority
      boundaries remain explicit and version changes are independently reviewed.

## D4. Quality and build integrity

- Must: A clean canonical checkout passes the trusted command set defined by
      `docs/fork/recovery/QUALITY_GATES.md` and the Phase 6 accepted audit driver,
      minus only recovery-scoped checks explicitly enumerated as retired. The
      matrix includes unit, integration, protocol, lifecycle, storage, TUI,
      build-support, static, dependency, warning, wildcard, shell, and diff gates.
- Must: No baseline or expected-output update is used to make a regression pass.
- Must: The Phase 6 expected-red starting point is preserved exactly: panic
      `31 -> 48`, swallowed-error `2987 -> 3074`, production-size red, and
      test-size red. Normalization either reduces these to trusted baselines or
      migrates them into one normal debt register with exact current counts,
      affected files, ownership, rationale, and triggers. No unidentified or
      recovery-scoped red remains.
- Must: Build artifacts are reproducible from documented commands, and source,
      binary, build hash, protocol version, and runtime identity agree.
- Must: No stale build process, remote builder, queue entry, lock, or cache is
      required for a successful clean build.

## D5. Documentation and task-state integrity

- Must: Root documentation describes the canonical fork, supported platforms,
      build/run/test commands, runtime topology, sync policy, and known limits.
- Must: Current architecture and authority maps agree with code and tests.
- Must: Recovery-only instructions are clearly archived and cannot be mistaken for
      active operating instructions.
- Must: All recovery, W7, TODO, FIXME, issue, initiative, and task-list entries are
      closed, implemented, rejected with rationale, or migrated to normal backlog
      with an owner and trigger.
- Must: The completed recovery initiative is closed. The normalization initiative
      is complete. No stale “pending” status remains in an active document.
- Must: Operator runbooks cover build, install/select binary, daemon start/stop,
      reload, attach, diagnostics, rollback, upstream curation, and host cleanup.

## D6. Isolated live runtime validation

- Must: A disposable `JCODE_HOME` and runtime directory exercise the real compiled
      binary without mutating the user's normal profile until promotion.
- Must: A pre-start collision check proves the sandbox uses disjoint socket, port,
      pid, marker, home, and runtime paths from every pre-existing jcode process
      or user integration.
- Must: The sandbox daemon starts, exposes identity, accepts attach, stops cleanly,
      and leaves no orphan process or socket. This requirement does not authorize
      stopping the pre-existing menubar, hotkey, or shared-server processes.
- Must: Reload verifies build/protocol compatibility, session continuity,
      interruption, resume, cancellation, and persistence across restart.
- Must: TUI/CLI message flow, storage round trip, swarm assignment/reclaim, tool
      execution, and MCP lifecycle pass deterministic end-to-end scenarios.
- Must: Consent, credential, telemetry, installer, updater, and acquisition paths
      remain fail-closed in sandbox tests.
- Must: At least one explicitly authorized real provider turn succeeds end-to-end
      with secrets redacted from logs. Without this gate the result is only
      “core-runtime validated,” and full completion remains open.
- Must: Installer/updater behavior is validated against local signed/checksummed
      fixtures. Real installation, update, release, tag, signing, publication,
      or account mutation is not required and remains separately authorized.

## D7. Local-host normalization

- Must: A complete before/after manifest covers source trees, worktrees, branches,
      tags, stash objects, binaries on `PATH`, aliases, symlinks, application
      bundles, launchd agents, processes, sockets, pid/marker files,
      configuration, credential references, logs, caches, temporary paths, build
      queues, and package/profile entries.
- Must: Only one documented canonical runtime selection has precedence in a new
      shell. The home-manager/Nix binary is declaratively managed and is never
      deleted or edited directly. Any declarative change requires explicit user
      approval and modification of its source configuration.
- Must: Pre-existing user integrations, including menubar, hotkey, shared server,
      and `com.jcode.hotkey`, have explicit retain/restart decisions. They are not
      silently killed by sandbox validation or broad cleanup.
- Must: Processes or LaunchAgents that merely match `jcode` by name or working path
      are not assumed to be jcode runtime components. In particular,
      `com.jcode.lesson-library-shadow` and its ignored `.wrangler/` state are
      out of scope and retained absent a separate user decision.
- Must: At most one intended canonical jcode server runtime exists after promotion,
      and its binary/repo identity matches canonical `main`; separately intended
      UI/hotkey integrations are documented rather than miscounted as daemons.
- Must: Recovery worktrees and `/private/tmp/jcode-*` directories are removed only
      after all-ref and stash-object archive verification plus branch-by-branch
      disposition. Git reports no prunable worktree metadata.
- Must: Stale binaries, sockets, pid files, launch agents, aliases, symlinks,
      caches, and misleading configs are removed or explicitly retained with an
      owner and purpose.
- Must: User sessions, credentials, configuration, and logs are preserved unless a
      separately approved retention decision says otherwise.
- Must: Before any live binary repoint, the exact current and shared-server link
      targets and binary hashes are recorded. The restore procedure repoints the
      links to those recorded targets and gracefully reloads the intended user
      integrations; it is dry-run exercised before promotion.
- Must: A cold start from a new shell resolves the expected binary and can execute
      the documented smoke test without relying on hidden session state.

## D8. Security and operational safety

- Must: Secrets stay in approved stores with least-privilege permissions.
- Must: Logs and evidence are checked for credential or payload leakage.
- Must: Provider, tool, MCP, installer, updater, release, and daemon actions obey
      explicit consent boundaries and fail closed.
- Must: Rollback from canonical runtime to the recorded pre-promotion binary links
      and the archived recovery artifact is documented and exercised in a
      disposable environment without data loss. The production restore command
      remains approval-gated unless an actual rollback is required.

## D9. Final independent sign-off

- Must: One independent architecture reviewer and one independent operational
      reviewer sign the same fixed commit and host manifest PASS. Neither reviewer
      authored, steered, or approved an implementation lane being reviewed.
- Must: There are zero unresolved CRITICAL or IMPORTANT findings.
- Must: The final evidence package under `docs/fork/normalization/evidence/`
      reproduces Git topology, clean status, task/doc closure, trusted gates,
      runtime scenarios, host inventory, secret scan, canonical binary identity,
      and rollback.
- Must: The final handoff states the exact remote disposition: either an explicitly
      authorized push of canonical `main` and selected archive refs, or the
      qualified status `local-canonical, remote pending` with its risk recorded.
- Must: A concise operator handoff states what is canonical, what was archived,
      what normal debt remains, how upstream curation works, and how to recover.

Only after D0-D9 pass may the fork be described without qualification as tidy,
clean, safe, well-organized, and fully runtime-validated.
