# Daemon / Self-Dev Binary Divergence: Friction Analysis and Proposed Path

Date: 2026-06-26
Status: research + recommendation (no code shipped; proposes next steps)
Scope: the friction around the jcode shared daemon, self-developed binaries that differ
substantially from the running one, and the NixOS/Home-Manager packaging.

## 1. The friction, stated precisely (verified in code + on this machine)

There are **three diverging binary identities** for one tool, plus a protocol gap:

1. **Nix-store binary** on `$PATH`: `~/.nix-profile/bin/jcode -> /nix/store/...-jcode/bin/jcode`,
   from `home.packages` (`inputs.jcode.packages.<sys>.jcode`, `github:jerudnik/jcode/main`).
   Immutable, declarative, only changes on a Home-Manager switch. On this machine it is
   itself a **wrapper script** (the `_ai` Phase-injection wrapper) that sets `PATH`/env and
   execs the real payload.
2. **Self-dev mutable launcher**: `~/.local/bin/jcode -> ~/.jcode/builds/current/jcode`, backed
   by a versioned store (`~/.jcode/builds/{versions/<hash>, current, stable, shared-server,
   canary}` + `*-version` marker files), with atomic channel-symlink swaps, smoke tests,
   manifest + pending-activation + rollback (`jcode-build-support`). This is a genuinely
   sophisticated zero-downtime swap system.
3. **Source checkouts**: the edit tree (`~/infrastructure/jcode`) and a separately
   auto-cloned `~/.jcode/source/jcode` (`selfdev setup` clones it if absent). These drift:
   this session began with the daemon's source at `80cfc8bb` while the edit tree was at
   `b83b6668`, and the build store still holds `0.29.4-dev-herdr-80cfc8bb`.

**The reload model (verified):** self-dev is a *session-local canary capability* on one
shared daemon (`UNIFIED_SELFDEV_SERVER_PLAN.md`, status Implemented). A reload re-execs the
**whole shared daemon** into a new binary; the TUI can `exec()` because the terminal owns the
window, and all clients reconnect. Binary selection is **mtime + channel** based
(`server_has_newer_binary`, `newest_reload_candidate`, `reload_exec_target`), with a real
wrapper/payload-resolution fix already in place (commit `8cc66bc3`) and a downgrade guard.

**The core gaps (where the friction actually bites):**

- **G1 No protocol/version compatibility gate.** The `Subscribe` handshake (`wire.rs`) carries
  no build/protocol version; the server never rejects a version-mismatched client. So a reload
  into a *substantially different* binary is trusted blindly. `jcode_build_meta::VERSION` is
  used only to *write reload state*, not to negotiate.
- **G2 Two launcher ownership models collide.** Nix wants an immutable store path on `$PATH`;
  self-dev wants a mutable swappable launcher. Today they coexist only because the mutable
  `~/.local/bin` is a *different path* from the Nix `~/.nix-profile/bin`. Which one runs
  depends on `$PATH` order, and the Nix wrapper-vs-payload indirection re-introduces exactly
  the identity confusion that `8cc66bc3` fixed internally, now across the Nix boundary.
- **G3 "Substantially different binary" has no first-class meaning.** Freshness is mtime/hash;
  there is no notion of *protocol-compatible* vs *incompatible* reload, so the system cannot
  choose "hot re-exec" vs "must drain/re-instance" deliberately.
- **G4 Source-of-truth ambiguity.** "The source the daemon was built from" (`~/.jcode/source`)
  vs "the source you are editing" diverge silently; a reload from the edit tree and a reload
  from the clone can land different binaries with the same version label intent.

## 2. What the field does (sourced; verdicts are for *this* use case)

### A. Zero-downtime daemon binary swap

- **nginx**: `USR2` -> new master re-execs, old workers drain (`-s`/`QUIT`). **PROMISING** model
  for "new process inherits listeners, old drains." (nginx control docs)
- **HAProxy**: master-worker `SIGUSR2` reload + `-sf` to stop old workers after handoff.
  **PROMISING**; the clean drain-old/accept-new template. (HAProxy management guide)
- **Envoy hot restart**: child inherits listeners + shared memory; works but has real caveats
  (wrapper helpers, edge instability). **PROMISING with caveats.**
- **systemd socket activation**: systemd owns the listening socket and passes fds via
  `LISTEN_FDS`/`sd_listen_fds` to each freshly exec'd service. **PROMISING** and the most
  Nix/declarative-friendly: the *socket outlives the binary*, so a swap never drops the
  listener. tmux even integrates with systemd for process lifecycle.
- **SCM_RIGHTS fd passing**: explicit user-space fd handoff. **PROMISING (advanced)**; more
  complexity than jcode needs given exec-inheritance already works.
- **tmux**: no trivial atomic binary swap of a running server across state/ABI changes; a real
  upgrade restarts the server. **TRIED-AND-FAILS** for substantial changes — which *validates*
  jcode's "re-exec + reconnect" over in-place patching.

### B. Protocol / state version negotiation

- **LSP**: capability-based `initialize` handshake (advertise features), restart on
  incompatibility. **PROMISING** — capability negotiation beats a strict numeric wire version.
- **gRPC/Protobuf**: additive-field, reserved-tag schema evolution for forward/backward compat;
  explicit version or blue/green for breaking changes. **PROMISING** — the discipline for the
  evidence/session/wire types.
- **Guarded upgrade**: handshake advertises version/capabilities; client detects mismatch and
  either refuses+reconnects or falls back to a reduced feature set. **PROMISING** — directly
  fills G1/G3.
- **Migration-by-decomposition**: move mutable session state into a small *versioned* external
  store with migrations instead of an opaque in-memory graph. **PROMISING** — jcode already has
  versioned `SessionLogEvent`/session snapshots; lean into that.
- **Dual-running window**: old + new daemon side by side behind a dispatcher until clients
  migrate. **PROMISING but operationally heavy** — probably overkill for a single-user laptop.

### C. Reconciling immutable Nix with a fast mutable dev loop

- **Nix wrappers (makeWrapper/wrapProgram)**: the wrapper-vs-payload split is a *known* cause of
  identity-check confusion. **TRIED-AND-FAILS for naive freshness checks** — confirms G2; jcode
  must resolve through the wrapper to the payload across the Nix boundary too.
- **`nix develop` / devShell for iteration**: run the mutable cargo build from a devshell;
  production uses the store binary. **PROMISING (Nix-native)** — the cleanest separation.
- **Intentional shadowing**: a mutable `~/.local/bin/jcode` deliberately shadowing the Nix
  profile (PATH order). **PROMISING but manual/fragile** — this is essentially today's setup,
  and its fragility is the friction.
- **`programs.<tool>.package` / overlay override**: declaratively point the entry point at a
  stable package, with a per-user devshell alias to the mutable build in dev. **PROMISING
  (Nix-native)** — the declarative way to express "store in prod, cargo in dev."
- **Honest identity check**: resolve the wrapper to the payload and/or use a version/capability
  handshake instead of path/mtime. **PROMISING** — pairs with G1.

### D. Multiple-checkout / run-from-source divergence

- **Emacs daemon (`restart-emacs`)**: exec-restart the daemon while restoring frames/socket;
  works when the surrounding protocol is stable. **PROMISING** — same shape as jcode's re-exec.
- **Practical rule**: when schema changes are substantial, prefer **reconnect-and-fail-fast**
  (detect incompatibility, force a clean reconnect/restart) over trusting the swap. **PROMISING**
  — the governing principle for G3.
- **Subsecond (Dioxus hot-patch)**: jump-table indirection patches functions in place, but
  **cannot hot-reload struct layouts** — if a struct's size/alignment changes it crashes unless
  the framework throws out old state and re-instances. **TRIED-AND-FAILS for substantial
  changes** — strong evidence that jcode's "full re-exec + reconnect" is the *correct* choice
  whenever state shape changes; in-place patching is a trap for a stateful daemon.

## 3. Synthesis: what is actually wrong and what is already right

**Already right (do not rebuild):** the versioned build store, atomic channel symlinks,
smoke-test-before-activate, manifest + rollback, wrapper/payload resolution, the downgrade
guard, and the "re-exec the whole daemon + reconnect" choice (the field validates re-instancing
over in-place patching for a stateful daemon).

**The real, fixable gaps, in priority order:**

1. **G1/G3 — add a version/capability handshake and an explicit compatibility verdict.** This is
   the highest-leverage fix and the one the research most strongly supports (LSP, gRPC,
   guarded-upgrade, fail-fast). It turns "reload into a substantially different binary" from an
   untyped risk into a deliberate decision: *compatible -> hot re-exec; incompatible -> drain and
   reconnect cleanly with a clear message*, never a silent wedge.
2. **G2 — make the Nix/dev launcher boundary explicit and declarative.** Stop relying on
   accidental `$PATH` shadowing. Express "store binary in prod, mutable dev build in a devshell"
   as a 4nix devShell + a documented launcher policy, and make jcode's identity checks resolve
   the Nix wrapper to its payload (the same fix as `8cc66bc3`, extended across the store
   boundary).
3. **G4 — make the build's source provenance unambiguous.** Record, in the build manifest and
   the version label, *which checkout + commit + dirty-state* a binary was built from, and
   surface it so "the daemon is running a binary built from a different checkout than you are
   editing" is visible, not inferred.
4. **(Optional, later) socket stability.** If reload drops ever become annoying, adopt
   systemd-socket-activation (or an fd-passing handoff) so the *listening socket outlives the
   binary* and clients never see a closed socket during a swap. Nix/NixOS makes socket
   activation trivial to declare. Defer until measured.

## 4. Proposed next steps (smallest-first, each independently shippable)

- **NS1 (smallest, highest leverage): protocol/version handshake + compatibility verdict.**
  Add `protocol_version` (and `build_hash`) to the `Subscribe` handshake; the server compares
  and returns a typed verdict (`Compatible` / `IncompatibleReconnect`). On incompatible, the
  client shows a clear message and re-execs to the matching launcher instead of silently
  attaching. Pure protocol + handshake; testable with a fixture; no Nix changes. Directly fixes
  G1/G3 and is the research's top recommendation.
- **NS2: wrapper-aware identity across the Nix boundary.** Extend `resolve_binary_payload` usage
  so freshness/identity checks unwrap the Nix wrapper script to the store payload, and add a test
  that a Nix-wrapped jcode is not seen as a phantom update vs a self-dev build. Fixes the
  cross-boundary half of G2.
- **NS3: declarative dev-vs-prod launcher policy in 4nix.** A `nix develop` devShell (or a
  documented `JCODE_INSTALL_DIR`/launcher convention) that makes "use the mutable self-dev build
  here, the store binary everywhere else" explicit and reproducible, instead of `$PATH` luck.
  Document it in the jcode fork (reusable) and consume in 4nix.
- **NS4: build provenance in the manifest.** Stamp source checkout path + commit + dirty into
  the build manifest/version label and surface it in `selfdev status` / chrome, so checkout
  divergence is visible. Fixes G4.
- **NS5 (deferred, measure first): socket-activation / fd-handoff** so the listener survives a
  binary swap. Only if reload reconnect cost proves annoying in practice.

## 5. Validation stance

- NS1/NS2/NS4 are unit/integration-testable in an isolated `JCODE_HOME` sandbox (the existing
  selfdev test pattern), no live-daemon risk.
- NS3 is validated by `nix develop` entering the devshell and `which jcode` resolving as
  intended, plus a Home-Manager build.
- NS5 would need a VM/host test that a reload across a socket-activated unit keeps clients
  connected; explicitly deferred.

## Sources

DeepWiki: tmux (client/server + Unix socket + imsg; systemd integration), Dioxus Subsecond
(jump-table hot-patch; cannot reload struct layouts -> re-instance). Parallel research:
nginx/HAProxy/Envoy hot restart, systemd socket activation, SCM_RIGHTS, LSP capability
handshake, gRPC/Protobuf schema evolution, Nix makeWrapper/devShell/package-override, Emacs
restart-emacs. Codebase: `UNIFIED_SELFDEV_SERVER_PLAN.md`, `jcode-build-support`,
`server/util.rs`, `tool/selfdev/*`, commit `8cc66bc3`, `4nix/.../ai-client-jcode.nix`.
