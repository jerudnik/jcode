# R01 implementation evidence

Date: 2026-07-19

## Scope

Implemented railway node R01 within the owned paths. No R03 terminal/TUI files were modified by this work.

## Repairs

### 1. Atomic version publishing

`install_binary_at_version` now:

1. Creates the version directory and a unique temporary binary in that same directory.
2. Copies bytes from the source instead of creating a hard link.
3. Sets executable permissions, calls `sync_all`, and rejects a zero-byte staged file.
4. Smoke-tests the staged path before it is visible as `versions/<version>/jcode`.
5. Renames the staged file atomically into place and syncs the directory best-effort.
6. Removes the staged file and empty version directory on every pre-publish failure.

Regression coverage:

- `atomic_publish_tests::concurrent_source_truncation_between_stage_and_rename_preserves_published_copy`
- `atomic_publish_tests::failed_smoke_test_leaves_no_version_entry`

### 2. Explicit reload target selection

Added a structured shared reload target resolver used by reload request preflight and daemon exec selection. It records:

- chosen exec path and resolved payload
- preferred, alternate, shared-server, stable, dev, and current-exe candidates
- candidate mtimes
- duplicate/missing/older rejection reasons
- refusal reason

Before exec it emits a machine-readable log line prefixed with `RELOAD_TARGET_DECISION`.

A forced server reload refuses when the approved shared-server target is stale while a strictly newer dev candidate exists, instead of silently re-execing the stale payload. The refusal names both paths, payloads, and mtimes and explains that debug/selfdev publishing must explicitly promote the newer binary first.

Non-forced reload output now says that no strictly newer approved reload target was found by mtime and includes the compared channel candidates and mtimes. It no longer claims that the daemon is running the globally “newest binary.”

Regression coverage:

- `target_resolution_tests::target_resolution_refuses_forced_stale_shared_server_when_newer_dev_exists`
- `target_resolution_tests::target_resolution_allows_forced_shared_server_when_it_is_freshest`

### 3. Durable resume identity binding

Resume now persists a durable source/client-session to resumed-agent-session alias under the jcode state directory. Alias resolution follows bounded chains and detects cycles.

`resolve_debug_session` resolves explicit or fallback reconnect aliases to the live resumed agent. If the alias target is no longer active, the error names the stale requested ID, the resolved target, and currently active sessions.

Reload recovery intent eligibility now includes every non-headless swarm member with at least one live attached connection, even when its status is not `running`. Candidate trace records include attached connection counts.

Regression coverage:

- `reload_recovery::tests::session_alias_roundtrips_and_follows_resume_chain`
- `debug_command_exec::tests::resolve_debug_session_resolves_reconnect_alias_to_live_agent`
- `debug_command_exec::tests::resolve_debug_session_unknown_id_error_names_alias_and_active_sessions`
- `reload::r01_tests::recovery_intents_include_attached_non_headless_sessions`

## Acceptance results

### Build support suite

Command:

```sh
scripts/dev_cargo.sh test -p jcode-build-support
```

Result: **PASS**, 50 passed, 0 failed.

This includes both atomic-publish gates:

- source truncation after staging cannot corrupt the published artifact
- failed smoke leaves no `versions/<version>` entry

### Focused app-core gates

Commands:

```sh
scripts/dev_cargo.sh test -p jcode-app-core resolve_debug_session -- --nocapture
scripts/dev_cargo.sh test -p jcode-app-core recovery_intents_include_attached_non_headless_sessions -- --nocapture
scripts/dev_cargo.sh test -p jcode-app-core target_resolution_refuses_forced_stale_shared_server_when_newer_dev_exists -- --nocapture
scripts/dev_cargo.sh test -p jcode-app-core target_resolution_allows_forced_shared_server_when_it_is_freshest -- --nocapture
scripts/dev_cargo.sh test -p jcode-app-core session_alias_roundtrips_and_follows_resume_chain -- --nocapture
```

Result: **PASS**. The first command matched six debug-resolution tests. Each remaining named regression passed.

### Full touched app-core suite

Command:

```sh
scripts/dev_cargo.sh test -p jcode-app-core
```

Result: **PASS**, 1133 passed, 0 failed, 23 ignored.

### Required selfdev build

Equivalent coordinated command:

```sh
scripts/dev_cargo.sh build --profile selfdev -p jcode --bin jcode
```

Executed through `selfdev build target=tui` on final commit `4ad36e517`.

Result: **PASS**.

## Commits

- `7387597e1 fix(build): publish version binaries atomically`
- `418c92935 fix(server): bind reconnect aliases across reload`
- `d7807e4ca fix(server): make reload target decisions explicit`
- `c5ab2c9d5 test(server): isolate reload resolution fixtures`
- `4ad36e517 chore(server): scope legacy reload helpers to tests`
