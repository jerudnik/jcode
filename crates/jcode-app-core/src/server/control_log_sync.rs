//! W1 step 3: dual-write bridge between the server's in-memory swarm maps and
//! the per-swarm control-plane event log (`jcode_swarm_core::control_log`).
//!
//! Design: rather than instrumenting every mutation site individually, the
//! sync piggybacks on the one funnel every mutation path already goes through
//! — `persist_swarm_state_for` — and appends the *delta* between the log's
//! fold and the current in-memory view (`diff_events`). This guarantees the
//! core W1 invariant at every persistence point:
//!
//!     fold(control log) == in-memory member/task control views
//!
//! by construction, for current AND future mutation paths (a new handler that
//! persists is automatically covered; one that doesn't persist is already a
//! durability bug today).
//!
//! The log file lives next to the snapshot: `jcode-swarm-state/<id>.control.jsonl`
//! under `storage::runtime_dir()`. Handles are cached per path (not per swarm
//! id) so tests that switch `JCODE_RUNTIME_DIR` never cross-contaminate.

use super::swarm_persistence::control_log_path;
use super::{SwarmMember, VersionedPlan};
use jcode_swarm_core::control_log::{
    ControlLogWriter, LOCAL_ORIGIN, MemberControlState, SwarmControlEvent, SwarmControlState,
    TaskControlState, diff_events, replay,
};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex as StdMutex};

struct ControlLogHandle {
    writer: ControlLogWriter,
    /// Fold of everything appended so far; kept in lockstep with the file so
    /// the diff never re-reads the log on the hot path.
    fold: SwarmControlState,
}

/// Per-path handle cache. Keyed by the log file path (not the swarm id):
/// tests repoint `JCODE_RUNTIME_DIR` per test, and a swarm-id key would leak
/// a stale writer across runtime dirs.
static CONTROL_LOGS: LazyLock<StdMutex<HashMap<PathBuf, ControlLogHandle>>> =
    LazyLock::new(|| StdMutex::new(HashMap::new()));

/// Project the server's in-memory member records into the fold's member view.
fn target_member_view(members: &[SwarmMember]) -> HashMap<String, MemberControlState> {
    members
        .iter()
        .map(|member| {
            (
                member.session_id.clone(),
                MemberControlState {
                    role: member.role.clone(),
                    status: member.status.clone(),
                    friendly_name: member.friendly_name.clone(),
                },
            )
        })
        .collect()
}

/// Project the plan into the fold's task view. `None` plan means the swarm
/// has no plan: an empty view, which diffs to `TaskRemoved` for anything the
/// fold still carries.
fn target_task_view(plan: Option<&VersionedPlan>) -> HashMap<String, TaskControlState> {
    let Some(plan) = plan else {
        return HashMap::new();
    };
    plan.items
        .iter()
        .map(|item| {
            (
                item.id.clone(),
                TaskControlState {
                    assigned_to: item.assigned_to.clone(),
                    status: item.status.clone(),
                    last_heartbeat_ms: plan
                        .task_progress
                        .get(&item.id)
                        .and_then(|progress| progress.last_heartbeat_unix_ms),
                    // Artifact evidence is appended explicitly by
                    // complete_node (append_control_event), never derived
                    // from a state diff: the plan does not carry it.
                    last_artifact: None,
                },
            )
        })
        .collect()
}

/// Append whatever events are needed to bring `fold(log)` up to the current
/// in-memory view. Called from `persist_swarm_state_for` (the mutation funnel)
/// with the same loaded runtime the snapshot is written from.
///
/// Returns the log's resume offset after the sync (the compaction checkpoint
/// cursor recorded in the snapshot by W1 step 4). IO failures are logged and
/// swallowed like snapshot failures: the control log must never take down a
/// mutation path it observes.
pub(super) fn sync_swarm_control_log(
    swarm_id: &str,
    members: &[SwarmMember],
    plan: Option<&VersionedPlan>,
) -> Option<u64> {
    sync_control_log_inner(swarm_id, target_member_view(members), Some(target_task_view(plan)))
}

/// Member-only sync: bring the fold's member view up to date without touching
/// task state. This is the hook for `broadcast_swarm_status`, the funnel every
/// membership-visible change (join/leave/status/role) flows through — several
/// of which (`update_member_status`, headless joins) do not persist a snapshot.
pub(super) fn sync_swarm_control_log_members(swarm_id: &str, members: &[SwarmMember]) -> Option<u64> {
    sync_control_log_inner(swarm_id, target_member_view(members), None)
}

fn sync_control_log_inner(
    swarm_id: &str,
    target_members: HashMap<String, MemberControlState>,
    target_tasks: Option<HashMap<String, TaskControlState>>,
) -> Option<u64> {
    let path = control_log_path(swarm_id);
    let mut logs = CONTROL_LOGS.lock().ok()?;
    let handle = open_handle(&mut logs, swarm_id, &path)?;

    for event in diff_events(&handle.fold, &target_members, target_tasks.as_ref()) {
        match handle.writer.append(event.clone()) {
            Ok(_) => handle.fold.apply(&event),
            Err(error) => {
                crate::logging::warn(&format!(
                    "control log append failed for {}: {}",
                    swarm_id, error
                ));
                return None;
            }
        }
    }
    // Offsets are byte positions; the writer appends synchronously, so the
    // current file length is the fully-covered resume offset.
    std::fs::metadata(&path).map(|meta| meta.len()).ok()
}

/// Append an explicit control event that is NOT derivable from a state diff
/// (W2: `ArtifactFiled` evidence). The event also updates the cached fold so
/// subsequent diffs do not re-derive against a stale view.
pub(super) fn append_control_event(swarm_id: &str, event: SwarmControlEvent) -> Option<u64> {
    let path = control_log_path(swarm_id);
    let mut logs = CONTROL_LOGS.lock().ok()?;
    let handle = open_handle(&mut logs, swarm_id, &path)?;
    match handle.writer.append(event.clone()) {
        Ok(_) => handle.fold.apply(&event),
        Err(error) => {
            crate::logging::warn(&format!(
                "control log append failed for {}: {}",
                swarm_id, error
            ));
            return None;
        }
    }
    std::fs::metadata(&path).map(|meta| meta.len()).ok()
}

fn open_handle<'a>(
    logs: &'a mut HashMap<PathBuf, ControlLogHandle>,
    swarm_id: &str,
    path: &PathBuf,
) -> Option<&'a mut ControlLogHandle> {
    if !logs.contains_key(path) {
        let (fold, _offset) = match replay(path) {
            Ok(replayed) => replayed,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                (SwarmControlState::default(), 0)
            }
            Err(error) => {
                crate::logging::warn(&format!(
                    "control log replay failed for {}: {}",
                    swarm_id, error
                ));
                return None;
            }
        };
        let writer = match ControlLogWriter::open(path, swarm_id, LOCAL_ORIGIN) {
            Ok(writer) => writer,
            Err(error) => {
                crate::logging::warn(&format!(
                    "control log open failed for {}: {}",
                    swarm_id, error
                ));
                return None;
            }
        };
        logs.insert(path.clone(), ControlLogHandle { writer, fold });
    }
    logs.get_mut(path)
}

/// Drop the cached handle for a swarm's log (e.g. after archival). The file
/// itself is deliberately kept: completed-swarm logs are the observation/
/// evaluation dataset per the W1 decision record.
#[cfg_attr(not(test), expect(dead_code, reason = "used by tests; wired for archival later"))]
pub(super) fn drop_control_log_handle(swarm_id: &str) {
    let path = control_log_path(swarm_id);
    if let Ok(mut logs) = CONTROL_LOGS.lock() {
        logs.remove(&path);
    }
}

/// Fold the on-disk control log for a swarm. Query surface for consumers that
/// want log-derived state (step 5 shim retirement, tests, future daemons).
/// Reads the file directly rather than the cached fold so it also observes
/// events written by other handles/processes.
pub(super) fn fold_swarm_control_log(swarm_id: &str) -> SwarmControlState {
    match replay(&control_log_path(swarm_id)) {
        Ok((state, _offset)) => state,
        Err(_) => SwarmControlState::default(),
    }
}

#[cfg(test)]
pub(super) mod test_support {
    use super::*;
    use std::collections::HashSet;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    /// Equivalence assert (W1 step 3): fold(control log) must agree with the
    /// in-memory maps' member/task control views for `swarm_id`. Call after
    /// any handler that persisted swarm state.
    pub(crate) async fn assert_control_log_matches_maps(
        swarm_id: &str,
        swarm_members: &Arc<RwLock<HashMap<String, SwarmMember>>>,
        swarm_plans: &Arc<RwLock<HashMap<String, VersionedPlan>>>,
    ) {
        let folded = fold_swarm_control_log(swarm_id);

        let expected_members: HashMap<String, MemberControlState> = {
            let members = swarm_members.read().await;
            target_member_view(
                &members
                    .values()
                    .filter(|member| member.swarm_id.as_deref() == Some(swarm_id))
                    .cloned()
                    .collect::<Vec<_>>(),
            )
        };
        assert_eq!(
            folded.members, expected_members,
            "fold(control log) member view diverged from in-memory members for {swarm_id}"
        );

        let expected_tasks: HashMap<String, TaskControlState> = {
            let plans = swarm_plans.read().await;
            target_task_view(plans.get(swarm_id))
        };
        // Heartbeats in the fold are monotonic evidence: the map view may
        // have dropped progress records the log legitimately remembers, so
        // compare assignment/status exactly and heartbeats only when the map
        // still carries one.
        let folded_task_ids: HashSet<&String> = folded.tasks.keys().collect();
        let expected_task_ids: HashSet<&String> = expected_tasks.keys().collect();
        assert_eq!(
            folded_task_ids, expected_task_ids,
            "fold(control log) task set diverged from plan for {swarm_id}"
        );
        for (task_id, expected) in &expected_tasks {
            let actual = &folded.tasks[task_id];
            assert_eq!(
                actual.assigned_to, expected.assigned_to,
                "assigned_to diverged for task {task_id} in {swarm_id}"
            );
            assert_eq!(
                actual.status, expected.status,
                "status diverged for task {task_id} in {swarm_id}"
            );
            if expected.last_heartbeat_ms.is_some() {
                assert_eq!(
                    actual.last_heartbeat_ms, expected.last_heartbeat_ms,
                    "heartbeat diverged for task {task_id} in {swarm_id}"
                );
            }
        }
    }
}
