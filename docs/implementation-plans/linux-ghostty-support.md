# Implementation Prompt: Linux Ghostty Terminal Spawn Support

We are in `/home/john/infrastructure/jcode` in a self-dev context. Please add native Linux Ghostty support to Jcode's terminal auto-spawn logic, with tests and documentation.

## Goal / intent

Jcode currently supports Ghostty for macOS terminal spawning, but Linux Ghostty is not included in the non-macOS Unix terminal candidate list and is not handled in the non-macOS spawn command matcher. On my Linux system I use Ghostty from Home Manager, so `/selfdev` created a self-dev session but could not auto-open a supported terminal.

Desired behavior:

- On Linux/non-macOS Unix, Jcode should detect Ghostty when running inside Ghostty.
- Jcode should include `ghostty` in Unix terminal spawn candidates.
- Jcode should spawn a new Linux Ghostty window using Ghostty's CLI `-e <program> <args...>`.
- Existing macOS Ghostty behavior must continue unchanged.
- Existing `JCODE_SPAWN_HOOK` behavior must remain preferred over built-in terminal detection.
- Add deterministic unit tests. Do not rely on actually opening a GUI terminal in tests.

## Context and citations

Read and follow the repo contracts:

- Read root `AGENTS.md` before editing. DOX says to read applicable AGENTS files and not rely on memory: `AGENTS.md:21-31`.
- Current branch rails: check current branch and remotes before editing. `main` is where stable custom fork behavior and app features belong: `AGENTS.md:114-136`.
- Build/test workflow: use fast iteration, rebuild when done, and in self-dev sessions prefer `selfdev build` or `selfdev build-reload` for TUI changes: `AGENTS.md:142-173`.
- APM-generated docs should not be hand-edited: `AGENTS.md:67-82` and `AGENTS.md:181-190`.
- The docs DOX says fork/integration docs should stay close to implementation and use function-oriented validation language: `docs/AGENTS.md:21-37`.

Relevant code:

- `/selfdev` creates a session and reports "could not auto-open" when `launch.launched` is false: `crates/jcode-tui/src/tui/app/commands.rs:2166-2192`.
- `enter_selfdev_session` creates the session, marks it `self-dev`, saves it, then calls `session_launch::spawn_selfdev_in_new_terminal`: `crates/jcode-app-core/src/tool/selfdev/launch.rs:3-73`.
- The spawn hook contract exists and should stay first: `crates/jcode-base/src/terminal_launch.rs:22-35`.
- Built-in Unix terminal candidates currently include `handterm`, `kitty`, `wezterm`, `alacritty`, `gnome-terminal`, `konsole`, `xterm`, and `foot`, but not `ghostty`: `crates/jcode-terminal-launch/src/lib.rs:225-260`.
- The non-Windows/non-macOS detection branch currently returns `None` after checking Kitty, WezTerm, and Alacritty env vars: `crates/jcode-terminal-launch/src/lib.rs:162-209`.
- `build_spawn_command` has a `ghostty` case only behind `#[cfg(target_os = "macos")]`: `crates/jcode-terminal-launch/src/lib.rs:474-493`.
- Other terminals use direct `Command::new(term)` forms with `-e` or equivalent: `crates/jcode-terminal-launch/src/lib.rs:494-519`.
- Spawn metadata is exported to launched terminal processes after command construction: `crates/jcode-terminal-launch/src/lib.rs:552-557`.

Ghostty local CLI contract to verify before implementing:

- Run:
  ```sh
  ghostty +help
  ```
- Expected docs say `ghostty -e <command>` runs a specific command inside the terminal emulator. Use the local CLI output as validation, not memory.

## Implementation plan

1. Preflight:
   ```sh
   git branch --show-current
   git remote -v
   ```
   Confirm we are on `main` or an appropriate feature branch.
   Read `AGENTS.md`, `docs/AGENTS.md`, and any closer AGENTS files for touched paths.

2. Add Linux/non-macOS Ghostty detection:
   - In `crates/jcode-terminal-launch/src/lib.rs`, update `detected_resume_terminal()` for Unix.
   - Detect Ghostty on non-macOS Unix via reliable env vars present in Ghostty:
     - `TERM_PROGRAM=ghostty`
     - `GHOSTTY_RESOURCES_DIR`
     - `GHOSTTY_BIN_DIR`
   - Be careful not to make macOS behavior regress. It already has Ghostty-specific logic.

3. Add Ghostty to non-macOS Unix candidates:
   - Insert `"ghostty"` in the non-macOS Unix fallback list, preferably near the other modern terminals:
     ```rust
     "ghostty",
     "handterm",
     "kitty",
     // ...
     ```
     or after `handterm` if preserving handterm priority matters.
   - Preserve user override priority from `JCODE_TERMINAL`.

4. Add a Linux-compatible `build_spawn_command` arm:
   - Keep the existing macOS `ghostty` arm using `open -na Ghostty`.
   - Add a separate non-macOS Unix arm:
     ```rust
     #[cfg(all(unix, not(target_os = "macos")))]
     "ghostty" => {
         cmd.args(["-e"]).arg(&command.program).args(&command.args);
     }
     ```
   - Confirm metadata env export remains applied after the match, as it is today.

5. Add tests in `crates/jcode-terminal-launch/src/lib.rs` test module:
   - Test non-macOS Unix detection recognizes `TERM_PROGRAM=ghostty`.
   - Test non-macOS Unix detection recognizes `GHOSTTY_RESOURCES_DIR` or `GHOSTTY_BIN_DIR`.
   - Test non-macOS Unix candidates include `ghostty`.
   - Test `build_spawn_command("ghostty", ...)` builds `ghostty -e <program> <args...>` on non-macOS Unix.
   - Keep tests guarded with appropriate `#[cfg(all(unix, not(target_os = "macos")))]`.
   - Use the existing `ENV_LOCK` pattern to avoid env-var races.

6. Documentation:
   - If terminal support docs or default config comments list supported terminals, update them.
   - If no durable docs need changes, say so explicitly in the final report.
   - Do not hand-edit generated APM outputs.

## Deterministic validation

Run targeted checks:

```sh
cargo test -p jcode-terminal-launch ghostty
cargo test -p jcode-base spawn_hook
cargo check -p jcode --bin jcode
```

If package names differ, inspect `Cargo.toml` and use the exact crate/package names.

Then run the self-dev build path:

```sh
selfdev build target=tui
```

or, if ready to reload:

```sh
selfdev build-reload target=tui
```

Do not use slow release/LTO builds.

## Functionality / operator testing

After tests/build pass:

1. Confirm local Ghostty is available:
   ```sh
   command -v ghostty
   ghostty +help | grep -F -- '-e <command>'
   ```

2. Confirm the current environment is detected by code-level tests. If a small unit/integration helper exists, use it. Otherwise rely on the new unit tests.

3. Manual Jcode operator test:
   - Start a normal Jcode session in Ghostty without `JCODE_SPAWN_HOOK`.
   - Run `/selfdev`.
   - Expected:
     - Jcode creates a self-dev session.
     - A new Ghostty window opens.
     - The new window runs:
       ```sh
       <jcode-binary> --fresh-spawn --resume <session-id> self-dev
       ```
     - The previous "could not auto-open a supported terminal" message no longer appears.

4. Regression test:
   - Set a fake spawn hook, e.g. a script that records argv/env and exits 0.
   - Confirm Jcode still uses `JCODE_SPAWN_HOOK` before built-in Ghostty detection.
   - This behavior is required by `crates/jcode-base/src/terminal_launch.rs:22-35`.

## Definition of done

- Linux/non-macOS Unix Ghostty is detected when running inside Ghostty.
- Linux/non-macOS Unix terminal candidates include Ghostty.
- Built-in spawn can launch `ghostty -e <jcode> <args...>`.
- macOS Ghostty behavior remains unchanged.
- Spawn-hook precedence remains unchanged.
- Deterministic tests cover detection, candidate ordering/inclusion, and Ghostty command construction.
- Targeted cargo tests and `cargo check -p jcode --bin jcode` pass.
- A TUI self-dev build succeeds via `selfdev build target=tui` or `selfdev build-reload target=tui`.
- Manual `/selfdev` operator test in Ghostty succeeds without needing `JCODE_SPAWN_HOOK`.
- DOX/APM docs are updated only if durable behavior docs changed; generated docs are not hand-edited.
- Commit only the intended changes with a focused commit message, and push if the repo convention requires it.
