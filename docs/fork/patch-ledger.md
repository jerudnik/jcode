# Downstream patch ledger

Track downstream patches that may need to be upstreamed, watched, or retired.
Permanent fork behavior can stay here too, but every temporary shim must have a
retirement condition and validation command.

| Patch | Class | Status | Upstream ref | Retire condition | Validation |
|---|---|---|---|---|---|
| OpenAI-compatible schema format stripping | `compat(openai)` | `temporary-shim` | none yet | Upstream accepts MCP/fetch schemas with unsupported JSON Schema `format` keywords removed or handles them before OpenAI submission. | `cargo test -p jcode-provider-core openai_schema` |
| APM MCP and skill surface loading | `feature(agent-tools)` | `permanent-downstream` | none | Keep unless upstream adopts equivalent APM/tool-surface loading. | `cargo check --workspace` |
| Herdr lifecycle and pin reporting | `feature(herdr)` | `permanent-downstream` | none | Keep while Herdr is a 4nix-local harness integration. | `cargo check --workspace` |
| ACP session config options | `feature(acp)` | `planned-upstream-pr` | none yet | Retire or reduce once upstream exposes equivalent ACP session config controls. | `cargo check --workspace` |
| Auth refresh warning suppression | `compat(auth)` | `temporary-shim` | none yet | Upstream no longer warns on multi-provider model state after auth refresh. | `cargo check --workspace` |
| dev_cargo clang fallback | `distro(dev)` | `local-only` | none | Keep while local development environments may lack clang. | `scripts/dev_cargo.sh --help` |

Statuses:

- `local-only`
- `temporary-shim`
- `planned-upstream-pr`
- `submitted-upstream-pr`
- `waiting-upstream-release`
- `permanent-downstream`
- `retire-candidate`
- `retired`
