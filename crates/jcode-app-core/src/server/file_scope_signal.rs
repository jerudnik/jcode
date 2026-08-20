use super::{
    FileAccess, FileTouchService, SessionInterruptQueues, SwarmMember,
    queue_soft_interrupt_for_session,
};
use crate::agent::Agent;
use crate::bus::FileTouch;
use jcode_agent_runtime::SoftInterruptSource;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{Mutex, RwLock};

type SessionAgents = Arc<RwLock<HashMap<String, Arc<Mutex<Agent>>>>>;
type ScopeSignalKey = (String, String, PathBuf);

pub(super) struct FileScopeSignals {
    swarm_members: Arc<RwLock<HashMap<String, SwarmMember>>>,
    swarm_coordinators: Arc<RwLock<HashMap<String, String>>>,
    sessions: SessionAgents,
    soft_interrupt_queues: SessionInterruptQueues,
    sent: HashSet<ScopeSignalKey>,
}

impl FileScopeSignals {
    pub(super) fn new(
        swarm_members: Arc<RwLock<HashMap<String, SwarmMember>>>,
        swarm_coordinators: Arc<RwLock<HashMap<String, String>>>,
        sessions: SessionAgents,
        soft_interrupt_queues: SessionInterruptQueues,
    ) -> Self {
        Self {
            swarm_members,
            swarm_coordinators,
            sessions,
            soft_interrupt_queues,
            sent: HashSet::new(),
        }
    }

    pub(super) async fn record_touch(&mut self, file_touch: &FileTouchService, touch: &FileTouch) {
        file_touch
            .record_touch(
                touch.path.clone(),
                FileAccess {
                    session_id: touch.session_id.clone(),
                    op: touch.op.clone(),
                    timestamp: Instant::now(),
                    absolute_time: std::time::SystemTime::now(),
                    intent: touch.intent.clone(),
                    summary: touch.summary.clone(),
                    detail: touch.detail.clone(),
                },
            )
            .await;

        maybe_queue_scope_signal(
            &touch.path,
            &touch.session_id,
            &self.swarm_members,
            &self.swarm_coordinators,
            &self.sessions,
            &self.soft_interrupt_queues,
            &mut self.sent,
        )
        .await;
    }
}

fn canonical_touch_path(path: &Path) -> Option<PathBuf> {
    match std::fs::canonicalize(path) {
        Ok(path) => Some(path),
        Err(_) => {
            let parent = path.parent()?;
            let file_name = path.file_name()?;
            match std::fs::canonicalize(parent) {
                Ok(parent) => Some(parent.join(file_name)),
                Err(err) => {
                    crate::logging::warn(&format!(
                        "Could not canonicalize file-touch path {}: {}",
                        path.display(),
                        err
                    ));
                    None
                }
            }
        }
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "scope signal needs the touch, swarm ownership, coordinator, and delivery state"
)]
pub(super) async fn maybe_queue_scope_signal(
    path: &Path,
    session_id: &str,
    swarm_members: &Arc<RwLock<HashMap<String, SwarmMember>>>,
    swarm_coordinators: &Arc<RwLock<HashMap<String, String>>>,
    sessions: &SessionAgents,
    soft_interrupt_queues: &SessionInterruptQueues,
    sent_signals: &mut HashSet<ScopeSignalKey>,
) {
    let Some((member_name, member_root, swarm_id)) = ({
        let members = swarm_members.read().await;
        members.get(session_id).and_then(|member| {
            Some((
                member
                    .friendly_name
                    .clone()
                    .unwrap_or_else(|| member.session_id.clone()),
                member.working_dir.clone()?,
                member.swarm_id.clone()?,
            ))
        })
    }) else {
        return;
    };

    let canonical_root = match std::fs::canonicalize(&member_root) {
        Ok(root) => root,
        Err(err) => {
            crate::logging::warn(&format!(
                "Could not canonicalize swarm member working directory {}: {}",
                member_root.display(),
                err
            ));
            return;
        }
    };
    let Some(canonical_path) = canonical_touch_path(path) else {
        return;
    };
    if canonical_path.starts_with(&canonical_root) {
        return;
    }

    let signal_key = (swarm_id.clone(), session_id.to_string(), canonical_root);
    if sent_signals.contains(&signal_key) {
        return;
    }

    let coordinator_id = {
        let coordinators = swarm_coordinators.read().await;
        coordinators.get(&swarm_id).cloned()
    };
    let Some(coordinator_id) = coordinator_id else {
        return;
    };
    let message = format!(
        "⚠ scope signal: {} touched files outside its working directory ({}), first: {}",
        member_name,
        member_root.display(),
        path.display()
    );
    if queue_soft_interrupt_for_session(
        &coordinator_id,
        message,
        false,
        SoftInterruptSource::System,
        soft_interrupt_queues,
        sessions,
    )
    .await
    {
        sent_signals.insert(signal_key);
    } else {
        crate::logging::warn(&format!(
            "Could not queue scope signal for coordinator {}",
            coordinator_id
        ));
    }
}

#[cfg(test)]
#[path = "file_scope_signal_tests.rs"]
mod tests;
