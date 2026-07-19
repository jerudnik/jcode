# D021 implementation evidence

## Outcome

Implemented the advertised Claude Agent/Task compatibility bridge as a registered `subagent` tool. The tool now launches a forked worker through the same shared worker helper used by `run_swarm_task`, captures `run_once_capture` output, and returns it to the caller. The existing `/subagent` handler remains unchanged and now succeeds because its registry lookup resolves to the registered bridge.

## Implementation

- Added `crates/jcode-app-core/src/tool/subagent.rs` with the advertised input surface: `description`, `prompt`, optional `subagent_type`, `run_in_background`, and `model`. The standard optional `intent` schema property is also present for app-core tool consistency.
- Extracted the worker-session core into `run_subagent_worker` in the tool module. It creates the child session, inherits the parent provider/auth route and working directory, applies an optional model override, filters worker tools, constructs the child `Agent`, and calls `run_once_capture`.
- Changed `server/swarm.rs::run_swarm_task` to delegate to that helper instead of duplicating child-session and worker construction.
- Registered `subagent` as a provider-bound per-session tool in `Registry::new`.
- Added Claude OAuth identity-name resolution at the app-core registry seam. The app-core test hardcodes the nine advertised names with a source comment because app-core cannot depend on `jcode-provider-anthropic` without reversing the dependency direction.
- Kept the recursive worker blocklist intact: `subagent`, `task`, `todo`, `todowrite`, and `todoread` are removed before child-agent construction.
- `run_in_background=true` is intentionally synchronous with an explicit note, as permitted by D021, rather than introducing new background infrastructure.

## Tests

Added or inverted tests in `crates/jcode-app-core/src/tool/tests.rs`:

1. `subagent_tool_is_registered` verifies the bridge is exposed.
2. `subagent_tool_returns_captured_worker_output` runs the tool against a deterministic mock provider and asserts the captured output is returned.
3. The same execution test records the child worker's advertised tools and asserts `subagent` is absent, protecting the no-recursive-fork blocklist.
4. `claude_identity_tool_names_resolve_to_registered_tools` verifies every name in `claude_code_identity_tools` resolves through `Registry::resolve_tool_name` to a registered tool.

## Slash-command verification

`crates/jcode-app-core/src/server/client_actions.rs::handle_run_subagent` still executes `tool_name = "subagent"` through `registry.execute`. No handler change was required after registration.

## Validation

Executed successfully at commit `6c633b785`:

```text
scripts/dev_cargo.sh test -p jcode-app-core
1138 passed; 0 failed; 23 ignored

scripts/dev_cargo.sh build --profile selfdev -p jcode --bin jcode
Finished selfdev profile successfully
```

Focused bridge and registry tests also passed before the full suite.

A repository-wide `cargo fmt --check` reports pre-existing formatting drift in unrelated R01-owned server files. No unrelated files were reformatted or committed. `git diff --check` passes for the resulting worktree.

## Commits

- `16646d9f4` `fix(tools): bridge subagent through swarm worker path`
- `6c633b785` `fix(tools): include intent in subagent schema`
