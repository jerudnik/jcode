use anyhow::Result;
pub use jcode_terminal_launch::{
    SpawnAttempt, TerminalCommand, detected_resume_terminal, resume_terminal_candidates, sh_escape,
    shell_command,
};
use std::path::Path;

/// When this env var is set to a non-empty, non-`0` value, every call to
/// [`spawn_command_in_new_terminal`] reports "no terminal launched"
/// (`Ok(false)`) without actually spawning anything. The standard test
/// harness ([`crate::storage::TestJcodeHome::acquire`]) sets this to `1`
/// so that test runs cannot accidentally launch new Ghostty / iTerm /
/// Terminal.app windows for `/transfer`, `/resume`, self-dev handoff,
/// or any other TUI flow that would normally spawn a sibling jcode
/// process in a new terminal.
///
/// Tests that genuinely need to verify the spawn path (for example the
/// `spawn_*_in_new_terminal_uses_handterm_exec_mode` tests) should clear
/// this var within their scope using a scoped `EnvVarGuard` so the spawn
/// loop is exercised against a fake binary on `PATH`.
pub const DISABLE_ENV_VAR: &str = "JCODE_DISABLE_TERMINAL_SPAWN";

/// Opt-in escape hatch for tests that need to exercise the real terminal
/// spawn path. When the `jcode` lib is built with `#[cfg(test)]`, terminal
/// spawning is **default-deny**: set `JCODE_TEST_ALLOW_TERMINAL_SPAWN=1`
/// (and only then) to invoke the real spawn loop. This makes it impossible
/// for any test path to pop a real Ghostty / iTerm / Terminal.app window,
/// regardless of which helper installed the `TestJcodeHome` guard.
#[cfg(test)]
pub const TEST_ALLOW_ENV_VAR: &str = "JCODE_TEST_ALLOW_TERMINAL_SPAWN";

fn is_disabled() -> bool {
    #[cfg(test)]
    {
        // Test builds: default-deny. The opt-in var, when set,
        // ALSO overrides the production `JCODE_DISABLE_TERMINAL_SPAWN`
        // var (which the test harness installs at `lock_test_env()`
        // init time), so the spawn-loop tests can actually exercise
        // the real spawn path.
        let allow = std::env::var_os(TEST_ALLOW_ENV_VAR)
            .map(|v| !v.is_empty() && v != "0")
            .unwrap_or(false);
        if allow {
            return false;
        }
        return true;
    }
    #[allow(unreachable_code)]
    std::env::var_os(DISABLE_ENV_VAR)
        .map(|v| !v.is_empty() && v != "0")
        .unwrap_or(false)
}

pub fn spawn_command_in_new_terminal(command: &TerminalCommand, cwd: &Path) -> Result<bool> {
    if is_disabled() {
        return Ok(false);
    }
    jcode_terminal_launch::spawn_command_in_new_terminal_with(command, cwd, |cmd| {
        crate::platform::spawn_detached(cmd).map(|_| ())
    })
}
