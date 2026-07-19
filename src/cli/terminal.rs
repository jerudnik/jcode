use anyhow::Result;
use std::io::{self, IsTerminal, Write};
use std::panic;

use crate::{id, session, telemetry, tui};

pub struct TuiRuntimeState {
    mouse_capture: bool,
    keyboard_enhanced: bool,
    focus_change: bool,
}

struct InheritedTerminalGuard {
    state: TuiRuntimeState,
    armed: bool,
}

impl InheritedTerminalGuard {
    fn new(modes: InheritedTerminalModes) -> Self {
        Self {
            state: TuiRuntimeState {
                mouse_capture: modes.mouse_capture,
                keyboard_enhanced: modes.keyboard_enhanced,
                focus_change: modes.focus_change,
            },
            armed: true,
        }
    }

    fn transfer_to(mut self, guard: TuiRuntimeGuard) -> TuiRuntimeGuard {
        self.armed = false;
        guard
    }
}

impl Drop for InheritedTerminalGuard {
    fn drop(&mut self) {
        if self.armed {
            cleanup_tui_runtime(&self.state, true);
            self.armed = false;
        }
    }
}

const INHERITED_MODES_ENV: &str = "JCODE_TUI_INHERITED_MODES";
const INHERITED_THEME_ENV: &str = "JCODE_TUI_INHERITED_THEME";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct InheritedTerminalModes {
    mouse_capture: bool,
    keyboard_enhanced: bool,
    focus_change: bool,
}

impl InheritedTerminalModes {
    fn encode(self) -> String {
        format!(
            "mouse={},keyboard={},focus={}",
            u8::from(self.mouse_capture),
            u8::from(self.keyboard_enhanced),
            u8::from(self.focus_change)
        )
    }

    fn decode(value: &str) -> Option<Self> {
        let mut modes = Self {
            mouse_capture: false,
            keyboard_enhanced: false,
            focus_change: false,
        };
        let mut seen = 0u8;
        for field in value.split(',') {
            let (name, raw) = field.split_once('=')?;
            let enabled = match raw {
                "0" => false,
                "1" => true,
                _ => return None,
            };
            match name {
                "mouse" => {
                    modes.mouse_capture = enabled;
                    seen |= 1;
                }
                "keyboard" => {
                    modes.keyboard_enhanced = enabled;
                    seen |= 2;
                }
                "focus" => {
                    modes.focus_change = enabled;
                    seen |= 4;
                }
                _ => return None,
            }
        }
        (seen == 7).then_some(modes)
    }
}

fn has_terminal_exec_handoff(
    is_resuming: bool,
    inherited_modes: Option<InheritedTerminalModes>,
) -> bool {
    is_resuming && inherited_modes.is_some()
}

/// RAII guard that guarantees the terminal is restored to a sane state when the
/// TUI runtime ends, even if the run loop returns an error or unwinds via panic.
///
/// Without this guard, an error propagated by `?` (e.g. an I/O error from a
/// `terminal.draw` call, or any other fallible step in the event loop) would
/// skip the explicit `cleanup_tui_runtime` call and leave the terminal in raw
/// mode / alternate screen. That manifests as a corrupted terminal after exit:
/// typed input is invisible because echo and cooked mode were never restored
/// (see issue #214).
///
/// The normal teardown path should call [`TuiRuntimeGuard::finish`] (or
/// [`TuiRuntimeGuard::finish_for_run_result`]) which performs the restore and
/// disarms the guard. If neither is called (error/panic path), `Drop` performs
/// a best-effort full restore.
pub struct TuiRuntimeGuard {
    state: TuiRuntimeState,
    armed: bool,
}

#[cfg(test)]
thread_local! {
    /// Counts how many times the guard's `Drop` performed an emergency restore.
    /// Used by tests to verify the error/panic safety net fires exactly once.
    static GUARD_DROP_RESTORES: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
    static CLEANUP_RECORDS: std::cell::RefCell<Vec<(bool, bool, bool, bool)>> = const {
        std::cell::RefCell::new(Vec::new())
    };
    static SUPPRESS_REAL_CLEANUP: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

impl TuiRuntimeGuard {
    fn new(state: TuiRuntimeState) -> Self {
        Self { state, armed: true }
    }

    /// Normal teardown for the simple case: restore the terminal and disarm.
    pub fn finish(mut self, restore_terminal: bool) {
        cleanup_tui_runtime(&self.state, restore_terminal);
        self.armed = false;
    }

    /// Complete an interactive run while retaining ownership across a possible
    /// exec handoff. On the exec success path `action` never returns. If it does
    /// return an error, this guard remains armed and its `Drop` performs a full
    /// restore before the error propagates.
    pub fn finish_for_run_result(
        mut self,
        run_result: &crate::tui::RunResult,
        extra_exec: bool,
        action: impl FnOnce() -> Result<()>,
    ) -> Result<()> {
        if run_result_will_exec(run_result, extra_exec) {
            export_tui_exec_handoff(&self.state);
            cleanup_tui_runtime(&self.state, false);
            action()?;
        } else {
            cleanup_tui_runtime(&self.state, true);
            action()?;
        }
        self.armed = false;
        Ok(())
    }
}

impl Drop for TuiRuntimeGuard {
    fn drop(&mut self) {
        if self.armed {
            // Reached only on an error/panic path that skipped explicit
            // teardown. Always perform a full restore so the user's terminal is
            // not left corrupted.
            cleanup_tui_runtime(&self.state, true);
            self.armed = false;
            #[cfg(test)]
            GUARD_DROP_RESTORES.with(|c| c.set(c.get() + 1));
        }
    }
}

pub fn set_current_session(session_id: &str) {
    crate::set_current_session(session_id);
}

pub fn get_current_session() -> Option<String> {
    crate::get_current_session()
}

pub fn install_panic_hook() {
    let default_hook = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        default_hook(info);

        if let Some(session_id) = get_current_session() {
            print_session_resume_hint(&session_id);

            if let Some((provider, model)) = telemetry::current_provider_model() {
                telemetry::record_crash(&provider, &model, telemetry::SessionEndReason::Panic);
            }

            if let Ok(mut session) = session::Session::load(&session_id)
                && let Err(error) =
                    session.mark_crashed_and_persist(Some(format!("Panic: {}", info)))
            {
                crate::logging::warn(&format!(
                    "failed to persist panic crash state for {}: {}",
                    session_id, error
                ));
            }
        }
    }));
}

pub fn mark_current_session_crashed(message: String) {
    if let Some(session_id) = get_current_session() {
        if let Some((provider, model)) = telemetry::current_provider_model() {
            telemetry::record_crash(&provider, &model, telemetry::SessionEndReason::Signal);
        }
        if let Ok(mut session) = session::Session::load(&session_id)
            && matches!(session.status, session::SessionStatus::Active)
            && let Err(error) = session.mark_crashed_and_persist(Some(message))
        {
            crate::logging::warn(&format!(
                "failed to persist signal crash state for {}: {}",
                session_id, error
            ));
        }
    }
}

pub fn panic_payload_to_string(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "unknown panic payload".to_string()
    }
}

pub fn show_crash_resume_hint() {
    let crashed = session::find_recent_crashed_sessions();
    if crashed.is_empty() {
        return;
    }

    let (id, name) = &crashed[0];
    let session_label = id::extract_session_name(id).unwrap_or(name.as_str());

    if crashed.len() == 1 {
        eprintln!(
            "\x1b[33m💥 Session \x1b[1m{}\x1b[0m\x1b[33m crashed. Resume with:\x1b[0m  jcode --resume {}",
            session_label, id
        );
    } else {
        eprintln!(
            "\x1b[33m💥 {} sessions crashed recently. Most recent: \x1b[1m{}\x1b[0m",
            crashed.len(),
            session_label
        );
        eprintln!("\x1b[33m   Resume with:\x1b[0m  jcode --resume {}", id);
        eprintln!("\x1b[33m   List all:\x1b[0m     jcode --resume");
    }
    eprintln!();
}

fn init_tui_terminal(inherited_terminal: bool) -> Result<ratatui::DefaultTerminal> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        anyhow::bail!("jcode TUI requires an interactive terminal (stdin/stdout must be a TTY)");
    }
    if inherited_terminal {
        init_tui_terminal_resume()
    } else {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(ratatui::init)).map_err(|payload| {
            anyhow::anyhow!(
                "failed to initialize terminal: {}",
                panic_payload_to_string(payload.as_ref())
            )
        })
    }
}

pub fn init_tui_runtime() -> Result<(ratatui::DefaultTerminal, TuiRuntimeGuard)> {
    let is_resuming = std::env::var_os("JCODE_RESUMING").is_some();
    let inherited_theme = std::env::var(INHERITED_THEME_ENV).ok();
    let inherited_modes_raw = std::env::var(INHERITED_MODES_ENV).ok();
    let inherited_modes = inherited_modes_raw
        .as_deref()
        .and_then(InheritedTerminalModes::decode);
    // JCODE_RESUMING describes the session lifecycle, but only a valid modes
    // handoff proves the previous process deliberately left the terminal live
    // across exec. A restart used to restore the terminal before exec while the
    // new process still took the resume path, leaving it on the primary screen
    // without mouse capture.
    let inherited_terminal = has_terminal_exec_handoff(is_resuming, inherited_modes);
    // Own inherited protocol modes before any successor initialization can
    // fail or unwind. Ownership transfers only after the normal runtime guard
    // has been constructed.
    let inherited_guard = inherited_terminal
        .then(|| InheritedTerminalGuard::new(inherited_modes.expect("validated handoff modes")));
    if inherited_terminal {
        // OSC terminal queries are unsafe here because the previous process
        // deliberately exec'd without leaving raw mode or the alternate screen.
        crate::tui::theme_detect::init_theme_mode_for_resume(inherited_theme.as_deref());
    } else {
        // The OSC 11 query needs the cooked terminal and must happen before init.
        crate::tui::theme_detect::init_theme_mode();
    }
    let terminal = init_tui_terminal(inherited_terminal)?;
    crate::tui::mermaid::install_jcode_mermaid_hooks();
    crate::tui::markdown::install_jcode_markdown_hooks();
    crate::tui::mermaid::init_picker();

    let perf_policy = crate::perf::tui_policy();
    // These private handoff values apply only to this exec boundary. Avoid
    // leaking them into tools or unrelated child jcode processes.
    crate::env::remove_var(INHERITED_MODES_ENV);
    crate::env::remove_var(INHERITED_THEME_ENV);

    let fallback_modes = InheritedTerminalModes {
        mouse_capture: perf_policy.enable_mouse_capture,
        keyboard_enhanced: perf_policy.enable_keyboard_enhancement,
        focus_change: perf_policy.enable_focus_change,
    };
    let modes = if inherited_terminal {
        // The previous process intentionally preserved these modes across exec.
        // Reassert idempotent modes because terminals, multiplexers, or an older
        // process may have cleared them during the handoff. Do not push Kitty's
        // stack-based keyboard enhancement flags again. A later normal exit must
        // still disable every inherited mode, so retain them in the guard.
        let modes = inherited_modes.unwrap_or(fallback_modes);
        crossterm::execute!(std::io::stdout(), crossterm::event::EnableBracketedPaste)?;
        if modes.focus_change {
            crossterm::execute!(std::io::stdout(), crossterm::event::EnableFocusChange)?;
        }
        if modes.mouse_capture {
            crossterm::execute!(std::io::stdout(), crossterm::event::EnableMouseCapture)?;
        }
        modes
    } else {
        let keyboard_enhanced = if perf_policy.enable_keyboard_enhancement {
            tui::enable_keyboard_enhancement()
        } else {
            false
        };
        let modes = InheritedTerminalModes {
            mouse_capture: perf_policy.enable_mouse_capture,
            keyboard_enhanced,
            focus_change: perf_policy.enable_focus_change,
        };
        crossterm::execute!(std::io::stdout(), crossterm::event::EnableBracketedPaste)?;
        if modes.focus_change {
            crossterm::execute!(std::io::stdout(), crossterm::event::EnableFocusChange)?;
        }
        if modes.mouse_capture {
            crossterm::execute!(std::io::stdout(), crossterm::event::EnableMouseCapture)?;
        }
        modes
    };

    crate::logging::info(&format!(
        "EVENT event=TUI_TERMINAL_MODES phase=initialized pid={} resuming={} handoff={} handoff_raw={} raw_mode={} mouse_capture={} keyboard_enhanced={} focus_change={} idempotent_modes_reasserted={}",
        std::process::id(),
        is_resuming,
        inherited_terminal,
        inherited_modes_raw.as_deref().unwrap_or("none"),
        crossterm::terminal::is_raw_mode_enabled().unwrap_or(false),
        modes.mouse_capture,
        modes.keyboard_enhanced,
        modes.focus_change,
        inherited_terminal,
    ));

    let runtime_guard = TuiRuntimeGuard::new(TuiRuntimeState {
        mouse_capture: modes.mouse_capture,
        keyboard_enhanced: modes.keyboard_enhanced,
        focus_change: modes.focus_change,
    });
    let runtime_guard = match inherited_guard {
        Some(guard) => guard.transfer_to(runtime_guard),
        None => runtime_guard,
    };

    Ok((terminal, runtime_guard))
}

fn cleanup_tui_runtime(state: &TuiRuntimeState, restore_terminal: bool) {
    #[cfg(test)]
    CLEANUP_RECORDS.with(|records| {
        records.borrow_mut().push((
            restore_terminal,
            state.mouse_capture,
            state.keyboard_enhanced,
            state.focus_change,
        ));
    });
    crate::logging::info(&format!(
        "EVENT event=TUI_TERMINAL_MODES phase=cleanup pid={} restore_terminal={} raw_mode={} mouse_capture={} keyboard_enhanced={} focus_change={}",
        std::process::id(),
        restore_terminal,
        crossterm::terminal::is_raw_mode_enabled().unwrap_or(false),
        state.mouse_capture,
        state.keyboard_enhanced,
        state.focus_change,
    ));
    if restore_terminal {
        #[cfg(test)]
        let suppress_real_cleanup = SUPPRESS_REAL_CLEANUP.with(std::cell::Cell::get);
        #[cfg(not(test))]
        let suppress_real_cleanup = false;
        if !suppress_real_cleanup {
            let _ = write_terminal_protocol_cleanup(std::io::stdout(), state);
            ratatui::restore();
        }
    }

    crate::tui::mermaid::clear_image_state();
}

fn write_terminal_protocol_cleanup(
    mut writer: impl Write,
    state: &TuiRuntimeState,
) -> io::Result<()> {
    let mut first_error = None;
    let mut remember = |result: io::Result<()>| {
        if let Err(error) = result
            && first_error.is_none()
        {
            first_error = Some(error);
        }
    };

    remember(crossterm::execute!(
        writer,
        crossterm::event::DisableBracketedPaste
    ));
    if state.focus_change {
        remember(crossterm::execute!(
            writer,
            crossterm::event::DisableFocusChange
        ));
    }
    if state.mouse_capture {
        remember(crossterm::execute!(
            writer,
            crossterm::event::DisableMouseCapture
        ));
    }
    if state.keyboard_enhanced {
        remember(crossterm::execute!(
            writer,
            crossterm::event::PopKeyboardEnhancementFlags
        ));
    }
    first_error.map_or(Ok(()), Err)
}

#[cfg(unix)]
fn restore_signal_terminal(
    mut writer: impl Write,
    disable_raw_mode: impl FnOnce(),
) -> io::Result<()> {
    let protocol_result =
        write_terminal_protocol_cleanup(&mut writer, &inherited_all_modes_state());
    disable_raw_mode();
    let screen_result = crossterm::execute!(
        writer,
        crossterm::terminal::LeaveAlternateScreen,
        crossterm::cursor::Show
    );
    protocol_result.and(screen_result)
}

#[cfg(unix)]
fn inherited_all_modes_state() -> TuiRuntimeState {
    TuiRuntimeState {
        mouse_capture: true,
        keyboard_enhanced: true,
        focus_change: true,
    }
}

fn run_result_will_exec(run_result: &crate::tui::RunResult, extra_exec: bool) -> bool {
    extra_exec
        || run_result.reload_session.is_some()
        || run_result.rebuild_session.is_some()
        || run_result.update_session.is_some()
        || run_result.restart_session.is_some()
}

fn export_tui_exec_handoff(state: &TuiRuntimeState) {
    let modes = InheritedTerminalModes {
        mouse_capture: state.mouse_capture,
        keyboard_enhanced: state.keyboard_enhanced,
        focus_change: state.focus_change,
    };
    crate::env::set_var(INHERITED_MODES_ENV, modes.encode());
    let theme = crate::tui::theme_detect::current_theme_label();
    crate::env::set_var(INHERITED_THEME_ENV, theme);
    crate::logging::info(&format!(
        "EVENT event=TUI_TERMINAL_MODES phase=exec_handoff pid={} raw_mode={} modes={} theme={}",
        std::process::id(),
        crossterm::terminal::is_raw_mode_enabled().unwrap_or(false),
        modes.encode(),
        theme,
    ));
}

pub fn print_session_resume_hint(session_id: &str) {
    let _ = write_session_resume_hint(io::stderr().lock(), session_id);
}

fn write_session_resume_hint(mut writer: impl Write, session_id: &str) -> io::Result<()> {
    let session_name = id::extract_session_name(session_id).unwrap_or(session_id);
    writeln!(writer)?;
    writeln!(
        writer,
        "\x1b[33mSession \x1b[1m{}\x1b[0m\x1b[33m - to resume:\x1b[0m",
        session_name
    )?;
    writeln!(writer, "  jcode --resume {}", session_id)?;
    writeln!(writer)?;
    Ok(())
}

fn init_tui_terminal_resume() -> Result<ratatui::DefaultTerminal> {
    use ratatui::{Terminal, backend::CrosstermBackend};

    crossterm::terminal::enable_raw_mode()
        .map_err(|e| anyhow::anyhow!("failed to enable raw mode on resume: {}", e))?;

    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)
        .map_err(|e| anyhow::anyhow!("failed to create terminal on resume: {}", e))?;

    terminal
        .clear()
        .map_err(|e| anyhow::anyhow!("failed to clear terminal on resume: {}", e))?;

    Ok(terminal)
}

#[cfg(unix)]
pub fn signal_name(sig: i32) -> &'static str {
    match sig {
        1 => "SIGHUP",
        2 => "SIGINT",
        3 => "SIGQUIT",
        4 => "SIGILL",
        6 => "SIGABRT",
        9 => "SIGKILL",
        11 => "SIGSEGV",
        13 => "SIGPIPE",
        14 => "SIGALRM",
        15 => "SIGTERM",
        _ => "unknown",
    }
}

#[cfg(not(unix))]
pub fn signal_name(_sig: i32) -> &'static str {
    "unknown"
}

#[cfg(unix)]
fn signal_crash_reason(sig: i32) -> String {
    match sig {
        libc::SIGHUP => "Terminal or window closed (SIGHUP)".to_string(),
        libc::SIGTERM => "Terminated (SIGTERM)".to_string(),
        libc::SIGINT => "Interrupted (SIGINT)".to_string(),
        libc::SIGQUIT => "Quit signal (SIGQUIT)".to_string(),
        _ => format!("Terminated by signal {} ({})", signal_name(sig), sig),
    }
}

#[cfg(unix)]
fn handle_termination_signal(sig: i32) -> ! {
    mark_current_session_crashed(signal_crash_reason(sig));

    let _ = restore_signal_terminal(std::io::stderr(), || {
        let _ = crossterm::terminal::disable_raw_mode();
    });

    if let Some(session_id) = get_current_session() {
        print_session_resume_hint(&session_id);
    }

    std::process::exit(128 + sig);
}

#[cfg(unix)]
pub fn spawn_session_signal_watchers() {
    use tokio::signal::unix::{SignalKind, signal};

    fn spawn_one(sig: i32, kind: SignalKind) {
        tokio::spawn(async move {
            let mut stream = match signal(kind) {
                Ok(s) => s,
                Err(e) => {
                    crate::logging::error(&format!(
                        "Failed to install {} handler: {}",
                        signal_name(sig),
                        e
                    ));
                    return;
                }
            };
            if stream.recv().await.is_some() {
                crate::logging::info(&format!("Received {} in TUI process", signal_name(sig)));
                handle_termination_signal(sig);
            }
        });
    }

    spawn_one(libc::SIGHUP, SignalKind::hangup());
    spawn_one(libc::SIGTERM, SignalKind::terminate());
    spawn_one(libc::SIGINT, SignalKind::interrupt());
    spawn_one(libc::SIGQUIT, SignalKind::quit());
}

#[cfg(not(unix))]
pub fn spawn_session_signal_watchers() {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static TEST_SESSION_LOCK: Mutex<()> = Mutex::new(());

    fn test_guard() -> TuiRuntimeGuard {
        // All terminal-mode flags disabled so teardown only performs the minimal
        // (and TTY-safe) restore path during tests.
        TuiRuntimeGuard::new(TuiRuntimeState {
            mouse_capture: false,
            keyboard_enhanced: false,
            focus_change: false,
        })
    }

    fn all_modes() -> InheritedTerminalModes {
        InheritedTerminalModes {
            mouse_capture: true,
            keyboard_enhanced: true,
            focus_change: true,
        }
    }

    fn reset_cleanup_records() {
        CLEANUP_RECORDS.with(|records| records.borrow_mut().clear());
        SUPPRESS_REAL_CLEANUP.with(|suppress| suppress.set(true));
    }

    fn cleanup_records() -> Vec<(bool, bool, bool, bool)> {
        CLEANUP_RECORDS.with(|records| records.borrow().clone())
    }

    #[test]
    fn inherited_terminal_modes_roundtrip() {
        let modes = InheritedTerminalModes {
            mouse_capture: true,
            keyboard_enhanced: false,
            focus_change: true,
        };
        assert_eq!(InheritedTerminalModes::decode(&modes.encode()), Some(modes));
    }

    #[test]
    fn inherited_terminal_modes_reject_malformed_values() {
        assert_eq!(InheritedTerminalModes::decode("mouse=1,keyboard=1"), None);
        assert_eq!(
            InheritedTerminalModes::decode("mouse=yes,keyboard=1,focus=1"),
            None
        );
    }

    #[test]
    fn resume_requires_valid_terminal_handoff_metadata() {
        let modes = InheritedTerminalModes {
            mouse_capture: true,
            keyboard_enhanced: true,
            focus_change: true,
        };
        assert!(has_terminal_exec_handoff(true, Some(modes)));
        assert!(!has_terminal_exec_handoff(true, None));
        assert!(!has_terminal_exec_handoff(false, Some(modes)));
    }

    #[test]
    fn every_exec_action_preserves_terminal_modes() {
        let with = |field: &str| {
            let mut result = crate::tui::RunResult::default();
            match field {
                "reload" => result.reload_session = Some("session_test".into()),
                "rebuild" => result.rebuild_session = Some("session_test".into()),
                "update" => result.update_session = Some("session_test".into()),
                "restart" => result.restart_session = Some("session_test".into()),
                _ => unreachable!(),
            }
            result
        };

        for field in ["reload", "rebuild", "update", "restart"] {
            assert!(
                run_result_will_exec(&with(field), false),
                "{field} must preserve terminal modes across exec"
            );
        }
        assert!(run_result_will_exec(
            &crate::tui::RunResult::default(),
            true
        ));
        assert!(!run_result_will_exec(
            &crate::tui::RunResult::default(),
            false
        ));
    }

    #[test]
    fn guard_drop_restores_terminal_when_not_finished() {
        // Simulates the error/panic path where explicit teardown is skipped:
        // the guard must restore the terminal exactly once on drop (issue #214).
        GUARD_DROP_RESTORES.with(|c| c.set(0));
        {
            let _guard = test_guard();
        }
        let restores = GUARD_DROP_RESTORES.with(|c| c.get());
        assert_eq!(
            restores, 1,
            "dropping an un-finished guard must restore the terminal once"
        );
    }

    #[test]
    fn guard_finish_disarms_drop_restore() {
        // The happy path calls finish(); the drop safety net must NOT fire again.
        GUARD_DROP_RESTORES.with(|c| c.set(0));
        let guard = test_guard();
        guard.finish(true);
        let restores = GUARD_DROP_RESTORES.with(|c| c.get());
        assert_eq!(
            restores, 0,
            "finish() should disarm the guard so drop does not double-restore"
        );
    }

    #[test]
    fn failed_exec_handoff_restores_all_inherited_modes() {
        reset_cleanup_records();
        GUARD_DROP_RESTORES.with(|c| c.set(0));
        let mut run_result = crate::tui::RunResult::default();
        run_result.reload_session = Some("session_exec_failure".into());
        let guard = TuiRuntimeGuard::new(TuiRuntimeState {
            mouse_capture: true,
            keyboard_enhanced: true,
            focus_change: true,
        });

        let error = guard
            .finish_for_run_result(&run_result, false, || {
                Err(anyhow::anyhow!("mock replace_process failure"))
            })
            .expect_err("mock exec replacement must fail");

        assert!(error.to_string().contains("mock replace_process failure"));
        assert_eq!(
            cleanup_records(),
            vec![(false, true, true, true), (true, true, true, true)],
            "handoff export must be followed by a full restore when exec returns"
        );
        assert_eq!(GUARD_DROP_RESTORES.with(|c| c.get()), 1);
        crate::env::remove_var(INHERITED_MODES_ENV);
        crate::env::remove_var(INHERITED_THEME_ENV);
    }

    #[test]
    fn inherited_guard_restores_at_every_successor_init_boundary() {
        for boundary in [
            "theme_resume",
            "enable_raw_mode",
            "terminal_new",
            "terminal_clear",
            "hook_install",
            "perf_policy",
            "bracketed_paste",
            "focus_change",
            "mouse_capture",
            "runtime_guard_construction",
        ] {
            reset_cleanup_records();
            let result: Result<()> = (|| {
                let _handoff = InheritedTerminalGuard::new(all_modes());
                Err(anyhow::anyhow!("injected failure at {boundary}"))
            })();
            assert!(result.is_err());
            assert_eq!(
                cleanup_records(),
                vec![(true, true, true, true)],
                "inherited modes were not restored at boundary {boundary}"
            );
        }

        reset_cleanup_records();
        let unwind = std::panic::catch_unwind(|| {
            let _handoff = InheritedTerminalGuard::new(all_modes());
            panic!("injected successor unwind");
        });
        assert!(unwind.is_err());
        assert_eq!(cleanup_records(), vec![(true, true, true, true)]);
    }

    #[test]
    fn inherited_guard_transfers_without_early_cleanup() {
        reset_cleanup_records();
        let handoff = InheritedTerminalGuard::new(all_modes());
        let runtime = handoff.transfer_to(TuiRuntimeGuard::new(TuiRuntimeState {
            mouse_capture: true,
            keyboard_enhanced: true,
            focus_change: true,
        }));
        assert!(cleanup_records().is_empty());
        runtime.finish(false);
        assert_eq!(cleanup_records(), vec![(false, true, true, true)]);
    }

    #[test]
    fn protocol_cleanup_emits_all_four_disables() {
        let mut output = Vec::new();
        write_terminal_protocol_cleanup(
            &mut output,
            &TuiRuntimeState {
                mouse_capture: true,
                keyboard_enhanced: true,
                focus_change: true,
            },
        )
        .unwrap();

        assert!(output.windows(8).any(|bytes| bytes == b"\x1b[?2004l"));
        assert!(output.windows(8).any(|bytes| bytes == b"\x1b[?1004l"));
        assert!(output.windows(8).any(|bytes| bytes == b"\x1b[?1006l"));
        assert!(output.windows(5).any(|bytes| bytes == b"\x1b[<1u"));
    }

    #[cfg(unix)]
    #[test]
    fn signal_cleanup_emits_full_disable_set_and_leaves_alt_screen() {
        let mut output = Vec::new();
        let raw_mode_disabled = std::cell::Cell::new(false);
        restore_signal_terminal(&mut output, || raw_mode_disabled.set(true)).unwrap();

        assert!(raw_mode_disabled.get());
        assert!(output.windows(8).any(|bytes| bytes == b"\x1b[?2004l"));
        assert!(output.windows(8).any(|bytes| bytes == b"\x1b[?1004l"));
        assert!(output.windows(8).any(|bytes| bytes == b"\x1b[?1006l"));
        assert!(output.windows(5).any(|bytes| bytes == b"\x1b[<1u"));
        assert!(output.windows(8).any(|bytes| bytes == b"\x1b[?1049l"));
    }

    #[test]
    fn test_session_recovery_tracking() {
        let _guard = TEST_SESSION_LOCK.lock().unwrap();
        set_current_session("test_session_123");

        let stored = get_current_session();
        assert_eq!(stored.as_deref(), Some("test_session_123"));
    }

    #[test]
    fn test_session_recovery_message_format() {
        let _guard = TEST_SESSION_LOCK.lock().unwrap();
        let test_session = "session_format_test_12345";
        set_current_session(test_session);

        if let Some(session_id) = get_current_session() {
            let mut output = Vec::new();
            write_session_resume_hint(&mut output, &session_id).unwrap();
            let output = String::from_utf8(output).unwrap();
            let expected_cmd = format!("jcode --resume {}", session_id);
            assert!(output.contains(&expected_cmd));
            assert!(output.contains("to resume"));
            assert!(!session_id.is_empty());
        } else {
            panic!("Session ID should be set");
        }
    }

    #[test]
    fn session_resume_hint_writer_reports_closed_stderr_without_panicking() {
        struct ClosedWriter;

        impl Write for ClosedWriter {
            fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
                Err(io::Error::new(io::ErrorKind::BrokenPipe, "stderr closed"))
            }

            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }

        let error = write_session_resume_hint(ClosedWriter, "session_closed_pipe")
            .expect_err("closed stderr should be reported as an I/O error");
        assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
    }
}
