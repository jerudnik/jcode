//! Watchdog for the "stuck on loading session…" bug.
//!
//! Split out of `remote.rs` because it is a self-contained policy unit: three
//! timing constants and the single function that decides, on each tick, whether
//! to re-request history and what to tell the user when re-requests run out.
//!
//! The distinction this module exists to preserve: **"we gave up waiting" is
//! not the same fact as "the session is gone."** A server that is merely slow
//! must never be reported as unavailable, because advising `/restart` there
//! discards a session that was about to answer.

use super::super::backend::RemoteConnection;
use super::{App, DisplayMessage};
use std::time::{Duration, Instant};

/// Watchdog bookkeeping for one remote connection's history bootstrap.
///
/// When a remote (re)connect never receives the bootstrap `History` event,
/// every prompt path is gated behind `has_loaded_history()` and the session is
/// stuck on "loading session…". These fields track when the current connection
/// began waiting and how many times history has been re-requested, so the
/// watchdog can re-issue `GetHistory` a bounded number of times before giving
/// up.
#[derive(Default)]
pub(super) struct HistoryRecoveryState {
    /// When the current connection started waiting for bootstrap history.
    pub(super) wait_started: Option<Instant>,
    pub(super) attempts: u32,
    pub(super) last_attempt: Option<Instant>,
    /// Whether the most recent history re-request was accepted by the server.
    ///
    /// This is the signal that separates "slow" from "unavailable". A send that
    /// succeeds proves the connection is alive and the server simply has not
    /// answered yet (a cold model-catalog build has been measured at 17s), so
    /// advising `/restart` would discard a working session.
    pub(super) last_send_ok: bool,
}

/// First wait before the watchdog re-requests history. Generous enough that a
/// normal (slow) bootstrap completes on its own, short enough that a genuinely
/// stuck session recovers in seconds instead of requiring a manual `/restart`.
pub(super) const REMOTE_HISTORY_RECOVERY_FIRST_DELAY: Duration = Duration::from_secs(6);
/// Spacing between subsequent re-requests after the first one.
pub(super) const REMOTE_HISTORY_RECOVERY_RETRY_INTERVAL: Duration = Duration::from_secs(5);
/// How many times we re-request history before giving up and telling the user
/// to `/restart`. Bounded so a server that genuinely never answers does not
/// spin forever.
pub(super) const REMOTE_HISTORY_RECOVERY_MAX_ATTEMPTS: u32 = 4;

/// Watchdog for the "stuck on loading session…" bug.
///
/// Every remote prompt path is gated behind `RemoteConnection::has_loaded_history()`:
/// `submit_prepared_remote_input` parks prompts in `pending_prompt_before_history`,
/// and `process_remote_followups` returns early at the `!has_loaded_history()`
/// gate. That gate only clears when the server delivers a `History` event. If
/// that event never arrives after a (re)connect or reload handoff (dropped
/// event, momentarily busy agent, or a path that returned without history), the
/// client is stuck forever showing "loading session…" and the only escape is a
/// manual `/restart`.
///
/// This watchdog detects a connection that has been waiting too long for the
/// bootstrap history and re-requests it a bounded number of times. If history
/// still never loads, it surfaces an actionable message instead of leaving the
/// user staring at a frozen header.
pub(super) async fn recover_stuck_remote_history(
    app: &mut App,
    remote: &mut RemoteConnection,
) -> bool {
    // Once history has loaded the watchdog has nothing to do; make sure its
    // budget is cleared so a later rewind-triggered reload starts fresh.
    if remote.has_loaded_history() {
        if app.remote_history_recovery.wait_started.is_some() {
            app.clear_remote_history_wait();
        }
        return false;
    }

    // A pending server reload intentionally leaves history unloaded until the
    // reload fires (see `process_remote_followups`); don't fight that path.
    if app.pending_server_reload {
        return false;
    }

    // Only meaningful for an established remote client connection. During the
    // initial connect/reconnect handshake the run loop drives history loading
    // directly, and there is no point re-requesting before we've attached.
    if !app.is_remote {
        return false;
    }

    let now = Instant::now();
    let waited = match app.remote_history_recovery.wait_started {
        Some(started) => now.saturating_duration_since(started),
        None => {
            // Begin tracking from the first tick that observes unloaded history
            // on a live connection.
            app.remote_history_recovery.wait_started = Some(now);
            return false;
        }
    };

    if waited < REMOTE_HISTORY_RECOVERY_FIRST_DELAY {
        return false;
    }

    if app.remote_history_recovery.attempts >= REMOTE_HISTORY_RECOVERY_MAX_ATTEMPTS {
        // We've exhausted re-requests. Surface a one-time actionable hint so the
        // user isn't stuck on a silent "loading session…" forever.
        if app.remote_history_recovery.last_attempt.is_some() {
            // Distinguish "slow" from "unavailable". Every re-request above was
            // *accepted* by the server, so the connection is alive and the
            // session is not lost: it is busy behind a long-running request
            // (a cold model-catalog build has been measured at 17s). Telling
            // the user to /restart here is both false and harmful, because it
            // discards a session that was about to answer.
            // A re-request that the server accepted proves the socket is alive.
            let alive = app.remote_history_recovery.last_send_ok;
            crate::logging::warn(&format!(
                "Remote history not loaded after {} re-requests (connected={}); reporting a slow \
                 startup rather than an unavailable session",
                REMOTE_HISTORY_RECOVERY_MAX_ATTEMPTS, alive,
            ));
            if alive {
                app.push_display_message(DisplayMessage::system(
                    "⏳ Still starting up… the server is connected but hasn't finished \
                     preparing this session yet. It should arrive on its own; there is no \
                     need to restart."
                        .to_string(),
                ));
                app.set_status_notice("Still starting up - server connected, preparing session");
            } else {
                app.push_display_message(DisplayMessage::system(
                    "⚠ Lost contact with the server while loading this session. \
                     Run /restart to reconnect."
                        .to_string(),
                ));
                app.set_status_notice("Disconnected while loading - try /restart");
            }
            // Clear last_attempt so we don't repeat the message every tick, but
            // keep attempts at max so we don't re-enter the retry path.
            app.remote_history_recovery.last_attempt = None;
            return true;
        }
        return false;
    }

    // Rate-limit re-requests so we don't flood the server.
    if let Some(last) = app.remote_history_recovery.last_attempt
        && now.saturating_duration_since(last) < REMOTE_HISTORY_RECOVERY_RETRY_INTERVAL
    {
        return false;
    }

    app.remote_history_recovery.attempts += 1;
    app.remote_history_recovery.last_attempt = Some(now);
    crate::logging::warn(&format!(
        "Remote history still not loaded after {}s; re-requesting session history (attempt {}/{}, session={:?})",
        waited.as_secs(),
        app.remote_history_recovery.attempts,
        REMOTE_HISTORY_RECOVERY_MAX_ATTEMPTS,
        app.remote_session_id,
    ));
    match remote.request_history().await {
        Ok(_) => {
            app.remote_history_recovery.last_send_ok = true;
            app.set_status_notice("Loading session… re-requesting history");
        }
        Err(err) => {
            app.remote_history_recovery.last_send_ok = false;
            crate::logging::error(&format!(
                "History recovery re-request failed: {err}; will retry on next watchdog tick"
            ));
        }
    }
    true
}
