# R01 BLOCKING-1 fix evidence

Date: 2026-07-19

## Scope

Fixed R01 review blocker `BLOCKING-1` within the owned server and evidence paths. No terminal or TUI source files were modified.

## Reload force propagation

Commit: `923bba4aa` (`fix(server): preserve reload force through exec`)

The reload request's `force` value now follows the request through the reload channel and into final target resolution:

1. `client_session::handle_reload` sends the actual request `force` value with the reload signal.
2. Production `ReloadSignal` carries `force` and exposes it through `ReloadSignal::force()`.
3. `reload::await_reload_signal` records the value in reload tracing and passes it to `reload_exec_target`.
4. `reload_exec_target` calls `resolve_reload_target(is_selfdev_session, force)` instead of hardcoding `true`.

This makes preflight and exec use the same refusal inputs. A non-forced request that passed preflight because a strictly newer approved exec candidate exists cannot be reinterpreted as a forced stale-shared-server refusal after sessions have been drained. It resolves the newer exec candidate instead of returning `None` and entering `reload_exec_failed`/exit 42.

The existing forced stale-shared-server refusal remains force-gated and unchanged in behavior.

Regression coverage:

- `server::reload_state::non_forced_reload_signal_retains_request_force`
- `server::util::target_resolution_tests::non_forced_stale_shared_server_with_newer_candidate_resolves_exec_target`
- Existing `server::util::target_resolution_tests::target_resolution_refuses_forced_stale_shared_server_when_newer_dev_exists`
- Existing reload signal receive tests

## Bounded session-alias garbage collection

Commit: `293384c53` (`fix(server): collect stale session aliases`)

The startup recovery GC now sweeps both stores:

- `reload-recovery/` retains its existing delivered/stale/corrupt record behavior.
- `session-aliases/` removes JSON records whose file mtime is at least `PENDING_RECORD_MAX_AGE` (7 days), while retaining fresh aliases.

Alias removal also cleans the matching backup path through the existing `remove_record_files` helper.

Regression coverage:

- `server::reload_recovery::tests::garbage_collection_sweeps_stale_session_aliases`

## Focused validation

All focused regressions passed:

```text
non_forced_reload_signal_retains_request_force ... ok
non_forced_stale_shared_server_with_newer_candidate_resolves_exec_target ... ok
target_resolution_refuses_forced_stale_shared_server_when_newer_dev_exists ... ok
garbage_collection_sweeps_stale_session_aliases ... ok
receive_reload_signal_* ... 2 passed
```

## Required acceptance validation

### Build support suite

Command:

```sh
scripts/dev_cargo.sh test -p jcode-build-support
```

Result: **PASS**, 50 passed, 0 failed.

### App core suite

Command:

```sh
scripts/dev_cargo.sh test -p jcode-app-core
```

Result: **PASS**, 1136 passed, 0 failed, 23 ignored.

### Selfdev-profile jcode build

Command:

```sh
scripts/dev_cargo.sh build --profile selfdev -p jcode --bin jcode
```

Result: **PASS**, finished the `selfdev` profile successfully.

The test and build logs contained no actionable Rust compiler errors or warnings.
