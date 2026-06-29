//! NS1 client-side handshake action: act on the server's compatibility verdict.
//!
//! When the daemon answers `Subscribe` with
//! [`HandshakeCompatibility::IncompatibleReconnect`], attaching to a
//! substantially-different server is unsafe (the research validates re-exec +
//! reconnect over in-place patching: Dioxus Subsecond cannot reload struct
//! layouts). This module decides, as a pure function, whether to attach,
//! re-exec into the matching launcher, or refuse, so the actual `exec()` is a
//! thin shell over tested logic. See
//! `docs/architecture/SELFDEV_NIX_DAEMON_DIVERGENCE.md` (NS1).

use std::path::{Path, PathBuf};

use jcode_protocol::HandshakeCompatibility;

/// Env marker set on the child before re-exec so a single mismatch can trigger
/// at most one re-exec. If we relaunch and the new binary still sees an
/// incompatible verdict, we must not loop: we attach (or refuse) instead.
pub(crate) const REEXEC_GUARD_ENV: &str = "JCODE_NS1_REEXECED";

/// What the client should do given the server's verdict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum HandshakeAction {
    /// Proceed normally: attach to the daemon as today.
    Attach,
    /// Re-exec into this launcher and reconnect.
    ReExec(PathBuf),
    /// Refuse to attach; show this message. Fail safe when no matching launcher
    /// can be resolved, rather than attaching to an incompatible daemon.
    Refuse(String),
}

/// Inputs to the pure decision, kept separate from process/env access so the
/// decision is fully unit-testable.
#[derive(Debug, Clone)]
pub(crate) struct HandshakeContext<'a> {
    pub compatibility: HandshakeCompatibility,
    /// The launcher to re-exec into, resolved by the caller via the existing
    /// reload/identity code (`preferred_reload_candidate`). `None` when no
    /// candidate could be resolved.
    pub reexec_target: Option<PathBuf>,
    /// The currently running executable, canonicalized to its payload. Used to
    /// avoid re-execing into the same binary (which would not fix anything).
    pub current_exe: Option<&'a Path>,
    /// Whether this process is already the product of an NS1 re-exec
    /// (`REEXEC_GUARD_ENV` was set in our environment).
    pub already_reexeced: bool,
    /// Human-readable server detail string, surfaced in the refuse/relaunch
    /// message.
    pub detail: &'a str,
}

/// Decide the client's action from a server handshake verdict. Pure.
pub(crate) fn decide_handshake_action(ctx: &HandshakeContext<'_>) -> HandshakeAction {
    // Compatible (and the legacy "no verdict" path, which never calls this) just
    // attaches.
    if ctx.compatibility.is_compatible() {
        return HandshakeAction::Attach;
    }

    // Incompatible, but we already re-execed once: do not loop. Attaching to a
    // mismatched daemon is the lesser evil versus an infinite relaunch cycle,
    // and the user has already been told once.
    if ctx.already_reexeced {
        return HandshakeAction::Attach;
    }

    match &ctx.reexec_target {
        Some(target) => {
            // Re-execing into the same binary we are already running cannot
            // resolve the mismatch (the daemon differs, not us). Attaching is
            // the honest outcome here; the divergence is on the server side and
            // a server reload is the fix, not a client relaunch.
            let target_payload = canonical_payload(target);
            let current_payload = ctx.current_exe.map(canonical_payload);
            if current_payload.as_deref() == Some(target_payload.as_path()) {
                return HandshakeAction::Attach;
            }
            HandshakeAction::ReExec(target.clone())
        }
        // Fail safe: no launcher resolved -> refuse rather than attach blindly.
        None => HandshakeAction::Refuse(format!(
            "Refusing to attach to an incompatible daemon and no matching launcher was found. {}\n\
             Fix: rebuild/reinstall a matching jcode, or run `jcode server reload`, then retry.",
            ctx.detail
        )),
    }
}

/// Canonicalize a path to the payload that actually runs, looking through
/// release wrapper scripts (reuses the existing identity logic). Falls back to
/// the input path when it cannot be resolved.
fn canonical_payload(path: &Path) -> PathBuf {
    crate::build::resolve_binary_payload(path)
}

/// Outcome of acting on a server handshake verdict.
pub(crate) enum HandshakeOutcome {
    /// Attach to the daemon as today (the common path).
    Attach,
    /// We are refusing to attach; show this message and quit.
    Refuse(String),
    // Re-exec never returns on success, so it has no variant here.
}

/// Act on the server's handshake verdict. Resolves the re-exec target with the
/// same code the reload path uses (`preferred_reload_candidate`), runs the pure
/// [`decide_handshake_action`], and performs the side effect: on `ReExec` it
/// prints what mismatched and `exec()`s into the matching launcher (never
/// returns on success); on `Refuse` it returns the message for the caller to
/// surface; on `Attach` it returns [`HandshakeOutcome::Attach`].
pub(crate) fn act_on_verdict(
    compatibility: HandshakeCompatibility,
    detail: &str,
    is_selfdev_session: bool,
) -> HandshakeOutcome {
    if compatibility.is_compatible() {
        return HandshakeOutcome::Attach;
    }

    let reexec_target =
        crate::build::preferred_reload_candidate(is_selfdev_session).map(|(path, _label)| path);
    let current_exe = std::env::current_exe().ok();
    let already_reexeced = std::env::var_os(REEXEC_GUARD_ENV).is_some();

    let ctx = HandshakeContext {
        compatibility,
        reexec_target: reexec_target.clone(),
        current_exe: current_exe.as_deref(),
        already_reexeced,
        detail,
    };

    match decide_handshake_action(&ctx) {
        HandshakeAction::Attach => HandshakeOutcome::Attach,
        HandshakeAction::Refuse(msg) => HandshakeOutcome::Refuse(msg),
        HandshakeAction::ReExec(target) => {
            // Tell the user exactly what mismatched and where we are going.
            eprintln!(
                "jcode: incompatible daemon detected. {detail}\n\
                 jcode: re-executing into matching launcher: {}",
                target.display()
            );
            crate::logging::warn(&format!(
                "NS1 handshake: re-execing into {} (detail: {detail})",
                target.display()
            ));

            let mut cmd = build_reexec_command(&target, std::env::args_os().skip(1));
            let err = crate::platform::replace_process(&mut cmd);
            // exec() only returns on failure. Fail safe: surface, do not attach.
            HandshakeOutcome::Refuse(format!(
                "Failed to re-exec into {}: {err}. {detail}",
                target.display()
            ))
        }
    }
}

/// Build the re-exec `Command` for `target`: forward the current invocation's
/// arguments and set the [`REEXEC_GUARD_ENV`] marker so a still-incompatible
/// verdict in the child cannot trigger another re-exec (loop guard). Pure
/// command construction, separated from the `exec()` side effect so the
/// targeting + guard wiring is unit-testable.
fn build_reexec_command<I, S>(target: &Path, forwarded_args: I) -> std::process::Command
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let mut cmd = std::process::Command::new(target);
    cmd.args(forwarded_args);
    cmd.env(REEXEC_GUARD_ENV, "1");
    cmd
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx<'a>(
        compatibility: HandshakeCompatibility,
        reexec_target: Option<PathBuf>,
        current_exe: Option<&'a Path>,
        already_reexeced: bool,
    ) -> HandshakeContext<'a> {
        HandshakeContext {
            compatibility,
            reexec_target,
            current_exe,
            already_reexeced,
            detail: "client abc != server def",
        }
    }

    #[test]
    fn compatible_always_attaches() {
        let action = decide_handshake_action(&ctx(
            HandshakeCompatibility::Compatible,
            None,
            None,
            false,
        ));
        assert_eq!(action, HandshakeAction::Attach);
    }

    #[test]
    fn incompatible_with_distinct_target_reexecs() {
        let target = PathBuf::from("/home/u/.jcode/builds/current/jcode");
        let current = PathBuf::from("/nix/store/abc-jcode/bin/jcode");
        let action = decide_handshake_action(&ctx(
            HandshakeCompatibility::IncompatibleReconnect,
            Some(target.clone()),
            Some(current.as_path()),
            false,
        ));
        assert_eq!(action, HandshakeAction::ReExec(target));
    }

    #[test]
    fn incompatible_without_target_refuses() {
        let action = decide_handshake_action(&ctx(
            HandshakeCompatibility::IncompatibleReconnect,
            None,
            None,
            false,
        ));
        match action {
            HandshakeAction::Refuse(msg) => {
                assert!(msg.contains("Refusing to attach"));
                assert!(msg.contains("client abc != server def"));
            }
            other => panic!("expected Refuse, got {other:?}"),
        }
    }

    #[test]
    fn incompatible_after_reexec_attaches_to_avoid_loop() {
        let target = PathBuf::from("/home/u/.jcode/builds/current/jcode");
        let action = decide_handshake_action(&ctx(
            HandshakeCompatibility::IncompatibleReconnect,
            Some(target),
            None,
            true, // already re-execed once
        ));
        assert_eq!(action, HandshakeAction::Attach);
    }

    #[test]
    fn incompatible_target_equals_current_attaches() {
        // Re-execing into the same binary would not fix a server-side mismatch.
        let same = PathBuf::from("/usr/local/bin/jcode-real");
        let action = decide_handshake_action(&ctx(
            HandshakeCompatibility::IncompatibleReconnect,
            Some(same.clone()),
            Some(same.as_path()),
            false,
        ));
        assert_eq!(action, HandshakeAction::Attach);
    }

    #[test]
    fn build_reexec_command_sets_guard_env_and_forwards_args() {
        let cmd = build_reexec_command(
            Path::new("/home/u/.jcode/builds/current/jcode"),
            ["serve", "--socket", "/tmp/x.sock"],
        );
        assert_eq!(
            cmd.get_program(),
            std::ffi::OsStr::new("/home/u/.jcode/builds/current/jcode")
        );
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy().into_owned()).collect();
        assert_eq!(args, vec!["serve", "--socket", "/tmp/x.sock"]);
        let guard = cmd
            .get_envs()
            .find(|(k, _)| *k == std::ffi::OsStr::new(REEXEC_GUARD_ENV))
            .and_then(|(_, v)| v)
            .map(|v| v.to_string_lossy().into_owned());
        assert_eq!(guard.as_deref(), Some("1"));
    }

    /// Process-level smoke: the re-exec command actually launches the target
    /// with the guard env and forwarded args set, proving the exec seam wires
    /// the right process (the real `exec()` itself is exercised by the server
    /// reload path). Unix-only because it shells out to `/bin/sh`.
    #[cfg(unix)]
    #[test]
    fn build_reexec_command_actually_launches_target_with_guard_and_args() {
        // Use `sh -c` as the "launcher": print the guard env and the first
        // forwarded arg so we can assert both crossed the process boundary.
        let script = format!("printf '%s:%s' \"${REEXEC_GUARD_ENV}\" \"$1\"");
        let mut cmd = build_reexec_command(
            Path::new("/bin/sh"),
            ["-c", script.as_str(), "sh", "forwarded-marker"],
        );
        let output = cmd.output().expect("re-exec command should launch");
        assert!(output.status.success(), "launcher should exit cleanly");
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert_eq!(
            stdout, "1:forwarded-marker",
            "the child must see REEXEC_GUARD_ENV=1 and the forwarded args"
        );
    }
}
