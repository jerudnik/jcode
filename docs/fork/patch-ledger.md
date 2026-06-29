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
| `jcode doctor` binary-identity diagnostics | `feature(fork-maint)` | `permanent-downstream` | none | Keep; surfaces client vs daemon build identity/origin/verdict for the self-dev + Nix divergence problem. Zero new protocol (reads the server registry). | `cargo test -p jcode --lib cli::commands::doctor` |
| Herdr lifecycle and pin reporting | `feature(herdr)` | `permanent-downstream` | none | Keep while Herdr is a 4nix-local harness integration. | `cargo check --workspace` |
| ACP session config options | `feature(acp)` | `planned-upstream-pr` | none yet | Retire or reduce once upstream exposes equivalent ACP session config controls. | `cargo check --workspace` |
| Auth refresh warning suppression | `compat(auth)` | `temporary-shim` | none yet | Upstream no longer warns on multi-provider model state after auth refresh. | `cargo check --workspace` |
| dev_cargo clang fallback | `distro(dev)` | `local-only` | none | Keep while local development environments may lack clang. | `scripts/dev_cargo.sh --help` |
| Nix dependency-cache stability (git stamp out of `buildDepsOnly`) | `distro(nix)` | `permanent-downstream` | none | Keep; this is a packaging correctness property, not an upstream concern. Watch that future `package.nix` edits never move `JCODE_BUILD_GIT_*` back into `commonArgs`. | gitHash A vs B must yield identical `cargoArtifacts.drvPath` (see commit `02bcc628`) |

Statuses:

- `local-only`
- `temporary-shim`
- `planned-upstream-pr`
- `submitted-upstream-pr`
- `waiting-upstream-release`
- `permanent-downstream`
- `retire-candidate`
- `retired`
