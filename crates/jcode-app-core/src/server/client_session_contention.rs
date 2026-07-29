//! User-visible warnings for multi-client session contention.
//!
//! Both situations handled here were silent before the 2026-07-20 multi-client
//! incident: a reconnect could rescope an established session's working
//! directory, and a refused takeover could leave two clients attached to one
//! session with neither told. Neither condition is refused outright (see the
//! per-function notes), so the warning *is* the mitigation, which is why it
//! lives in its own module rather than inline in the request handlers.

use super::state::fanout_live_client_event;
use super::{ClientConnectionInfo, ClientDebugState, SwarmMember};
use crate::agent::Agent;
use crate::protocol::{NotificationType, ServerEvent};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock, mpsc};

/// Warn every attached client that a reconnect moved an established session to
/// a different working directory.
///
/// The change is still applied by the caller: a client may legitimately reopen
/// a session elsewhere, and refusing would strand it against a stale directory.
/// But the move changes swarm identity and every relative path the session
/// resolves, so it must never happen silently. In the 2026-07-20 incident a
/// reconnect moved a 13-hour-old session from `/Users/jrudnik/labs/jcode` to
/// `/Users/jrudnik` with no user-visible trace.
pub(super) async fn apply_and_announce_working_dir(
    agent: &Arc<Mutex<Agent>>,
    client_event_tx: &mpsc::UnboundedSender<ServerEvent>,
    client_session_id: &str,
    client_connection_id: &str,
    new_dir: &str,
) {
    let previous_working_dir = {
        let mut agent_guard = agent.lock().await;
        let previous = agent_guard.working_dir().map(str::to_string);
        agent_guard.set_working_dir(new_dir);
        previous
    };
    let Some(previous) = previous_working_dir.as_deref() else {
        return;
    };
    if previous == new_dir {
        return;
    }
    crate::logging::warn(&format!(
        "Subscribe changed established working_dir for session {} on connection {}: {} -> {}",
        client_session_id, client_connection_id, previous, new_dir
    ));
    crate::logging::event_warn(
        "SESSION_LIFECYCLE",
        vec![
            ("phase", "subscribe_working_dir_changed".to_string()),
            ("session_id", client_session_id.to_string()),
            ("client_connection_id", client_connection_id.to_string()),
            ("previous_working_dir", previous.to_string()),
            ("new_working_dir", new_dir.to_string()),
        ],
    );
    if client_event_tx
        .send(ServerEvent::Notification {
            from_session: client_session_id.to_string(),
            from_name: None,
            notification_type: NotificationType::Message {
                scope: None,
                tldr: None,
            },
            message: format!(
                "⚠ Session working directory changed on reconnect: {} → {}. Relative paths and swarm identity now resolve against the new directory.",
                previous, new_dir
            ),
        })
        .is_err()
    {
        // The notification is the only user-visible trace of this move; the
        // warn above goes to the log, not the client. If delivery fails the
        // client is already gone, so record that the trace was lost rather
        // than reproducing the silent move this function exists to prevent.
        crate::logging::event_warn(
            "SESSION_LIFECYCLE",
            vec![
                ("phase", "subscribe_working_dir_notify_failed".to_string()),
                ("session_id", client_session_id.to_string()),
                ("client_connection_id", client_connection_id.to_string()),
                ("previous_working_dir", previous.to_string()),
                ("new_working_dir", new_dir.to_string()),
            ],
        );
    }
}

/// Warn both clients that a refused takeover left two of them on one session.
///
/// Takeover is deliberately still refused by the caller: the existing owner is
/// a *different* live client instance and may be mid-turn, so disconnecting it
/// would kill a legitimately attached user's work. Before this warning the
/// refusal fell through silently, and each client then ran its own stall guard
/// and cancelled the other's turn.
pub(super) async fn warn_dual_attach(
    swarm_members: &Arc<RwLock<HashMap<String, SwarmMember>>>,
    session_id: &str,
    client_connection_id: &str,
    conflict: &ClientConnectionInfo,
) {
    let warning = format!(
        "⚠ Two clients are attached to session '{}'. Concurrent turns can cancel each other; use one client, or start a separate session.",
        session_id
    );
    crate::logging::warn(&format!(
        "Dual attach on session {}: connection {} joined while {} is still attached; warning both clients",
        session_id, client_connection_id, conflict.client_id
    ));
    let delivered = fanout_live_client_event(
        swarm_members,
        session_id,
        ServerEvent::Notification {
            from_session: session_id.to_string(),
            from_name: None,
            notification_type: NotificationType::Message {
                scope: None,
                tldr: None,
            },
            message: warning,
        },
    )
    .await;
    crate::logging::event_warn(
        "SESSION_LIFECYCLE",
        vec![
            ("phase", "dual_attach_warned".to_string()),
            ("session_id", session_id.to_string()),
            ("client_connection_id", client_connection_id.to_string()),
            ("conflict_client_id", conflict.client_id.clone()),
            ("clients_warned", delivered.to_string()),
        ],
    );
}

/// Find another live connection already attached to `session_id`.
///
/// Contention is defined by connection identity, not client identity: the same
/// user reconnecting gets a new `client_id`, so a match here means two live
/// connections genuinely share one session.
pub(super) async fn find_conflicting_live_client(
    client_connections: &Arc<RwLock<HashMap<String, ClientConnectionInfo>>>,
    client_connection_id: &str,
    session_id: &str,
) -> Option<ClientConnectionInfo> {
    let connections = client_connections.read().await;
    connections
        .values()
        .find(|info| info.client_id != client_connection_id && info.session_id == session_id)
        .cloned()
}

/// Inputs to the resume-attach diagnostic line.
///
/// Grouped into a struct because every field is needed to reconstruct a
/// contention incident after the fact, and a free function taking eight
/// positional arguments is easy to mis-call.
pub(super) struct ResumeAttachDiagnostics<'a> {
    pub session_id: &'a str,
    pub old_session_id: &'a str,
    pub client_connection_id: &'a str,
    pub live_target_busy: bool,
    pub conflict: Option<&'a ClientConnectionInfo>,
    pub allow_session_takeover: bool,
    pub client_has_local_history: bool,
    pub incoming_client_instance_id: Option<&'a str>,
}

/// Record the full contention picture at the moment of a resume attach.
pub(super) fn log_resume_attach(d: ResumeAttachDiagnostics<'_>) {
    crate::logging::info(&format!(
        "Resume attach to existing live session {} from temporary {} on connection {}: live_target_busy={}, conflict_owner={}, conflict_processing={}, allow_takeover={}, local_history={}, incoming_instance={:?}",
        d.session_id,
        d.old_session_id,
        d.client_connection_id,
        d.live_target_busy,
        d.conflict.map(|i| i.client_id.as_str()).unwrap_or("<none>"),
        d.conflict.map(|i| i.is_processing).unwrap_or(false),
        d.allow_session_takeover,
        d.client_has_local_history,
        d.incoming_client_instance_id
    ));
}

/// Everything needed to decide, and carry out, a live-session takeover.
pub(super) struct TakeoverRequest<'a> {
    pub conflict: Option<ClientConnectionInfo>,
    pub session_id: &'a str,
    pub client_connections: &'a Arc<RwLock<HashMap<String, ClientConnectionInfo>>>,
    pub client_connection_id: &'a str,
    pub client_debug_state: &'a Arc<RwLock<ClientDebugState>>,
    pub allow_session_takeover: bool,
    pub client_has_local_history: bool,
    pub incoming_client_instance_id: Option<&'a str>,
}

/// Take the session over from a conflicting connection, or report that we did not.
///
/// Takeover requires all three of: the client asked for it, it has local
/// history for the session, and it is the *same* client instance reconnecting.
/// A distinct live instance is never evicted, because it may be mid-turn and
/// disconnecting it would kill a legitimately attached user's work.
///
/// Returns `Some(conflict)` when takeover was refused and the other client is
/// therefore still attached; the caller warns both once the new connection is
/// wired up. Returns `None` when there was no conflict or it was resolved.
pub(super) async fn resolve_live_session_takeover(
    req: TakeoverRequest<'_>,
) -> Option<ClientConnectionInfo> {
    let conflict = req.conflict?;
    let distinct_client_instances = req
        .incoming_client_instance_id
        .zip(conflict.client_instance_id.as_deref())
        .map(|(incoming, existing)| incoming != existing)
        .unwrap_or(false);
    if !(req.allow_session_takeover && req.client_has_local_history && !distinct_client_instances) {
        return Some(conflict);
    }

    let (disconnect_tx, debug_client_id, transferred_processing, transferred_tool_name) = {
        let mut connections = req.client_connections.write().await;
        match connections.remove(&conflict.client_id) {
            Some(info) => (
                Some(info.disconnect_tx),
                info.debug_client_id,
                info.is_processing,
                info.current_tool_name,
            ),
            None => (
                None,
                conflict.debug_client_id,
                conflict.is_processing,
                conflict.current_tool_name,
            ),
        }
    };
    if transferred_processing {
        crate::logging::warn(&format!(
            "Taking over live session {} from {} while old owner reports processing; new connection receives status/tool metadata but not the old processing task handle",
            req.session_id, conflict.client_id
        ));
    } else {
        crate::logging::info(&format!(
            "Taking over live session {} from idle owner {}",
            req.session_id, conflict.client_id
        ));
    }

    {
        let mut connections = req.client_connections.write().await;
        if let Some(info) = connections.get_mut(req.client_connection_id) {
            info.is_processing = transferred_processing;
            info.current_tool_name = transferred_tool_name;
        }
    }
    if let Some(debug_client_id) = debug_client_id.as_deref() {
        req.client_debug_state
            .write()
            .await
            .unregister(debug_client_id);
    }
    if let Some(disconnect_tx) = disconnect_tx {
        let _ = disconnect_tx.send(());
    }
    None
}
