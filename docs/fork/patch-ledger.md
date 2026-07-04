# Downstream patch ledger

Track downstream patches that may need to be upstreamed, watched, or retired.
Permanent fork behavior can stay here too, but every temporary shim must have a
retirement condition and validation command.

| Patch | Class | Status | Upstream ref | Retire condition | Validation |
|---|---|---|---|---|---|
| OpenAI-compatible schema format stripping | `compat(openai)` | `temporary-shim` | none yet | Upstream accepts MCP/fetch schemas with unsupported JSON Schema `format` keywords removed or handles them before OpenAI submission. | `cargo test -p jcode-provider-core openai_schema` |
| APM MCP and skill surface loading | `feature(agent-tools)` | `permanent-downstream` | none | Keep unless upstream adopts equivalent APM/tool-surface loading. `skill.rs` discovery is now an additive prepended `.apm`/`.agents` loop (upstream's `.jcode`/`.claude` blocks byte-identical), so it no longer conflicts on rebase and is a clean upstream-PR candidate. | `cargo test -p jcode-base skill` |
| MCP config: env placeholder expansion + APM/agents paths | `feature(agent-tools)` | `permanent-downstream` | none | During the v0.32.0 sync, upstream independently converged on the fork's `mcpServers` alias, transport/`is_stdio` skipping, and Claude/Codex import. The fork's wholesale custom `Deserialize` was retired in favor of upstream's design plus two additive seams in `McpConfig::load()`: exact `${VAR}` env expansion and `.apm`/`.agents` project-local MCP manifest paths. | `cargo test -p jcode-base mcp::` |
| OAuth tool exposure as an allowlist | `feature(agent-tools)` | `permanent-downstream` | none | Upstream forwards the full registry to the Anthropic OAuth endpoint; the fork narrows it to an explicit `OAUTH_EXTRA_TOOLS` allowlist (websearch/webfetch/nix) appended to the curated identity tools, with a no-leak guarantee. Upstream's `oauth_format_tools_keeps_full_custom_toolset` test asserts the old forward-all behavior and is dropped on rebase. | `cargo test -p jcode-provider-anthropic` |
| Shared `git rerere` conflict resolutions | `feature(fork-maint)` | `permanent-downstream` | none | Keep; fork-maintenance tooling so the 6h CI rebase self-heals recurring conflicts. `scripts/rerere-cache.sh` + `scripts/rerere-rebase.sh`, shared via tracked `.rerere-cache/`, wired into `nix.yml` sync-upstream and the devShell. | `shellcheck scripts/rerere-cache.sh scripts/rerere-rebase.sh && actionlint .github/workflows/nix.yml` |
| `jcode doctor` binary-identity diagnostics | `feature(fork-maint)` | `permanent-downstream` | none | Keep; surfaces client vs daemon build identity/origin/verdict for the self-dev + Nix divergence problem. Zero new protocol (reads the server registry). | `cargo test -p jcode --lib doctor` |
| Build-provenance source-dir stamp (NS4/G4) | `feature(fork-maint)` | `permanent-downstream` | none | Keep; stamps the source checkout path the binary was built from (`jcode_build_meta::BUILD_SOURCE_DIR`) and surfaces it as `doctor`'s `built-from:` line for source/selfdev origins only. Kept off `buildDepsOnly` (set in `package.nix` `buildMeta`, pinned to `nix-store` for Nix builds) so it never perturbs the crane dependency cache. Resolves "which checkout produced the running daemon" (G4 in `SELFDEV_NIX_DAEMON_DIVERGENCE.md`). | `cargo test -p jcode --lib doctor` |
| Protocol/build-version handshake + re-exec (NS1/G1+G3) | `feature(fork-maint)` | `permanent-downstream` | none | Keep; additive `Subscribe` fields (`protocol_version` + `build_hash`) and a `HandshakeVerdict` event let the daemon return a typed `Compatible`/`IncompatibleReconnect` verdict (`jcode_protocol::HandshakeCompatibility::evaluate`), and the TUI client acts on incompatibility by re-execing into the matching launcher (`preferred_reload_candidate`) or refusing rather than attaching blindly. Legacy clients advertise nothing and never receive the event, so the seam is roundtrip-compatible. Fixes G1 (no version gate) + G3 (no compatible-vs-incompatible notion). Re-exec guard env `JCODE_NS1_REEXECED` prevents relaunch loops; full wrapper-aware identity is NS2/FR-70. | `cargo test -p jcode-protocol handshake_verdict && cargo test -p jcode-app-core 'server::handshake' handshake_emits handshake_sends && cargo test -p jcode-tui --lib handshake` |
| Nix-wrapper-aware binary identity (NS2/G2) | `feature(fork-maint)` | `permanent-downstream` | none | Keep; extends `resolve_binary_payload` so freshness/identity checks unwrap not just the release `<stem>-*.bin` sibling but also a Nix/`makeWrapper`/`_ai`-style wrapper that `exec`s an **absolute** store path to the real `jcode` ELF (basename-filtered, unique-or-refuse, depth-bounded). Before this, the 842-byte Nix wrapper was treated as its own payload, so a Nix-wrapped daemon and a self-dev build compared wrapper-vs-payload mtimes and could look like phantom updates of each other. Closes the cross-boundary half of G2 in `SELFDEV_NIX_DAEMON_DIVERGENCE.md`. Verified live on this host: the `_ai` wrapper resolves to `/nix/store/...-jcode/bin/jcode`. | `cargo test -p jcode-build-support resolve_binary_payload phantom` |
| Herdr lifecycle and pin reporting | `feature(herdr)` | `permanent-downstream` | none | Keep while Herdr is a 4nix-local harness integration. | `cargo check --workspace` |
| ACP session config options | `feature(acp)` | `planned-upstream-pr` | none yet | Retire or reduce once upstream exposes equivalent ACP session config controls. | `cargo check --workspace` |
| Auth refresh warning suppression | `compat(auth)` | `temporary-shim` | none yet | Upstream no longer warns on multi-provider model state after auth refresh. | `cargo check --workspace` |
| dev_cargo clang fallback | `distro(dev)` | `local-only` | none | Keep while local development environments may lack clang. | `scripts/dev_cargo.sh --help` |
| Nix dependency-cache stability (git stamp out of `buildDepsOnly`) | `distro(nix)` | `permanent-downstream` | none | Keep; this is a packaging correctness property, not an upstream concern. Watch that future `package.nix` edits never move `JCODE_BUILD_GIT_*` back into `commonArgs`. | gitHash A vs B must yield identical `cargoArtifacts.drvPath` (see commit `02bcc628`) |
| Workspace lib-test hermeticity | `feature(test-hygiene)` | `planned-upstream-pr` | none yet | Clean upstream-PR candidate: every fix isolates ambient state instead of changing product behavior. Retire individual fixes if/when upstream lands the same isolation. Includes one real product fix (`JCODE_SHOW_AGENTGREP_OUTPUT` missing from `CONFIG_ENV_KEYS`). | `cargo test --workspace --lib --exclude jcode-tui` (Linux, clean `JCODE_HOME`) |
| Fork CI: macOS-first gate + tiered test execution | `distro(ci)` | `permanent-downstream` | none | Owned by the distro/nix layer (see docs/BRANCHING.md "CI ownership"). fork-ci.yml gates main: macOS build + integration tests blocking, macOS lib tests advisory (promote after a clean week), Linux tests advisory on push / blocking weekly. Upstream ci.yml is dispatch-only and byte-close to vendor. `jcode-tui` lib tests stay compile-only everywhere (see below). | `.github/workflows/fork-ci.yml`; `scripts/fork-health.sh` |
| Fork security policy: audit.toml + Security workflow | `distro(ci)` | `permanent-downstream` | none | Triaged advisory ignores live in `.cargo/audit.toml` (cargo-audit loads it natively); fork-only triage rows in `docs/fork/SECURITY_TRIAGE.md`; gate + weekly full report in `.github/workflows/security.yml`. Vendor `security_preflight.sh` ignore array stays pristine. | `cargo audit` exits 0 with audit.toml, flags the 4 fork-triaged IDs without it |
| Mermaid rendering enabled by default | `feature(mermaid)` | `permanent-downstream` | none | Upstream disabled Mermaid ("renderer is unstable") behind the `renderer`/`mermaid-renderer` cargo features, a `JCODE_ENABLE_MERMAID=1` runtime opt-in, and `DiagramDisplayMode::None`. The fork ships it on via a `mermaid` default feature, default-on runtime gate, and `Pinned` default. Keep while we want diagrams to render for Kitty/iTerm2/Sixel terminals. | `cargo test -p jcode-tui-mermaid --features renderer --test smoke_render` |
| Mermaid renderer SVG font-family quote fix | `compat(mermaid)` | `planned-upstream-pr` | `mermaid-rs-renderer` >v0.2.1 emits well-formed XML (escapes/single-quotes nested font-family names) | `mermaid-rs-renderer` v0.2.1 writes CSS font stacks with unescaped nested double quotes into the SVG `font-family` attribute, so `usvg::Tree::from_str` rejects every diagram. `sanitize_font_family_quotes()` rewrites the inner quotes before parsing. Retire when the upstream renderer is fixed (ideally PR it there). | `cargo test -p jcode-tui-mermaid --features renderer sanitize` |

## jcode-tui lib tests: compile-only (matches upstream)

`jcode-tui`'s lib tests are compiled but **not executed** in CI, on both this
fork and upstream (upstream's CI is `--lib --bins --no-run` for the whole
workspace). When first executed on a clean runner ~45 of them fail, and the
failures are not environment coupling: they assert UI/onboarding/model-catalog
behavior that upstream's own production code has since changed. Verified against
`upstream/master`:

- `model_picker_recommended_route_is_provider_aware` asserts `claude-opus-4-7`
  and DeepSeek are recommended, but `RECOMMENDED_MODELS` is `["gpt-5.5",
  "claude-opus-4-8"]` upstream too.
- The `pending_queued_dispatch`-on-remote-startup cluster asserts eager dispatch
  that `apply_restored_reload_input` deliberately defers for remote sessions
  ("the remote post-connect/history/tick paths will dispatch once it is safe").
- Others depend on the new post-login onboarding flow, reactor context for
  `handle_login_completed`/`handle_server_event` `tokio::spawn`, theme color
  drift, and snapshot nondeterminism.

These are stale identically upstream, so rewriting them in the fork would be pure
divergence with no payoff (the suite still will not run). They keep compile
coverage via the `--workspace ... --no-run` step. Revisit only if upstream starts
executing the `jcode-tui` lib suite.

### Triage data and the "don't grind, grow our own" decision

With the `mermaid` feature enabled, 47 lib tests fail. Running each one alone vs
in the full suite (see `docs/fork/jcode-tui-test-triage.md`) splits them:

- **13 pass alone, fail in the full suite** = cross-test global-state pollution.
  The product is fine; the harness is non-hermetic. These cannot be fixed by
  editing the test body, only by adding isolation infrastructure (high effort,
  high divergence).
- **34 fail even alone** = stale assertions (model catalog, the remote
  `pending_queued_dispatch` deferral, the 8->24MB full-prep cache threshold) plus
  ~2 structurally non-runnable (`current_exe()` is the test runner, never
  `jcode`).

Decision: do **not** grind these to green. The suite is `--no-run` in CI (upstream
parity), ~28% are unfixable without harness surgery, and the rest each require
reverse-engineering a deliberate product change to update an assertion that still
will not run. Instead, grow our own small, hermetic, `JCODE_HOME`-isolated tests
for fork-owned behavior as we touch it. `crates/jcode-tui-mermaid/tests/smoke_render.rs`
is the model: it tests real behavior we care about and passes deterministically.



Statuses:

- `local-only`
- `temporary-shim`
- `planned-upstream-pr`
- `submitted-upstream-pr`
- `waiting-upstream-release`
- `permanent-downstream`
- `retire-candidate`
- `retired`
