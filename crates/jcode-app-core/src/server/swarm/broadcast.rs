use super::*;

fn swarm_broadcast_key(
    swarm_id: &str,
    swarm_members: &Arc<RwLock<HashMap<String, SwarmMember>>>,
    swarms_by_id: &Arc<RwLock<HashMap<String, HashSet<String>>>>,
) -> String {
    format!(
        "{:p}:{:p}:{swarm_id}",
        Arc::as_ptr(swarm_members),
        Arc::as_ptr(swarms_by_id)
    )
}

async fn broadcast_swarm_status_now(
    session_ids: Vec<String>,
    swarm_members: &Arc<RwLock<HashMap<String, SwarmMember>>>,
) {
    if session_ids.is_empty() {
        return;
    }

    let members_guard = swarm_members.read().await;
    let members_list: Vec<crate::protocol::SwarmMemberStatus> = session_ids
        .iter()
        .filter_map(|sid| {
            members_guard
                .get(sid)
                .map(|m| crate::protocol::SwarmMemberStatus {
                    session_id: m.session_id.clone(),
                    friendly_name: m.friendly_name.clone(),
                    status: m.lifecycle_status().to_string(),
                    detail: m.detail.clone(),
                    task_label: m.task_label.clone(),
                    subagent_type: m.subagent_type.clone(),
                    role: Some(m.role.clone()),
                    is_headless: Some(m.is_headless),
                    live_attachments: Some(m.event_txs.len()),
                    status_age_secs: Some(status_age_secs(m.last_status_change)),
                    output_tail: m.output_tail.clone(),
                    report_back_to_session_id: m.report_back_to_session_id.clone(),
                    initial_prompt_delivered: m.initial_prompt_delivered,
                    todo_progress: m.todo_progress,
                    todo_items: m.todo_items.clone(),
                    runtime: crate::protocol::SwarmMemberRuntime {
                        model: m.runtime.model.clone(),
                        provider: m.runtime.provider.clone(),
                        auth_method: m.runtime.auth_method.clone(),
                        effort: m.runtime.effort.clone(),
                        elapsed_secs: if matches!(
                            m.lifecycle_status(),
                            "running"
                        ) {
                            Some(m.joined_at.elapsed().as_secs())
                        } else {
                            Some(m.runtime.elapsed_secs.unwrap_or(0))
                        },
                    },
                })
        })
        .collect();

    drop(members_guard);
    let event = ServerEvent::SwarmStatus {
        members: members_list,
    };
    for sid in session_ids {
        let _ = fanout_session_event(swarm_members, &sid, event.clone()).await;
    }
}

pub(in crate::server) async fn broadcast_swarm_status(
    swarm_id: &str,
    swarm_members: &Arc<RwLock<HashMap<String, SwarmMember>>>,
    swarms_by_id: &Arc<RwLock<HashMap<String, HashSet<String>>>>,
) {
    if claim_dead_pid_sweep(now_unix_ms(), swarm_dead_pid_sweep_interval()) {
        let changed = sweep_dead_pid_swarm_members(swarm_members, swarms_by_id).await;
        if !changed.is_empty() {
            log_swarm_lifecycle(
                "dead_pid_sweep",
                vec![("changed_swarms", changed.join(","))],
            );
        }
    }

    // W1 dual-write: broadcast_swarm_status is the funnel every membership
    // change (join/leave/status/role) flows through, including paths that do
    // not persist a snapshot (update_member_status, headless joins). Sync the
    // member view of the control log here so fold(log) tracks the maps.
    {
        let members: Vec<SwarmMember> = {
            let guard = swarm_members.read().await;
            guard
                .values()
                .filter(|member| member.swarm_id.as_deref() == Some(swarm_id))
                .cloned()
                .collect()
        };
        crate::server::control_log_sync::sync_swarm_control_log_members(swarm_id, &members);
    }

    let session_ids: Vec<String> = {
        let swarms = swarms_by_id.read().await;
        swarms
            .get(swarm_id)
            .map(|s| s.iter().cloned().collect())
            .unwrap_or_default()
    };
    if session_ids.is_empty() {
        return;
    }

    if session_ids.len() < swarm_status_debounce_member_threshold() {
        broadcast_swarm_status_now(session_ids, swarm_members).await;
        return;
    }

    let key = swarm_broadcast_key(swarm_id, swarm_members, swarms_by_id);
    let should_spawn = {
        let mut pending = pending_swarm_status_broadcasts()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let entry = pending.entry(key.clone()).or_default();
        if entry.scheduled {
            entry.dirty = true;
            false
        } else {
            entry.scheduled = true;
            entry.dirty = false;
            true
        }
    };

    if !should_spawn {
        return;
    }

    let swarm_id = swarm_id.to_string();
    let swarm_members = Arc::clone(swarm_members);
    let swarms_by_id = Arc::clone(swarms_by_id);
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(swarm_status_debounce_ms())).await;
            let session_ids: Vec<String> = {
                let swarms = swarms_by_id.read().await;
                swarms
                    .get(&swarm_id)
                    .map(|s| s.iter().cloned().collect())
                    .unwrap_or_default()
            };
            broadcast_swarm_status_now(session_ids, &swarm_members).await;

            let mut pending = pending_swarm_status_broadcasts()
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let Some(entry) = pending.get_mut(&key) else {
                break;
            };
            if entry.dirty {
                entry.dirty = false;
                continue;
            }
            pending.remove(&key);
            break;
        }
    });
}

/// Broadcast the authoritative swarm plan snapshot.
///
/// Plan snapshots are sent to explicit plan participants. If a plan has no
/// participants yet, fall back to all current swarm members.
pub(in crate::server) async fn broadcast_swarm_plan(
    swarm_id: &str,
    reason: Option<String>,
    swarm_plans: &Arc<RwLock<HashMap<String, VersionedPlan>>>,
    swarm_members: &Arc<RwLock<HashMap<String, SwarmMember>>>,
    swarms_by_id: &Arc<RwLock<HashMap<String, HashSet<String>>>>,
) {
    broadcast_swarm_plan_with_previous(
        swarm_id,
        reason,
        None,
        swarm_plans,
        swarm_members,
        swarms_by_id,
    )
    .await;
}

pub(in crate::server) async fn broadcast_swarm_plan_with_previous(
    swarm_id: &str,
    reason: Option<String>,
    previous_items: Option<&[PlanItem]>,
    swarm_plans: &Arc<RwLock<HashMap<String, VersionedPlan>>>,
    swarm_members: &Arc<RwLock<HashMap<String, SwarmMember>>>,
    swarms_by_id: &Arc<RwLock<HashMap<String, HashSet<String>>>>,
) {
    let (version, items, summary, mut participants): (
        u64,
        Vec<PlanItem>,
        crate::protocol::PlanGraphStatus,
        Vec<String>,
    ) = {
        let plans = swarm_plans.read().await;
        let Some(vp) = plans.get(swarm_id) else {
            return;
        };
        let newly_ready_ids = previous_items
            .map(|before| newly_ready_item_ids(before, &vp.items))
            .unwrap_or_default();
        let mut p: Vec<String> = vp.participants.iter().cloned().collect();
        p.sort();
        (
            vp.version,
            vp.items.clone(),
            crate::protocol::PlanGraphStatus::from_versioned_plan(
                swarm_id,
                vp,
                Some(3),
                newly_ready_ids,
            ),
            p,
        )
    };

    if participants.is_empty() {
        let swarms = swarms_by_id.read().await;
        participants = swarms
            .get(swarm_id)
            .map(|s| {
                let mut ids: Vec<String> = s.iter().cloned().collect();
                ids.sort();
                ids
            })
            .unwrap_or_default();
    }

    if participants.is_empty() {
        return;
    }

    let item_count = items.len();
    let reason_label = reason.clone().unwrap_or_else(|| "unspecified".to_string());
    let event = ServerEvent::SwarmPlan {
        swarm_id: swarm_id.to_string(),
        version,
        items,
        participants: participants.clone(),
        reason,
        summary: Some(summary),
    };

    let members = swarm_members.read().await;
    let participant_count = participants.len();
    let mut delivered_count = 0usize;
    for sid in participants {
        if let Some(member) = members.get(&sid)
            && member.event_tx.send(event.clone()).is_ok()
        {
            delivered_count += 1;
        }
    }
    log_swarm_lifecycle(
        "plan_broadcast",
        vec![
            ("swarm_id", swarm_id.to_string()),
            ("version", version.to_string()),
            ("item_count", item_count.to_string()),
            ("participant_count", participant_count.to_string()),
            ("delivered_count", delivered_count.to_string()),
            ("reason", reason_label),
        ],
    );
}

/// Send the current swarm plan snapshot to ONE session (subscribe/resume
/// refresh). Unlike [`broadcast_swarm_plan`] this does not fan out to all
/// participants: reconnecting clients would otherwise show no plan graph
/// until the next plan mutation happens to broadcast.
pub(in crate::server) async fn send_swarm_plan_to_session(
    session_id: &str,
    swarm_members: &Arc<RwLock<HashMap<String, SwarmMember>>>,
    swarm_plans: &Arc<RwLock<HashMap<String, VersionedPlan>>>,
) {
    let swarm_id = {
        let members = swarm_members.read().await;
        members
            .get(session_id)
            .and_then(|member| member.swarm_id.clone())
    };
    let Some(swarm_id) = swarm_id else {
        return;
    };

    let event = {
        let plans = swarm_plans.read().await;
        let Some(vp) = plans.get(&swarm_id) else {
            return;
        };
        if vp.items.is_empty() {
            return;
        }
        let mut participants: Vec<String> = vp.participants.iter().cloned().collect();
        participants.sort();
        ServerEvent::SwarmPlan {
            swarm_id: swarm_id.clone(),
            version: vp.version,
            items: vp.items.clone(),
            participants,
            reason: Some("reconnect".to_string()),
            summary: Some(crate::protocol::PlanGraphStatus::from_versioned_plan(
                &swarm_id,
                vp,
                Some(3),
                Vec::new(),
            )),
        }
    };

    let members = swarm_members.read().await;
    if let Some(member) = members.get(session_id) {
        let _ = member.event_tx.send(event);
    }
}

pub(in crate::server) async fn rename_plan_participant(
    swarm_id: &str,
    old_session_id: &str,
    new_session_id: &str,
    swarm_plans: &Arc<RwLock<HashMap<String, VersionedPlan>>>,
) {
    let mut plans = swarm_plans.write().await;
    if let Some(vp) = plans.get_mut(swarm_id) {
        if vp.participants.remove(old_session_id) {
            vp.participants.insert(new_session_id.to_string());
        }
        for item in &mut vp.items {
            if item.assigned_to.as_deref() == Some(old_session_id) {
                item.assigned_to = Some(new_session_id.to_string());
            }
        }
    }
}

pub(in crate::server) async fn remove_plan_participant(
    swarm_id: &str,
    session_id: &str,
    swarm_plans: &Arc<RwLock<HashMap<String, VersionedPlan>>>,
) {
    let mut plans = swarm_plans.write().await;
    if let Some(vp) = plans.get_mut(swarm_id) {
        vp.participants.remove(session_id);
    }
}
