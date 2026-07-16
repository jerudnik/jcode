## Verdict: FAIL

Swarm report submitted. I performed a read-only adversarial review only. I did not modify files, run builds/tests, use network, reload, or read the existing Opus verdict.

## Ranked findings

### Critical 1: Incompatible advertised `Subscribe` does not fail before session/PID/server-state mutation

The incompatible verdict/refusal happens before `handle_subscribe`, but **not before observable session and PID mutation**.

Evidence:

- `handle_client` derives the initial Subscribe working dir, then creates an `Agent`, session id, client connection, shutdown controls, session map entry, and runtime memory event before the Subscribe verdict is evaluated:
  - `crates/jcode-app-core/src/server/client_lifecycle.rs:428-542`
- `Agent::new_with_initial_working_dir` creates a `Session` and calls `mark_active()`:
  - `crates/jcode-app-core/src/agent.rs:409-421`
- `Session::mark_active()` records the current process PID:
  - `crates/jcode-base/src/session.rs:1110-1116`
- `register_active_pid` writes the active PID marker:
  - `crates/jcode-storage/src/active_pids.rs:58-67`
- The incompatible advertised Subscribe verdict/refusal is only evaluated later:
  - `crates/jcode-app-core/src/server/client_lifecycle.rs:1338-1381`

The focused test only checks that the **advertised client PID** was not registered, not that no provisional server/session active PID marker or session/server state was created:

- `crates/jcode-app-core/src/tool/communicate_tests/end_to_end.rs:768-778`

This violates the requested invariant: “incompatible advertised subscribe fails before session/member/PID/tool mutation.”

### Critical 2: Advertised runtime identity does not deterministically distinguish dirty builds from the same commit

`SourceState` can distinguish dirty same-commit states, but the runtime identity advertised by TUI and emitted by the server handshake uses `current_runtime_identity_projection`, which drops the fingerprint and dirty flag.

Evidence:

- `current_runtime_identity_projection` sets:
  - `version_label: jcode_build_meta::VERSION`
  - `source_fingerprint: None`
  - `source_dirty: None`
  - `source_hash: Some(jcode_build_meta::GIT_HASH)`
  - `crates/jcode-build-support/src/lib.rs:47-63`
- Build meta dirty version is only `v...-dev (hash, dirty)`, not `hash-dirty-fingerprint`:
  - `crates/jcode-build-meta/build.rs:113-124`
- TUI advertises this lossy current projection:
  - `crates/jcode-tui/src/tui/backend.rs:326-342`
- The server handshake also emits current projection through `server_runtime_identity_projection()`:
  - `crates/jcode-app-core/src/server/handshake.rs:24-38`
  - `crates/jcode-app-core/src/server/handshake.rs:85-92`

Positive counter-evidence exists for `SourceState` projection:

- Dirty fingerprint and `version_label = <hash>-dirty-<fingerprint>` are computed in:
  - `crates/jcode-build-support/src/source_state.rs:91-137`
- `SourceState::runtime_identity_projection` includes fingerprint and dirty:
  - `crates/jcode-selfdev-types/src/lib.rs:118-130`

But the advertising/runtime handshake path does not use that deterministic projection. So the invariant is only partially satisfied.

### Important 1: Documentation overclaims fail-before-mutation coverage

The R03A ledger says the server fails closed before subscribed/session/member/PID/tool mutation:

- `docs/fork/recovery/seams/R03A-wire-compatibility/ledger.md:146-150`

That is inaccurate given the pre-verdict Agent/session/client-connection/active-PID mutations above.

## Positive evidence

- R03A compatibility remains separate from R01 runtime identity. Compatibility uses protocol/build hash only:
  - `crates/jcode-protocol/src/wire.rs:66-112`
  - `crates/jcode-app-core/src/server/handshake.rs:24-38`
- TUI consumes verdicts and refuses or reexecs rather than ignoring them:
  - `crates/jcode-tui/src/tui/app/remote.rs:775-797`
  - `crates/jcode-tui/src/tui/app/handshake.rs:53-159`
- ACP deliberately omits advertisement:
  - `src/cli/acp.rs:641-655`
- Generic `server::Client` deliberately omits advertisement because it does not synchronously consume verdicts:
  - `crates/jcode-app-core/src/server/client_api.rs:84-103`
- Reload evidence preserves selected runtime identity through reload signal/state transitions:
  - `crates/jcode-app-core/src/tool/selfdev/reload.rs:305-338`
  - `crates/jcode-app-core/src/server/reload_state.rs:82-118`
  - `crates/jcode-app-core/src/server/reload.rs:96-110`
  - `crates/jcode-app-core/src/server/reload.rs:176-206`

## Commands used

Read-only commands only:

- `git status --short`
- `git log --oneline --decorate --no-abbrev-commit e7e47e42c..HEAD`
- `git diff --stat e7e47e42c..HEAD`
- `git diff --name-status e7e47e42c..HEAD`
- `git show --stat` for `c759e2504`, `28a63f9f4`, `e95d3fdfa`
- `git diff --find-renames --unified=80 ...`
- `rg`, `nl -ba`, `sed`, `tail`, `wc`

Final `git status --short` produced no output.

## Confidence

High. The fail-before-mutation violation is directly visible in control flow and line-cited before the verdict block.

## Open questions

- Should “fail before PID mutation” mean no advertised client PID mutation only, or no PID/session mutation at all? The user’s wording says session/member/PID/tool mutation, so I treated the stricter observable invariant as intended.
- Is the lossy `current_runtime_identity_projection` acceptable for ambient/release binaries if selfdev reload uses exact `SourceState` projection? The requested invariant did not state that exception, so I counted it as failing deterministic dirty-build distinction for advertising runtime identity.

## What I did not check

- I did not run builds, tests, or live daemon/TUI scenarios by instruction.
- I did not use network, reload, or activation.
- I did not read the existing Opus verdict.
- I did not inspect every unrelated legacy client path beyond all found `Request::Subscribe` constructors.