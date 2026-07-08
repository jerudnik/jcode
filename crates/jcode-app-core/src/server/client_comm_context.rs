use super::debug::ClientConnectionInfo;
use super::{FileTouchService, SwarmMember};
use crate::protocol::{AgentInfo, ServerEvent};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};

async fn swarm_id_for_session(
    session_id: &str,
    swarm_members: &Arc<RwLock<HashMap<String, SwarmMember>>>,
) -> Option<String> {
    let members = swarm_members.read().await;
    members.get(session_id).and_then(|m| m.swarm_id.clone())
}

#[expect(
    clippy::too_many_arguments,
    reason = "comm list joins swarm membership, file touches, live sessions, and connection activity"
)]
pub(super) async fn handle_comm_list(
    id: u64,
    req_session_id: String,
    client_event_tx: &mpsc::UnboundedSender<ServerEvent>,
    swarm_members: &Arc<RwLock<HashMap<String, SwarmMember>>>,
    swarms_by_id: &Arc<RwLock<HashMap<String, HashSet<String>>>>,
    file_touch: &FileTouchService,
    sessions: &super::SessionAgents,
    client_connections: &Arc<RwLock<HashMap<String, ClientConnectionInfo>>>,
) {
    let swarm_id = swarm_id_for_session(&req_session_id, swarm_members).await;

    if let Some(swarm_id) = swarm_id {
        let swarm_session_ids: Vec<String> = {
            let swarms = swarms_by_id.read().await;
            swarms
                .get(&swarm_id)
                .map(|sessions| sessions.iter().cloned().collect())
                .unwrap_or_default()
        };

        // Snapshot the static member fields first, releasing the members lock
        // before gathering per-session runtime extras (which briefly lock
        // individual agents and read the connection map).
        struct MemberStatic {
            session_id: String,
            friendly_name: Option<String>,
            files: Vec<String>,
            status: String,
            detail: Option<String>,
            task_label: Option<String>,
            subagent_type: Option<String>,
            role: String,
            is_headless: bool,
            report_back_to_session_id: Option<String>,
            latest_completion_report: Option<String>,
            live_attachments: usize,
            status_age_secs: u64,
        }

        let statics: Vec<MemberStatic> = {
            let members = swarm_members.read().await;
            let touches = file_touch.reverse_snapshot().await;
            swarm_session_ids
                .iter()
                .filter_map(|sid| {
                    members.get(sid).map(|member| {
                        let mut files: Vec<String> = touches
                            .get(sid)
                            .into_iter()
                            .flat_map(|paths| paths.iter())
                            .map(|path| path.display().to_string())
                            .collect();
                        files.sort();
                        MemberStatic {
                            session_id: sid.clone(),
                            friendly_name: member.friendly_name.clone(),
                            files,
                            status: member.status.clone(),
                            detail: member.detail.clone(),
                            task_label: member.task_label.clone(),
                            subagent_type: member.subagent_type.clone(),
                            role: member.role.clone(),
                            is_headless: member.is_headless,
                            report_back_to_session_id: member.report_back_to_session_id.clone(),
                            latest_completion_report: member.latest_completion_report.clone(),
                            live_attachments: member.event_txs.len(),
                            status_age_secs: member.last_status_change.elapsed().as_secs(),
                        }
                    })
                })
                .collect()
        };

        let mut member_list: Vec<AgentInfo> = Vec::with_capacity(statics.len());
        for m in statics {
            let extras = super::comm_sync::member_runtime_extras(
                &m.session_id,
                m.status == "running",
                sessions,
                client_connections,
            )
            .await;

            member_list.push(AgentInfo {
                session_id: m.session_id,
                friendly_name: m.friendly_name,
                files_touched: m.files,
                status: Some(m.status),
                detail: m.detail,
                task_label: m.task_label,
                subagent_type: m.subagent_type,
                role: Some(m.role),
                is_headless: Some(m.is_headless),
                report_back_to_session_id: m.report_back_to_session_id,
                latest_completion_report: m.latest_completion_report,
                live_attachments: Some(m.live_attachments),
                status_age_secs: Some(m.status_age_secs),
                last_activity_age_secs: extras.last_activity_age_secs,
                activity: extras.activity,
                provider_name: extras.provider_name,
                provider_model: extras.provider_model,
                turn_count: extras.turn_count,
                recent_total_tokens: extras.recent_total_tokens,
                recent_output_tokens: extras.recent_output_tokens,
                recent_window_secs: extras.recent_window_secs,
                cumulative_total_tokens: extras.cumulative_total_tokens,
                todos_completed: extras.todos_completed,
                todos_total: extras.todos_total,
            });
        }

        let _ = client_event_tx.send(ServerEvent::CommMembers {
            id,
            members: member_list,
        });
    } else {
        let _ = client_event_tx.send(ServerEvent::Error {
            id,
            message: "Not in a swarm. Use a git repository to enable swarm features.".to_string(),
            retry_after_secs: None,
        });
    }
}
