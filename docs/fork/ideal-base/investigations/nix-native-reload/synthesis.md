# Synthesis + Verification: collapsing the reload subsystem to a nix-native single path

Reviewer: synthesis pass over investigator-A.md and investigator-B.md, with independent
re-reading of the code (READ-ONLY at `/Users/jrudnik/labs/jcode`). Every load-bearing
claim below was re-derived from source, not taken on trust. All line numbers are ones I
opened myself in this pass.

---

## Bottom line

The collapse is **safe and, in fact, half-built already** — but the one thing you must not
get wrong is **atomicity of the reload target**. The version store's genuinely load-bearing
job is not "channels" or "rollback"; it is the atomic publish (stage a private temp copy,
fsync it, smoke-test that temp copy, then `rename(2)` it into place), proven by a real test
that truncates the *source* mid-install and asserts the published copy survives
(`crates/jcode-build-support/src/lib.rs:611-669`). A nix `result`/store path preserves that
atomicity for free (nix swaps a symlink); a bare cargo-written `target/selfdev/jcode` does
**not**, because rustc's linker rewrites that path in place. The target shape is therefore:
**one atomic fixed path for the daemon and headed clients (the nix profile binary / `result`),
plus a deliberately-kept fast `cargo build` for the self-dev inner loop whose output is
published through a *single* atomic rename into one fixed path (no `versions/<label>` dir, no
channel fan-out).** The "stable fallback comfort" costs almost nothing under nix: a pinned
profile generation or a `nix run github:...#jcode/<rev>` gives you a real known-good binary
without the hand-rolled stable/current/shared-server machinery. Subsystem A (GitHub
release/`install.sh`/`jcode-update-core`) can be retired outright. The scaffolding for all of
this already exists and is dead only because nothing sets `JCODE_NIX_MANAGED`.

---

## Verification results (each high-stakes claim, my own file:line)

### 1. Atomicity claim (B's biggest divergence) — **CONFIRMED**, and it is the crux.

- The atomic-publish code exists exactly as B described:
  - `install_binary_at_version_in_builds_dir` (`lib.rs:382-420`) does: stage via
    `copy_binary_to_staging_path` -> `run_after_install_stage_hook` (test seam) ->
    `smoke_test_staged_binary_for_install` (on the *staged* temp) -> `publish_staged_binary`
    (the rename) -> cleanup on error.
  - `copy_binary_to_staging_path` (`lib.rs:422-493`): `create_new(true)` private temp
    (`.jcode-publish-<pid>-<nanos>-<attempt>`), `std::io::copy`, `set_permissions_executable`,
    **`sync_all()` (fsync, lib.rs:457-458)**, an explicit **zero-byte guard**
    (`lib.rs:464-469`), then `sync_directory_best_effort`.
  - `publish_staged_binary` (`lib.rs:510-526`): `std::fs::rename(staged, dest)` — the atomic
    publish — followed by a best-effort dir fsync.
- The guarding test exists and proves the exact property: `concurrent_source_truncation_
  between_stage_and_rename_preserves_published_copy` (`lib.rs:611-669`). It installs a script,
  registers an after-stage hook that **truncates the source to zero bytes right after staging**
  (`lib.rs:630`), then asserts the versioned copy is non-empty and byte-identical to the
  original (`lib.rs:641-647`), and that the smoke test ran the **staged temp path**, not the
  source and not the final path (`lib.rs:651-667`). This is a real regression test, not a
  comment.
- **B's race analysis is correct and A's "single fixed path is fine" is too optimistic on
  this one axis.** If reloads point at a bare `target/selfdev/jcode`, a concurrent
  `cargo build` (which relinks that path in place; `scripts/dev_cargo.sh` builds into
  `target/selfdev`, see the `--profile selfdev` plumbing at lines 501-521) can be observed as a
  partial ELF by a process that execs it mid-link. The ENOENT retry loop in `hot_reload`
  (`src/cli/hot_exec.rs:102-125`) does **not** cover this: it retries on *missing* file, not on
  a *present-but-truncated* one.
- **Important nuance both investigators under-stated — verified in the daemon path:** the
  daemon does **not** exec the bare repo path *today*. `collect_reload_target_candidates`
  (`server/util.rs:343-406`) adds the shared-server/stable channels and the repo dev binary,
  but the repo dev binary is pushed with `exec_candidate = false` (`util.rs:383-394`), and
  `resolve_reload_target_from_candidates` filters to `exec_candidate` only
  (`util.rs:137-142`). So the daemon only ever execs a **published, atomically-renamed copy**
  (shared-server or stable channel), exactly the atomicity B is protecting. Meanwhile the
  **headed client already execs the bare `target/selfdev/jcode`**: `preferred_reload_candidate`
  prefers the newest repo build over the channel (`paths.rs:704-735`), and
  `client_update_candidate` returns the bare dev binary for selfdev (`paths.rs:573-578`). So
  clients already run under the truncation risk today, mitigated only by the timing convention
  that a human reloads *after* the build finishes.
- Net: the collapse must **keep an atomic step for the daemon** (nix `result` or a one-line
  stage+fsync+rename into a single fixed path). Pointing the daemon at bare `target/selfdev`
  would be a *regression* from today's behavior, not a lateral move. Confidence: **high.**

### 2. Rollback claim — **CONFIRMED** (restores symlinks, not the process; depends on the store).

- `rollback_pending_activation_for_session` (`lib.rs:254-274`) restores `current` (+ launcher)
  and `shared-server` symlinks to `previous_current_version` / `previous_shared_server_version`
  from the `PendingActivation` record. It execs nothing.
- The record is written on `/reload` with the previous versions captured
  (`tool/selfdev/reload.rs:309-319`), promoted onto shared-server (`reload.rs:322`), and
  rolled back on ack-timeout / `Failed` / unconfirmed-ready
  (`reload.rs:324,342,368,401,411`). On `Ready` it is completed
  (`complete_pending_activation_for_session`, `reload.rs:394` / `lib.rs:236-252`).
- Dead-initiator recovery exists: `reconcile_stale_pending_activation` (`lib.rs:327-374`,
  called from `server.rs`) validates the candidate via its `.source.json` sidecar
  (`pending_candidate_is_valid`, `lib.rs:298-315`) and either completes or rolls back, and it
  refuses to clobber a newer publish (`lib.rs:361-371`).
- **Losing the store does mean "a bad build can wedge the next spawn"** — but with a precise
  caveat both reports got right: the *running* process is protected by execv-failure semantics
  (`server/reload.rs:207-242` re-enters termination on a failed `replace_process`;
  `hot_exec.rs:102-125` for clients), **not** by rollback. Rollback only fixes the *pointer* so
  the *next* spawn/reload does not re-pick the bad build. And it only covers "replacement never
  became ready"; a build that boots, binds the socket, then crash-loops is **not** auto-reverted
  (the pending record is already cleared). So the store's rollback is narrower than it looks.
- **Does nix generations cover this?** Only if the binary is **installed into a profile**
  (`nix profile install` -> generations -> `nix profile rollback`), or you pin a rev
  (`nix run github:...#jcode/<good-rev>`). Plain `nix run` of the flake's default package does
  **not** create a generation and gives you no rollback pointer. This is the single most
  important design consequence for the "comfort" requirement. Confidence: **high.**

### 3. Dormant nix hook — **CONFIRMED both halves.**

- `JCODE_NIX_MANAGED` is set **nowhere**. Grep shows only readers and docs:
  `paths.rs:487` (`is_externally_managed`), `paths.rs:550` (doc), `doctor.rs:474`, plus
  `docs/NIX.md`, `docs/fork-sync-policy.md`. I confirmed the nix packaging never sets it:
  `nix/package.nix` sets only `JCODE_BUILD_*` build-meta and has **no** `makeWrapper`/
  `wrapProgram`; `nix/modules/home-manager.nix` sets only `JCODE_HOME` (`:121-122`);
  `flake.nix` never mentions it. So `nix_managed_launcher_override` is **dead code today**.
- The override **already implements the single-fixed-path model for the non-selfdev case**:
  `nix_managed_override_target` (`paths.rs:551-562`) fires only when
  `externally_managed && !is_selfdev_session` and returns the launcher (else `current_exe`),
  and it is consulted **first** in all three resolvers — `client_update_candidate`
  (`paths.rs:565`), `shared_server_update_candidate` (`paths.rs:603`),
  `preferred_reload_candidate` (`paths.rs:698`). Setting the flag collapses all three for
  non-selfdev in one move, and `update_launcher_symlink` becomes a no-op (`paths.rs:495`).
- The **self-dev branch bypasses it**: the `!is_selfdev_session` gate at `paths.rs:555` means
  every resolver falls through to `current`/`canary`/repo resolution for selfdev sessions
  (`client_update_candidate` selfdev arm at `paths.rs:573-583`). Since the maintainer works
  almost entirely in self-dev, **flipping one env var is NOT enough** — both reports are right,
  confirmed. Confidence: **high.**

### 4. Multi-process — **CONFIRMED**: single daemon, in-process swarm agents.

- The daemon is one long-lived `serve` process that reloads in place via execv
  (`server/reload.rs:57-244`, exec at `:204-207`).
- Headless **and inline** swarm agents run **in-process**: `comm_session.rs:682-685` —
  "Inline workers run in-process like headless ones; the difference is purely how the
  coordinator renders them." They have no own binary and ride the daemon's execv; they are
  re-spawned from persisted reload-recovery intents. Headed sessions are separate terminal
  processes that each reload themselves via `preferred_reload_candidate`.
- So there is **no "N processes must exec the same binary at the same instant" requirement.**
  The shared-server channel is a *policy* pointer (isolate the daemon from fast-moving dirty
  builds), not a synchronization primitive. This is what makes the collapse safe. Confidence:
  **high.**

### Bonus verifications (things I checked to ground the recommendation)

- Subsystem A is cleanly separable: crate `crates/jcode-update-core`, `scripts/install.sh`,
  `.github/workflows/release.yml`, `update.rs::download_and_install`. Retirable as a unit.
- The daemon no-downgrade guard, cross-flavor newest-candidate scan, forced-stale refusal, and
  wrapper->payload resolution are real and live: `reload_exec_target`/`resolve_reload_target`
  (`util.rs:88-128`), `guarded_reload_target` + `forced_stale_shared_server_refusal` (referenced
  from `resolve_reload_target_from_candidates`, `util.rs:149-152`), and `resolve_binary_payload`
  used to compare payloads not wrappers (`util.rs:114-118`, `paths.rs:716-721`).
- The flake ships an overlay, `packages.default`, and a home-manager module
  (`flake.nix:39-95`), and a public Cachix substituter (`flake.nix:23-28`). That is the natural
  home for both the fixed path and the stable-fallback generation.

---

## A-vs-B adjudication

Both reports are high quality and agree on the big picture. Where they diverge:

- **Atomicity as *the* load-bearing property (B) vs "smoke-test + rollback are the two safety
  properties" (A):** **B is more right.** A frames the store's value as smoke-gate +
  rollback-to-previous, and treats "single fixed path is fine if nix owns rollback." That is
  true *for the daemon's already-published targets*, but A under-weights that the store's
  irreducible core is the atomic stage+fsync+rename — the one thing the ENOENT retry and the
  execv-failure net do **not** provide. B correctly isolates atomicity as the non-negotiable
  invariant and correctly flags that `target/selfdev` is not equivalent to `result`. I verified
  B's exact claim against `lib.rs:382-420,611-669`.
- **A's "single fixed path is fine" optimism:** partly refuted. It is fine **only** if that
  fixed path is itself atomic (nix `result`/profile store path) or fed by a retained
  stage+fsync+rename. A's own "must-preserve #1/#2" list actually concedes this, so the
  disagreement is one of *emphasis and framing*, not substance.
- **Rollback narrowness:** B states more precisely than A that rollback does **not** cover a
  post-ready crash-loop (`reload.rs:394` clears the record on Ready). A treats rollback as a
  general "known-good fallback." B's narrower reading is the correct one — verified.
- **What both slightly missed:** the `exec_candidate=false` gate on the repo dev binary in the
  daemon's candidate collection (`util.rs:383-394,137-142`). This means the daemon is *already*
  protected from execing the bare `target/selfdev` path, while headed clients are *not*
  (`paths.rs:704-735`). Consequence: a naive "point everything at target/selfdev" collapse would
  **regress the daemon** specifically. Neither report stated this asymmetry crisply; it sharpens
  B's constraint into a concrete "do not lower the daemon below its current atomicity floor."
- Everything else (channels/version-store are drift-reconciliation for multiple self-managed
  pointers; shared-server is blast-radius policy; subsystem A is dead; fingerprint checks are
  correctness-critical only inside the label model) — **A and B agree and I concur.** A's Q4
  breakdown of the three fingerprint mechanisms (`dev_binary_matches_source` = staleness/optim;
  `ensure_source_state_matches` = tree-changed-during-build, cheap to keep; `validate_dev_binary
  _matches_source` = publish-time guard that dies with the store) is accurate and useful.

Overall confidence in the reconciled picture: **high** on mechanism, **medium-high** on the
policy call about deleting rollback (that is a judgment, not a proof).

---

## Target architecture (the dream + the comfort, concretely)

Two reload domains, each with a **single** target:

1. **Stable / "no nonsense" domain (the comfort).**
   - The reload/spawn target is the **nix profile binary** (`~/.nix-profile/bin/jcode`, a store
     symlink) — atomic by construction, moved only by `nix profile upgrade`/home-manager
     rebuild.
   - Activate it by having the nix wrapper export **`JCODE_NIX_MANAGED=1`** (a one-line
     `makeWrapper`/`wrapProgram` in `nix/package.nix`, or `home.sessionVariables` in the HM
     module). That flips `client_update_candidate` / `shared_server_update_candidate` /
     `preferred_reload_candidate` to the launcher for the non-selfdev case **with the code that
     already exists** (`paths.rs:565,603,698`).
   - **Fallback = a nix generation, not a hand-rolled channel.** Two cheap options, keep both:
     - `nix profile` generations: `nix profile rollback` reverts to the last-good binary
       atomically. This is the direct replacement for the version store's rollback.
     - A pinned escape hatch: `nix run github:1jehuang/jcode/<known-good-rev>#jcode`, or a
       `result-stable` symlink you bump deliberately. This replaces `JCODE_MIGRATE_BINARY ->
       stable` (`hot_exec.rs:59-77`, `debug_cmds.rs`).
   - This lets you **delete stable/current/shared-server/canary channels and `versions/`** while
     keeping a *real* known-good fallback.

2. **Self-dev inner loop (the dream: fast, plug/unplug at runtime).**
   - Keep the fast `cargo build` into `target/selfdev` (`scripts/dev_cargo.sh` unchanged — it is
     good and heavily tuned).
   - **Do not exec the bare `target/selfdev/jcode`.** Replace the copy-into-`versions/<label>` +
     channel fan-out with a **single atomic publish into one fixed path**, e.g.
     `~/.jcode/run/jcode` (or a repo-local `target/selfdev/jcode.published`): reuse the existing
     `copy_binary_to_staging_path` + `publish_staged_binary` primitives (stage in the dest dir,
     fsync, smoke-test the staged temp, `rename` into the one fixed path). No `versions/` tree,
     no `current`/`shared-server`/`canary`/`stable` symlinks, no markers.
   - Both the self-dev daemon and self-dev clients reload that **one** fixed atomic path. This
     keeps the truncation invariant (and actually *tightens* it for clients, which today accept
     the bare-path risk).
   - Alternative if you want zero bespoke publish code: have selfdev `nix build` and reload
     `result`. This is cleaner but slower per iteration than incremental `cargo`; keep it as an
     option, not the default.

**Preserve (correctness):**
- Atomic swap of the reload target (nix `result`/profile, or the retained
  stage+fsync+rename-into-one-path). Non-negotiable (verified crux).
- execv-failure-keeps-current-process-alive (`reload.rs:207-242`, `hot_exec.rs:102-125`).
- Reload-recovery intents for in-process headless/inline agents (`reload.rs` recovery,
  `comm_session.rs:682-685`).
- Wrapper->payload resolution (`resolve_binary_payload`, used at `util.rs:114-118`,
  `paths.rs:716-721`) — needed because the nix launcher can be a `makeWrapper` wrapper.
- Smoke-test-before-activate (`smoke_test_staged_binary_for_install` / `smoke_test_binary` /
  `smoke_test_server_binary`, `lib.rs:407,528-535` and the server smoke in
  `reload.rs`/`selfdev/reload.rs:301`). Keep it gating the single atomic publish.
- `ensure_source_state_matches` (tree-changed-during-build) — cheap, keep, decoupled from any
  store.

**Delete (dead / drift-only in a single atomic path world):**
- Subsystem A entirely: `crates/jcode-update-core`, `scripts/install.sh`,
  `.github/workflows/release.yml`, `update.rs` GitHub/source install path,
  `repair_stale_shared_server_after_update_check`, `install_local_release` (GitHub arm),
  `advance_shared_server_if_tracking_stable` callers.
- `stable`/`current`/`shared-server`/`canary` channel symlinks + all update/repair/advance/
  tracks-stable helpers (`lib.rs:1114-1400` region), the version store
  (`install_binary_at_version`, `version_binary_path`, `~/.jcode/builds/versions/`), the
  multi-source resolvers collapsed to the fixed path
  (`client_update_candidate`/`shared_server_update_candidate`/`preferred_reload_candidate`),
  and the cross-flavor candidate scan + forced-stale refusal in `server/util.rs` (moot with one
  monotonic path — but see Risk R3 before deleting the no-downgrade guard).
- The pending-activation/canary rollback (`PendingActivation`, set/complete/rollback/reconcile,
  manifest canary fields) — **policy deletion**: you trade auto-revert-of-a-bad-build for
  "fix source + rebuild, or `nix profile rollback` / `nix run <good-rev>`." Acceptable for a
  sole maintainer; call it out explicitly.
- The three fingerprint publish-guards die with the store except `ensure_source_state_matches`
  (keep).

---

## Staged migration plan (new work-graph nodes replacing F20)

F20 ("Extend hermetic installer and Rust updater acquisition fixtures through real local
daemon reload and rollback", `WORK_GRAPH.json:1839-1865`, state `pending`) is **the wrong
investment** now — it hardens subsystem A, which we are retiring. Replace it with F20a-F20e.
Each is independently landable; ordering is by dependency, not by force.

**F20a — Activate the nix single-fixed-path for non-selfdev (make the dormant hook live).**
- Owned: `nix/package.nix`, `nix/modules/home-manager.nix`, `docs/NIX.md`.
- Do: export `JCODE_NIX_MANAGED=1` from the nix wrapper (makeWrapper or HM
  `home.sessionVariables`). No Rust change — the resolvers already honor it.
- Acceptance gates:
  - In a nix install, `client_update_candidate`/`shared_server_update_candidate`/
    `preferred_reload_candidate` all resolve to the profile binary for a non-selfdev session
    (assert via `jcode doctor` drift check, `doctor.rs:473-491`).
  - `server reload --force` on the daemon re-execs the profile binary and never touches
    `~/.jcode/builds/*`.
  - A home-manager rebuild that bumps the package is picked up by a subsequent
    `server reload --force`.
- Independently landable: **yes** (pure packaging; reversible by unsetting the var).

**F20b — Provide the stable fallback via nix generations (retire `JCODE_MIGRATE_BINARY ->
stable`).**
- Owned: `nix/modules/home-manager.nix`, `docs/NIX.md`, `src/cli/hot_exec.rs` (migrate arm),
  `debug_cmds.rs` (migrate command).
- Do: document/support `nix profile rollback` and a pinned `nix run <rev>` as the "drop to last
  good build" path; repoint the migrate escape hatch at "re-exec the profile binary" or a pinned
  rev instead of the `stable` channel.
- Acceptance gates:
  - A deliberately-broken build can be escaped with a single documented command that lands on a
    known-good binary, with no `~/.jcode/builds/stable` present.
  - `nix profile rollback` restores the previous binary and a fresh `jcode` starts on it.
- Independently landable: **yes** (depends on F20a for the profile being authoritative).

**F20c — Collapse the self-dev reload to one atomic fixed path (the real work).**
- Owned: `crates/jcode-build-support/src/paths.rs`, `crates/jcode-build-support/src/lib.rs`,
  `crates/jcode-app-core/src/tool/selfdev/*`, `src/cli/hot_exec.rs`, `src/cli/selfdev.rs`.
- Do: introduce a single `selfdev_run_path` (one fixed location). Replace
  `publish_local_current_build_for_source` (channels+versions) with a **stage+fsync+smoke+
  rename into that one path**, reusing `copy_binary_to_staging_path`/`publish_staged_binary`.
  Point selfdev `client_update_candidate`/`preferred_reload_candidate` and the daemon's
  `server_update_candidate` at it. Keep the daemon's `exec_candidate` invariant (never exec a
  non-atomic path).
- Acceptance gates (the hill to climb):
  - **Atomicity regression test survives:** an analogue of `concurrent_source_truncation...`
    (`lib.rs:611-669`) but for the new single path — truncate the source mid-publish, assert the
    fixed path is intact. This is the go/no-go gate.
  - A self-dev `/reload` under a concurrent `cargo build` never execs a partial ELF (stress
    test: loop reload while looping build; assert the daemon and a headed client always start or
    cleanly ENOENT-retry, never SIGILL/"exec format error").
  - Smoke-test-before-activate still gates the publish (broken build never becomes the fixed
    path; assert via the `failed_smoke_test_leaves_no_version_entry` analogue, `lib.rs:671+`).
  - execv-failure still leaves the running daemon alive (`reload.rs:207-242` unchanged).
- Independently landable: **yes**, but it is the load-bearing change; land behind the atomicity
  test.

**F20d — Delete the channels, version store, and pending-activation rollback.**
- Owned: `crates/jcode-build-support/src/lib.rs` (channel/version/manifest region),
  `crates/jcode-build-support/src/paths.rs` (multi-source resolvers), `server/util.rs`
  (candidate scan / forced-stale — keep the no-downgrade guard unless F20c proves monotonicity),
  `tool/selfdev/reload.rs` (pending-activation), `crates/jcode-selfdev-types`.
- Acceptance gates:
  - No caller references `current`/`stable`/`shared-server`/`canary`/`versions` after removal
    (grep-clean; the census in `docs/fork/ideal-base/evidence/W0.2/source_census.md` is the
    checklist).
  - Full test suite + `nix flake check` green twice at one commit (inherit F21's "twice" gate).
  - Documented policy note: "no auto-revert of a post-ready crash-looping build; use
    `nix profile rollback`."
- Independently landable: **after F20c** (deleting the store before the fixed path exists would
  break selfdev).

**F20e — Retire subsystem A.**
- Owned: `crates/jcode-update-core` (remove), `scripts/install.sh` (remove),
  `.github/workflows/release.yml` (remove/neuter), `crates/jcode-app-core/src/update.rs`
  (remove GitHub/source install), `hot_exec.rs` (`hot_update`/`run_auto_update`/`run_update`).
- Acceptance gates: binary builds without the update crate; no `download_and_install` reachable;
  `jcode` no longer advertises self-update; docs updated to "update via nix."
- Independently landable: **yes** (fully orthogonal to B; can even land first as a pure
  deletion once you accept no curl|sh users exist).

Sequencing: **F20a and F20e can land immediately and in parallel** (packaging + dead-code
deletion). **F20c is the keystone**, gated by its atomicity test; **F20b** rides on F20a;
**F20d** rides on F20c. Fold the "run twice from clean" idea from the old F21 into F20d's gate.

---

## Risk register

| # | Risk | Mitigation | How we detect it went wrong |
|---|------|------------|-----------------------------|
| R1 | **Truncated-ELF race** if any reload target becomes a non-atomic in-place-written path (the crux). | Keep every exec target atomic: nix `result`/profile, or stage+fsync+smoke+rename into one fixed path. Preserve the daemon's `exec_candidate` invariant. | The F20c atomicity regression test (source-truncation) plus a build-vs-reload stress loop; failure surfaces as "exec format error"/SIGILL or a zero/partial binary on the fixed path. |
| R2 | **No fallback under plain `nix run`** (no generation) — a bad build wedges the next spawn with nothing to roll back to. | Standardize on `nix profile install` (generations) as the stable domain; document `nix profile rollback` and pinned `nix run <rev>`. Do not rely on `nix run` of the default package as the only entry. | `jcode doctor` drift check (`doctor.rs:473-491`) reports running-vs-installed mismatch; a deliberately-broken build test must be escapable in one command. |
| R3 | **Deleting the no-downgrade guard** (`guarded_reload_target`/`forced_stale_shared_server_refusal`, `util.rs`) before proving the fixed path is monotonic could let a stale binary become the target. | Keep the guard until F20c demonstrably makes the fixed path monotonic (nix store paths only move forward on rebuild; a repo publish only moves forward by mtime). Treat mtime-uncertainty as "do not downgrade" (existing behavior). | Reload lands on an older binary than the running one; detect via a reload-target mtime assertion in the stress test and via the daemon's own downgrade log lines. |
| R4 | **Losing auto-revert of a post-ready crash-loop** (rollback never covered this anyway, but the store's pending-activation is a partial net that goes away). | Accept as explicit policy for a sole maintainer; provide `nix profile rollback` and a fast "rebuild from good source" path. Optionally add a lightweight crash-loop counter that flips the daemon back to the profile/stable path after N fast exits. | A reload that boots then crash-loops keeps respawning onto the bad build; detect via a restart-rate watchdog / the existing `last_crash` manifest signal (repurposed) or systemd restart backoff. |
| R5 | **Self-dev branch still bypasses the override** after F20a, so flipping the env var alone silently leaves selfdev on the old channels and F20d deletion breaks selfdev. | Do F20c *before* F20d; never delete the store until the selfdev fixed path exists and its tests pass. | Grep gate in F20d (no channel/version references remain) + selfdev `/reload` integration test must pass on the new path before deletion merges. |
| R6 | **Wrapper/payload identity confusion** if the nix launcher is a `makeWrapper` script and comparisons look at the wrapper instead of the payload (the historic "phantom downgrade" bug). | Keep `resolve_binary_payload` on every mtime/identity comparison (already used at `util.rs:114-118`, `paths.rs:716-721`); add a test with a wrapper-shaped launcher. | Every release install/reload reports a spurious "update"/"downgrade" loop; detect via the reload-decision logs and a wrapper-shaped fixture test. |
| R7 | **In-process agent recovery breaks** if the reload path change perturbs recovery-intent replay. | Treat reload-recovery (`reload.rs` recovery, `comm_session.rs:682-685`) as orthogonal and untouched by F20c; add a reload-with-live-inline-agents test. | After a self-dev reload, previously-inline/headless agents are not re-spawned; detect via a swarm-reload integration test asserting agent continuity. |

---

## Confidence + residual unknowns

- **Confidence:** high on all four verified claims (I re-read each site). High that the collapse
  is safe *provided* the atomic-path constraint (R1) is honored. Medium-high on the policy call
  to delete pending-activation rollback (that is a judgment about acceptable risk for a
  sole-maintainer fork, not a proof).
- **Residual unknowns worth a 30-minute check before F20c lands:**
  1. Exactly which fixed location to use for the self-dev published path (`~/.jcode/run/jcode`
     vs a repo-local published copy) and whether `LD_LIBRARY_PATH`/wrapper needs apply there —
     confirm the nix devshell selfdev build produces a directly-execable binary or a wrapper
     (affects whether `resolve_binary_payload` matters on the selfdev path).
  2. Whether any non-selfdev/multi-machine scenario relies on the shared-server channel lagging
     stable (both reports believe not for a sole maintainer; confirm no synced-`~/.jcode/builds`
     across machines). If a second machine ever shares a daemon, R3/R4 change weight.
  3. Whether `nix profile` (generations) vs a home-manager-managed package is the intended
     "stable domain" — they give different rollback ergonomics (`nix profile rollback` vs a HM
     generation switch). Pick one and standardize F20b on it.
