use super::super::{
    FileTouchService, Server, SessionInterruptQueues, SwarmEvent, SwarmMember, VersionedPlan,
    register_session_interrupt_queue,
};
use crate::bus::{Bus, BusEvent, FileOp, FileTouch};
use crate::protocol::SwarmMemberRuntime;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, broadcast, mpsc};

fn member(
    session_id: &str,
    friendly_name: &str,
    role: &str,
    working_dir: Option<PathBuf>,
) -> SwarmMember {
    let (event_tx, _event_rx) = mpsc::unbounded_channel();
    SwarmMember {
        session_id: session_id.to_string(),
        event_tx,
        event_txs: HashMap::new(),
        working_dir,
        swarm_id: Some("scope-signal-test".to_string()),
        swarm_enabled: true,
        status: "ready".to_string(),
        detail: None,
        task_label: None,
        subagent_type: None,
        friendly_name: Some(friendly_name.to_string()),
        report_back_to_session_id: None,
        initial_prompt_delivered: None,
        latest_completion_report: None,
        role: role.to_string(),
        joined_at: Instant::now(),
        last_status_change: Instant::now(),
        is_headless: false,
        output_tail: None,
        todo_progress: None,
        todo_items: Vec::new(),
        runtime: SwarmMemberRuntime::default(),
    }
}

async fn publish_and_wait_for_touch(file_touch: &FileTouchService, session_id: &str, path: &Path) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        Bus::global().publish(BusEvent::FileTouch(FileTouch {
            session_id: session_id.to_string(),
            path: path.to_path_buf(),
            op: FileOp::Write,
            intent: None,
            summary: None,
            detail: None,
        }));

        if file_touch
            .reverse_snapshot()
            .await
            .get(session_id)
            .is_some_and(|paths| paths.contains(path))
        {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "bus monitor did not record {}",
            path.display()
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

#[tokio::test]
async fn out_of_scope_touch_queues_one_coordinator_signal_per_member_root() {
    let _env_lock = crate::storage::lock_test_env();
    let temp = tempfile::TempDir::new().expect("temp dir");
    let member_root = temp.path().join("member-root");
    let outside_root = temp.path().join("outside-root");
    std::fs::create_dir_all(&member_root).expect("create member root");
    std::fs::create_dir_all(&outside_root).expect("create outside root");

    let in_scope = member_root.join("inside.txt");
    let outside_first = outside_root.join("first.txt");
    let outside_second = outside_root.join("second.txt");
    std::fs::write(&in_scope, "inside").expect("write in-scope fixture");
    std::fs::write(&outside_first, "first").expect("write first outside fixture");
    std::fs::write(&outside_second, "second").expect("write second outside fixture");

    let coordinator_id = "scope-coordinator";
    let member_id = "scope-member";
    let unscoped_member_id = "scope-member-without-root";
    let swarm_members = Arc::new(RwLock::new(HashMap::from([
        (
            coordinator_id.to_string(),
            member(
                coordinator_id,
                "owl",
                "coordinator",
                Some(member_root.clone()),
            ),
        ),
        (
            member_id.to_string(),
            member(member_id, "fox", "agent", Some(member_root.clone())),
        ),
        (
            unscoped_member_id.to_string(),
            member(unscoped_member_id, "lynx", "agent", None),
        ),
    ])));
    let swarms_by_id = Arc::new(RwLock::new(HashMap::from([(
        "scope-signal-test".to_string(),
        HashSet::from([
            coordinator_id.to_string(),
            member_id.to_string(),
            unscoped_member_id.to_string(),
        ]),
    )])));
    let swarm_plans = Arc::new(RwLock::new(HashMap::<String, VersionedPlan>::new()));
    let swarm_coordinators = Arc::new(RwLock::new(HashMap::from([(
        "scope-signal-test".to_string(),
        coordinator_id.to_string(),
    )])));
    let sessions = Arc::new(RwLock::new(HashMap::new()));
    let soft_interrupt_queues: SessionInterruptQueues = Arc::new(RwLock::new(HashMap::new()));
    let coordinator_queue = Arc::new(std::sync::Mutex::new(Vec::new()));
    register_session_interrupt_queue(
        &soft_interrupt_queues,
        coordinator_id,
        Arc::clone(&coordinator_queue),
    )
    .await;

    let file_touch = FileTouchService::new();
    let event_history = Arc::new(RwLock::new(VecDeque::<SwarmEvent>::new()));
    let event_counter = Arc::new(AtomicU64::new(0));
    let (swarm_event_tx, _) = broadcast::channel(16);
    let monitor_task = tokio::spawn(Server::monitor_bus(
        file_touch.clone(),
        swarm_members,
        swarms_by_id,
        swarm_plans,
        swarm_coordinators,
        sessions,
        soft_interrupt_queues,
        event_history,
        event_counter,
        swarm_event_tx,
    ));

    publish_and_wait_for_touch(&file_touch, member_id, &in_scope).await;
    assert!(
        coordinator_queue
            .lock()
            .expect("coordinator queue")
            .is_empty(),
        "in-scope touches must not signal the coordinator"
    );

    publish_and_wait_for_touch(&file_touch, unscoped_member_id, &outside_first).await;
    assert!(
        coordinator_queue
            .lock()
            .expect("coordinator queue")
            .is_empty(),
        "members without a recorded working directory are exempt"
    );

    publish_and_wait_for_touch(&file_touch, member_id, &outside_first).await;
    let first_signal = {
        let pending = coordinator_queue.lock().expect("coordinator queue");
        assert_eq!(pending.len(), 1, "first outside touch should signal once");
        pending[0].clone()
    };
    assert_eq!(
        first_signal.content,
        format!(
            "⚠ scope signal: fox touched files outside its working directory ({}), first: {}",
            member_root.display(),
            outside_first.display()
        )
    );
    assert!(!first_signal.urgent);

    publish_and_wait_for_touch(&file_touch, member_id, &outside_second).await;
    assert_eq!(
        coordinator_queue.lock().expect("coordinator queue").len(),
        1,
        "a second outside touch for the same member root must be deduplicated"
    );

    monitor_task.abort();
}
