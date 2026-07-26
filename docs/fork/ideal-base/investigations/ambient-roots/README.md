# Ambient-roots audit: every path the process resolves from the environment

Recorded 2026-07-26 during F20c close-out, after five consecutive test-suite
flakes each root-caused to the same defect class: **process-global or ambient
state read outside the isolated storage helpers**. This file is the complete
inventory of that surface so the follow-up node (F29) starts from facts, not
another rediscovery.

## Why this exists

`crates/jcode-storage/src/lib.rs` now isolates three ambient roots under test
(`b43bdd899`): `jcode_dir()` (`~/.jcode` / `JCODE_HOME`), `app_config_dir()`
(platform config dir), and `user_home_path()` (`~`). Every consumer that calls
`dirs::*` directly bypasses that isolation. Concretely observed failures from
this class:

- model-picker route order flipped by the developer's real
  `model_picker_usage.json` (read leak via direct `app_config_dir` fallback,
  fixed at the source);
- five TUI tests red locally, green in CI, purely because CI's home is empty.

The write-side gate (`scripts/check_real_home_isolation.sh`) watches for test
*writes* under the real roots. Read leaks are invisible to it by design; the
only defense against reads is routing resolution through the isolated helpers.

## Inventory: 41 direct `dirs::` call sites in crates (+8 in `src/cli`)

Counted with:

```sh
grep -rn 'dirs::home_dir()\|dirs::config_dir()\|dirs::data_dir()\|dirs::cache_dir()\|dirs::data_local_dir()' \
  crates/*/src --include='*.rs' | grep -v 'jcode-storage/src'
```

### Class A: `JCODE_HOME` bypass defects (correctness bugs, not just hygiene)

These write or read under `~/.jcode` while ignoring `JCODE_HOME` and the
storage helpers, so a redirected home (tests, alternate profiles, harness
sandboxes) still leaks into the real one:

| Site | Path resolved | Notes |
| --- | --- | --- |
| `jcode-base/src/memory_log.rs:52` | `~/.jcode/logs` | ignores `JCODE_HOME` entirely; memory log writes land in the real home under test |
| `jcode-provider-copilot-runtime/src/lib.rs:206` | `~/.jcode/machine_id` | ignores `JCODE_HOME`; reads and creates in the real home |
| `jcode-tui/src/tui/ui_changelog.rs:167` | `~/.jcode/last_seen_changelog` | ignores `JCODE_HOME` |
| `jcode-provider-openrouter/src/lib.rs:349-359,556-560` | `~/.jcode/cache/*_models.json` | duplicates the `JCODE_HOME` env check by hand instead of calling `jcode_dir()`; env-var read is ambient at call time (config-env-lease class) |
| `jcode-base/src/mobile_server.rs:29` | `~/.jcode` | full reimplementation of `jcode_dir()`; drifts from the canonical resolver |
| `jcode-base/src/browser.rs:36-40` | `~/.jcode` | fallback arm only (primary is `storage::jcode_dir()`); fallback silently un-isolates on error |

### Class B: platform config/cache dirs bypassing `app_config_dir()` isolation

Same shape as the model-picker bug. Reads/writes under
`~/Library/Application Support/jcode` (or `~/.cache/jcode`) with no test
redirect:

| Site | Path resolved |
| --- | --- |
| `jcode-tui-visual-debug/src/lib.rs:336` | `<config>/jcode/visual-debug.txt` |
| `jcode-tui/src/tui/test_harness.rs:467` | `<config>/jcode/...` (in the test harness itself) |
| `jcode-tui/src/tui/app/debug.rs:730` | `<config>/jcode/recordings` |
| `jcode-base/src/auth/cursor.rs:396` | `<config>/cursor/auth.json` (has a hand-rolled `JCODE_HOME` special case above it; the fallback is still ambient) |
| `jcode-tui-mermaid/src/mermaid_cache_render.rs:51` | `<cache>/jcode/mermaid` |
| `jcode-tui-markdown/src/markdown_latex_image.rs:295` | `<cache>/jcode/latex` |

### Class C: user-home reads for external discovery and `~` expansion

Legitimately need the real home in production, but should route through
`user_home_path()` so tests see the harness home (a test that exercises skhd
discovery or `~` expansion currently reads the developer's real dotfiles):

- `jcode-setup-hints/src/lib.rs:345,369,1271` (last-dir file, LaunchAgent path, `~/.config` fallback)
- `jcode-setup-hints/src/keymap/external.rs:164,231,302` (omniwm/aerospace/skhd config readers)
- `jcode-setup-hints/src/macos_launcher.rs:102,107` (`~/Applications/Jcode.app`)
- `jcode-setup-hints/src/launch_hotkeys.rs:216` (`$HOME` target resolution)
- `jcode-terminal-launch/src/lib.rs:191,471` (app discovery, `~/` expansion)
- `jcode-app-core/src/tool/open.rs:239,244` (`~` expansion)
- `jcode-tui/src/tui/remote_diff.rs:92` (`~/` expansion)
- `jcode-tui/src/tui/ui_header.rs:494` (home abbreviation for display)
- `jcode-tui/src/video_export.rs:43,67` (`~/.cargo/bin` lookup, kitty.conf read) — file is owned by F28; F29 depends on F28 to serialize
- `jcode-base/src/browser.rs:675,680,691` (native-messaging-hosts dirs)
- `jcode-base/src/hooks.rs:112`, `jcode-base/src/surface_workspace.rs:224`, `jcode-base/src/config/config_file.rs:834`
- `jcode-provider-openai-runtime/src/openai_tests.rs:131` (test reads real `~/.codex` fixtures)
- `src/cli/`: 8 sites across `commands.rs`, `commands/doctor.rs`, `commands/menubar.rs`, `login.rs`, `auth_test/types.rs`

### Dependency feasibility

`jcode-tui-mermaid`, `jcode-tui-markdown`, `jcode-tui-visual-debug`,
`jcode-terminal-launch`, and `jcode-provider-openrouter` currently have **no
dependency on `jcode-storage`/`jcode-base`**. Routing them through the helpers
means either adding a small dep edge or (for leaf crates that only need one
root) accepting an injected path parameter from the caller. Decide per crate in
F29; do not force a dependency where a parameter is cleaner.

## The wider ambient shape (beyond filesystem roots)

Filed for completeness; each is its own defect class with a prior incident:

1. **Process-global mutable statics in the TUI** — 27 files in
   `crates/jcode-tui/src` hold `static Mutex/RwLock/OnceLock/LazyLock` state.
   Flicker history was one (fixed per-thread under test, `b7bdef31b`); F28
   owns the render-lock discipline for the rest.
2. **Ambient env reads at call time** — e.g. the openrouter cache-namespace
   and `JCODE_HOME` reads above. The fixed pattern (`b43bdd899`) is: pure
   `resolve_*` functions take ambient inputs as arguments; the thin public
   wrapper reads the environment exactly once at the boundary.
3. **`env::set_var` in non-test code** — present in ~10 `jcode-app-core`
   server files (reload/lifecycle/relay paths). Process-global env mutation
   in a multi-threaded server is the same class that forced the config-env
   lease. Not yet audited; noted here so it is not lost.
4. **Process-global session-name allocator cursor** — fixed in `a5bda6356`
   (one `fetch_add` per allocation, local scan), kept as the reference
   example of the concurrency shape.

## Proposed gate for F29 (grep gate, F20c-removal-gate style)

A script that fails if any crate outside `jcode-storage` matches
`dirs::(home|config|data|cache|data_local)_dir\(` — with an explicit,
documented allowlist that must shrink monotonically (baseline count checked
in, failure names each offender file:line). Complements, not replaces, the
write-watching `check_real_home_isolation.sh` and the storage unit tests that
pin read-side redirection.
