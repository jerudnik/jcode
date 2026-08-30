#![cfg_attr(test, allow(clippy::await_holding_lock))]

use super::{
    CoordinatorSpawnIdentity, SwarmSpawnCreation, SwarmSpawnSelection, auto_fallback_member_label,
    ensure_spawn_coordinator_swarm, explicit_route_for_configured_model, handle_comm_spawn,
    prepare_visible_spawn_session, provider_key_for_spawn_model, register_visible_spawned_member,
    resolve_coordinator_spawn_identity, resolve_spawn_working_dir, resolve_stop_target_session,
    resolve_swarm_spawn_creation, resolve_swarm_spawn_selection, swarm_stop_allowed_by_owner,
    validate_concrete_spawn_selection,
};
use crate::agent::Agent;
use crate::config::SwarmSpawnMode;
use crate::message::{Message, ToolDefinition};
use crate::plan::{NodeMeta, PlanItem};
use crate::protocol::{NotificationType, ServerEvent};
use crate::provider::ModelRoute;
use crate::provider::{EventStream, Provider};
use crate::server::swarm_mutation_state::SwarmMutationRuntime;
use crate::server::{SwarmEventType, SwarmMember, VersionedPlan};
use crate::tool::Registry;
use anyhow::Result;
use async_trait::async_trait;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::time::Instant;
use tokio::sync::{Mutex, RwLock, broadcast, mpsc};

use super::handle_comm_list_swarms;

#[path = "comm_session_tests/headless_spawn.rs"]
mod headless_spawn;

struct MockProvider;

#[async_trait]
impl Provider for MockProvider {
    async fn complete(
        &self,
        _messages: &[Message],
        _tools: &[ToolDefinition],
        _system: &str,
        _resume_session_id: Option<&str>,
    ) -> Result<EventStream> {
        Err(anyhow::anyhow!("mock provider should not be called"))
    }

    fn name(&self) -> &str {
        "mock"
    }

    fn fork(&self) -> Arc<dyn Provider> {
        Arc::new(MockProvider)
    }
}

fn member(
    session_id: &str,
    swarm_id: Option<&str>,
    role: &str,
) -> (SwarmMember, mpsc::UnboundedReceiver<ServerEvent>) {
    let (event_tx, event_rx) = mpsc::unbounded_channel();
    (
        SwarmMember {
            session_id: session_id.to_string(),
            event_tx,
            event_txs: HashMap::new(),
            working_dir: None,
            swarm_id: swarm_id.map(|id| id.to_string()),
            swarm_enabled: true,
            status: "ready".to_string(),
            detail: None,
            friendly_name: Some(session_id.to_string()),
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
            runtime: crate::protocol::SwarmMemberRuntime::default(),
            task_label: None,
            subagent_type: None,
        },
        event_rx,
    )
}

#[test]
fn spawn_provider_key_uses_resolver_for_explicit_catalog_and_native_models() {
    assert_eq!(
        provider_key_for_spawn_model(Some("openai-api:gpt-5.5"), None).as_deref(),
        Some("openai")
    );
    assert_eq!(
        provider_key_for_spawn_model(Some("anthropic-api:claude-sonnet-4-6"), None).as_deref(),
        Some("anthropic-api")
    );
    assert_eq!(
        provider_key_for_spawn_model(Some("composer-2-fast"), None).as_deref(),
        Some("cursor")
    );
    assert_eq!(
        provider_key_for_spawn_model(Some("openai-api:gpt-5.5"), Some("override")).as_deref(),
        Some("override")
    );
}

#[test]
fn configured_explicit_route_uses_single_resolver_result() {
    let selection =
        explicit_route_for_configured_model("openai-api:gpt-5.5").expect("explicit route");
    assert_eq!(selection.model.as_deref(), Some("gpt-5.5"));
    assert_eq!(selection.provider_key.as_deref(), Some("openai-api-key"));
    assert_eq!(
        selection.route_api_method.as_deref(),
        Some("openai-api-key")
    );

    assert!(explicit_route_for_configured_model("anthropic-api:claude-sonnet-4-6").is_none());
    assert!(explicit_route_for_configured_model("anthropic:claude-sonnet-4-6").is_none());
    assert!(explicit_route_for_configured_model("openai:gpt-5.5").is_none());
}

fn plan_item(id: &str, status: &str, priority: &str, assigned_to: Option<&str>) -> PlanItem {
    PlanItem {
        content: format!("task {id}"),
        status: status.to_string(),
        priority: priority.to_string(),
        id: id.to_string(),
        subsystem: None,
        file_scope: Vec::new(),
        blocked_by: Vec::new(),
        assigned_to: assigned_to.map(ToString::to_string),
    }
}

async fn test_agent_with_working_dir(session_id: &str, working_dir: &str) -> Arc<Mutex<Agent>> {
    let provider: Arc<dyn Provider> = Arc::new(MockProvider);
    let registry = Registry::new(provider.clone()).await;
    let mut session = crate::session::Session::create_with_id(session_id.to_string(), None, None);
    session.model = Some("mock".to_string());
    session.working_dir = Some(working_dir.to_string());
    let mut agent = Agent::new_with_session(provider, registry, session, None);
    agent.set_working_dir(working_dir);
    Arc::new(Mutex::new(agent))
}

#[tokio::test]
async fn comm_list_swarms_returns_live_fleet_rollup() {
    let swarm_id = "comm-list-swarms-rollup-test".to_string();
    let sessions = Arc::new(RwLock::new(HashMap::new()));
    let (mut coord, _coord_rx) = member("rollup-coord", Some(&swarm_id), "coordinator");
    coord.friendly_name = Some("falcon".to_string());
    let (mut worker, _worker_rx) = member("rollup-worker", Some(&swarm_id), "agent");
    worker.status = "running".to_string();
    worker.subagent_type = Some("implement".to_string());

    let swarm_members = Arc::new(RwLock::new(HashMap::from([
        ("rollup-coord".to_string(), coord),
        ("rollup-worker".to_string(), worker),
    ])));
    let swarms_by_id = Arc::new(RwLock::new(HashMap::from([(
        swarm_id.clone(),
        HashSet::from(["rollup-coord".to_string(), "rollup-worker".to_string()]),
    )])));
    let swarm_coordinators = Arc::new(RwLock::new(HashMap::from([(
        swarm_id.clone(),
        "rollup-coord".to_string(),
    )])));
    let swarm_plans = Arc::new(RwLock::new(HashMap::from([(
        swarm_id.clone(),
        VersionedPlan {
            items: vec![plan_item("task-verify", "running", "high", Some("rollup-worker"))],
            version: 7,
            participants: HashSet::from(["rollup-coord".to_string(), "rollup-worker".to_string()]),
            task_progress: HashMap::new(),
            mode: "deep".to_string(),
            node_meta: HashMap::from([(
                "task-verify".to_string(),
                NodeMeta {
                    kind: Some("verify".to_string()),
                    ..NodeMeta::default()
                },
            )]),
            max_nodes: None,
            frozen: false,
            safety_ledger: None,
        },
    )])));

    let (client_event_tx, mut client_event_rx) = mpsc::unbounded_channel();
    handle_comm_list_swarms(
        42,
        &sessions,
        &swarm_members,
        &swarms_by_id,
        &swarm_coordinators,
        &swarm_plans,
        |event| {
            let _ = client_event_tx.send(event);
        },
    )
    .await;

    match client_event_rx.recv().await.expect("fleet response") {
        ServerEvent::CommListSwarmsResponse { id, swarms } => {
            assert_eq!(id, 42);
            assert_eq!(swarms.len(), 1);
            let entry = &swarms[0];
            assert_eq!(entry.swarm_id, swarm_id);
            assert_eq!(entry.coordinator_session_id.as_deref(), Some("rollup-coord"));
            assert_eq!(entry.coordinator_name.as_deref(), Some("falcon"));
            assert_eq!(entry.coordinator_status.as_deref(), Some("ready"));
            assert_eq!(entry.member_count, 2);
            let worker = entry
                .members
                .iter()
                .find(|member| member.session_id == "rollup-worker")
                .expect("worker member");
            assert_eq!(worker.status, "running");
            assert_eq!(worker.subagent_type.as_deref(), Some("implement"));
            assert_eq!(worker.assigned_instance_id.as_deref(), Some("task-verify"));
            assert_eq!(entry.members_by_status.get("ready"), Some(&1));
            assert_eq!(entry.members_by_status.get("running"), Some(&1));
            assert_eq!(entry.members_by_type.get("verify"), Some(&1));
            assert_eq!(entry.members_by_type.get("untyped"), Some(&1));
            assert_eq!(entry.plan.version, 7);
            assert_eq!(entry.plan.mode, "deep");
            assert_eq!(entry.plan.active_ids, vec!["task-verify".to_string()]);
            assert_eq!(entry.tokens, None);
            assert!(entry.last_activity_age_secs.is_some());
            assert!(entry.control_log_offset.is_some());
        }
        other => panic!("unexpected response: {other:?}"),
    }
}

#[tokio::test]
async fn resolve_spawn_working_dir_prefers_explicit_then_spawner_agent_dir() {
    let sessions = Arc::new(RwLock::new(HashMap::new()));
    sessions.write().await.insert(
        "req".to_string(),
        test_agent_with_working_dir("req", "/tmp/spawner-agent").await,
    );
    let swarm_members = Arc::new(RwLock::new(HashMap::new()));

    assert_eq!(
        resolve_spawn_working_dir(
            Some("/tmp/explicit".to_string()),
            "req",
            &sessions,
            &swarm_members,
        )
        .await
        .as_deref(),
        Some("/tmp/explicit")
    );
    assert_eq!(
        resolve_spawn_working_dir(None, "req", &sessions, &swarm_members)
            .await
            .as_deref(),
        Some("/tmp/spawner-agent")
    );
}

#[tokio::test]
async fn resolve_spawn_working_dir_falls_back_to_member_dir() {
    let sessions = Arc::new(RwLock::new(HashMap::new()));
    let swarm_members = Arc::new(RwLock::new(HashMap::new()));
    let (mut req_member, _rx) = member("req", Some("swarm-1"), "coordinator");
    req_member.working_dir = Some(std::path::PathBuf::from("/tmp/member-dir"));
    swarm_members
        .write()
        .await
        .insert("req".to_string(), req_member);

    assert_eq!(
        resolve_spawn_working_dir(None, "req", &sessions, &swarm_members)
            .await
            .as_deref(),
        Some("/tmp/member-dir")
    );
}

#[test]
fn stop_permission_defaults_to_sessions_spawned_by_requesting_coordinator() {
    let (mut owned, _owned_rx) = member("worker-owned", Some("swarm-1"), "agent");
    owned.report_back_to_session_id = Some("coord".to_string());
    let (mut user_created, _user_rx) = member("worker-user", Some("swarm-1"), "agent");
    user_created.report_back_to_session_id = None;
    let (mut other_owned, _other_rx) = member("worker-other", Some("swarm-1"), "agent");
    other_owned.report_back_to_session_id = Some("other-coord".to_string());

    assert!(swarm_stop_allowed_by_owner("coord", &owned, false));
    assert!(!swarm_stop_allowed_by_owner("coord", &user_created, false));
    assert!(!swarm_stop_allowed_by_owner("coord", &other_owned, false));
    assert!(swarm_stop_allowed_by_owner("coord", &user_created, true));
}

#[tokio::test]
async fn stop_target_resolves_unique_friendly_name_and_suffix() {
    let swarm_members = Arc::new(RwLock::new(HashMap::new()));
    let (mut worker, _worker_rx) = member("session_jellyfish_1234_abcd", Some("swarm-1"), "agent");
    worker.friendly_name = Some("jellyfish".to_string());
    swarm_members
        .write()
        .await
        .insert(worker.session_id.clone(), worker);

    assert_eq!(
        resolve_stop_target_session("coord", "swarm-1", "jellyfish", false, &swarm_members)
            .await
            .as_deref(),
        Ok("session_jellyfish_1234_abcd")
    );
    assert_eq!(
        resolve_stop_target_session("coord", "swarm-1", "abcd", false, &swarm_members)
            .await
            .as_deref(),
        Ok("session_jellyfish_1234_abcd")
    );
}

#[tokio::test]
async fn stop_target_rejects_ambiguous_friendly_name() {
    let swarm_members = Arc::new(RwLock::new(HashMap::new()));
    let (mut first, _first_rx) = member("session_bear_1", Some("swarm-1"), "agent");
    first.friendly_name = Some("bear".to_string());
    let (mut second, _second_rx) = member("session_bear_2", Some("swarm-1"), "agent");
    second.friendly_name = Some("bear".to_string());
    let mut members = swarm_members.write().await;
    members.insert(first.session_id.clone(), first);
    members.insert(second.session_id.clone(), second);
    drop(members);

    let err = resolve_stop_target_session("coord", "swarm-1", "bear", false, &swarm_members)
        .await
        .expect_err("ambiguous friendly names should be rejected");
    assert!(err.contains("Ambiguous swarm session 'bear'"));
}

#[tokio::test]
async fn stop_target_resolves_owned_child_in_different_swarm() {
    let swarm_members = Arc::new(RwLock::new(HashMap::new()));
    let (coord, _coord_rx) = member("coord", Some("swarm-1"), "coordinator");
    let (mut child, _child_rx) = member("child-2", Some("swarm-2"), "agent");
    child.report_back_to_session_id = Some("coord".to_string());
    child.friendly_name = Some("otter".to_string());
    let mut members = swarm_members.write().await;
    members.insert("coord".to_string(), coord);
    members.insert("child-2".to_string(), child);
    drop(members);

    assert_eq!(
        resolve_stop_target_session("coord", "swarm-1", "otter", false, &swarm_members)
            .await
            .expect("owned child should resolve"),
        "child-2"
    );
}

#[tokio::test]
async fn stop_target_requires_cross_swarm_for_unrelated_swarm_member_resolution() {
    let swarm_members = Arc::new(RwLock::new(HashMap::new()));
    let (coord, _coord_rx) = member("coord", Some("swarm-1"), "coordinator");
    let (mut foreign, _foreign_rx) = member("foreign-2", Some("swarm-2"), "agent");
    foreign.report_back_to_session_id = Some("other-coord".to_string());
    foreign.friendly_name = Some("foreign-worker".to_string());
    let mut members = swarm_members.write().await;
    members.insert("coord".to_string(), coord);
    members.insert("foreign-2".to_string(), foreign);
    drop(members);

    let err =
        resolve_stop_target_session("coord", "swarm-1", "foreign-worker", false, &swarm_members)
            .await
            .expect_err("unrelated swarm member should not resolve without opt-in");
    assert!(err.contains("Unknown swarm session 'foreign-worker'"));

    let err = resolve_stop_target_session("coord", "swarm-1", "foreign-2", false, &swarm_members)
        .await
        .expect_err("exact unrelated swarm member should not resolve without opt-in");
    assert!(err.contains("Unknown swarm session 'foreign-2'"));

    assert_eq!(
        resolve_stop_target_session("coord", "swarm-1", "foreign-2", true, &swarm_members)
            .await
            .expect("exact cross-swarm target should resolve"),
        "foreign-2"
    );
    assert_eq!(
        resolve_stop_target_session("coord", "swarm-1", "foreign-worker", true, &swarm_members,)
            .await
            .expect("cross-swarm opt-in should resolve unrelated member"),
        "foreign-2"
    );

    let members = swarm_members.read().await;
    let target = members.get("foreign-2").expect("foreign member");
    assert!(!swarm_stop_allowed_by_owner("coord", target, false));
    assert!(swarm_stop_allowed_by_owner("coord", target, true));
}

#[tokio::test]
async fn register_visible_spawned_member_marks_startup_as_running() {
    let swarm_members = Arc::new(RwLock::new(HashMap::new()));
    let swarms_by_id = Arc::new(RwLock::new(HashMap::new()));
    let event_history = Arc::new(RwLock::new(VecDeque::new()));
    let event_counter = Arc::new(AtomicU64::new(0));
    let (swarm_event_tx, _swarm_event_rx) = broadcast::channel(8);

    register_visible_spawned_member(
        "child-1",
        "swarm-1",
        Some("/tmp/worktree"),
        true,
        Some("owner"),
        &swarm_members,
        &swarms_by_id,
        &event_history,
        &event_counter,
        &swarm_event_tx,
    )
    .await;

    let members = swarm_members.read().await;
    let member = members.get("child-1").expect("spawned member should exist");
    assert_eq!(member.status, "running");
    assert_eq!(member.detail.as_deref(), Some("startup queued"));
    assert_eq!(member.swarm_id.as_deref(), Some("swarm-1"));
    assert_eq!(
        member.working_dir.as_deref(),
        Some(std::path::Path::new("/tmp/worktree"))
    );
    drop(members);

    assert!(
        swarms_by_id
            .read()
            .await
            .get("swarm-1")
            .is_some_and(|members| members.contains("child-1"))
    );

    let history = event_history.read().await;
    assert!(history.iter().any(|event| {
            event.session_id == "child-1"
                && matches!(event.event, SwarmEventType::MemberChange { ref action } if action == "joined")
        }));
}

#[test]
fn prepare_visible_spawn_session_persists_startup_before_launch() {
    let _guard = crate::storage::lock_test_env();
    let temp_home = tempfile::TempDir::new().expect("temp home");
    crate::env::set_var("JCODE_HOME", temp_home.path());

    let worktree = tempfile::TempDir::new().expect("temp worktree");
    let startup = "Please start by auditing prompt delivery.";

    let (session_id, launched) = prepare_visible_spawn_session(
        Some(worktree.path().to_str().expect("utf8 worktree path")),
        None,
        None,
        None,
        None,
        false,
        Some(startup),
        |session_id, _cwd: &std::path::Path, _selfdev, provider_key| {
            assert_eq!(provider_key, None);
            let path = crate::storage::jcode_dir()
                .expect("jcode dir")
                .join(format!("client-input-{}", session_id));
            let data = std::fs::read_to_string(&path).expect("startup file should exist");
            assert!(
                data.contains(startup),
                "startup payload should be written before launch"
            );
            assert!(
                data.contains(r#""submit_on_restore":true"#),
                "startup payload should auto-submit on restore"
            );
            Ok(true)
        },
    )
    .expect("visible spawn preparation should succeed");

    assert!(launched);
    let path = crate::storage::jcode_dir()
        .expect("jcode dir")
        .join(format!("client-input-{}", session_id));
    assert!(
        path.exists(),
        "startup file should remain for launched visible session"
    );

    crate::env::remove_var("JCODE_HOME");
}

#[test]
fn prepare_visible_spawn_session_cleans_startup_when_launch_not_started() {
    let _guard = crate::storage::lock_test_env();
    let temp_home = tempfile::TempDir::new().expect("temp home");
    crate::env::set_var("JCODE_HOME", temp_home.path());

    let worktree = tempfile::TempDir::new().expect("temp worktree");

    let (session_id, launched) = prepare_visible_spawn_session(
        Some(worktree.path().to_str().expect("utf8 worktree path")),
        None,
        None,
        None,
        None,
        false,
        Some("Do the thing."),
        |_session_id, _cwd: &std::path::Path, _selfdev, _provider_key| Ok(false),
    )
    .expect("visible spawn preparation should succeed even when launch is skipped");

    assert!(!launched);
    let path = crate::storage::jcode_dir()
        .expect("jcode dir")
        .join(format!("client-input-{}", session_id));
    assert!(
        !path.exists(),
        "startup file should be removed when visible launch does not start"
    );
    assert!(
        !crate::session::session_exists(&session_id),
        "prepared session should be cleaned up when visible launch does not start"
    );

    crate::env::remove_var("JCODE_HOME");
}

#[test]
fn prepare_visible_spawn_session_cleans_session_when_launch_errors() {
    let _guard = crate::storage::lock_test_env();
    let temp_home = tempfile::TempDir::new().expect("temp home");
    crate::env::set_var("JCODE_HOME", temp_home.path());

    let worktree = tempfile::TempDir::new().expect("temp worktree");

    let error = prepare_visible_spawn_session(
        Some(worktree.path().to_str().expect("utf8 worktree path")),
        None,
        None,
        None,
        None,
        false,
        Some("Do the thing."),
        |_session_id, _cwd: &std::path::Path, _selfdev, _provider_key| {
            Err(anyhow::anyhow!("launch failed"))
        },
    )
    .expect_err("visible spawn preparation should surface launch error");

    assert!(error.to_string().contains("launch failed"));
    let sessions_dir = crate::storage::jcode_dir()
        .expect("jcode dir")
        .join("sessions");
    let mut remaining_sessions = match std::fs::read_dir(&sessions_dir) {
        Ok(entries) => entries
            .map(|entry| {
                entry
                    .expect("read prepared session directory entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect::<Vec<_>>(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(error) => panic!("read prepared sessions directory: {error}"),
    };
    remaining_sessions.sort();
    assert_eq!(
        remaining_sessions,
        Vec::<String>::new(),
        "failed visible launch should not leave orphan prepared sessions: {remaining_sessions:?}"
    );

    crate::env::remove_var("JCODE_HOME");
}

#[test]
fn prepare_visible_spawn_session_no_longer_guesses_openrouter_from_model_shape() {
    // `model@Provider` / `vendor/model` shapes used to persist
    // provider_key=openrouter purely from their spelling; the passthrough is
    // retired, so an uncataloged shape persists no provider key at all.
    let _guard = crate::storage::lock_test_env();
    let temp_home = tempfile::TempDir::new().expect("temp home");
    crate::env::set_var("JCODE_HOME", temp_home.path());

    let worktree = tempfile::TempDir::new().expect("temp worktree");
    let (session_id, launched) = prepare_visible_spawn_session(
        Some(worktree.path().to_str().expect("utf8 worktree path")),
        Some("openai/gpt-5.4@OpenAI"),
        None,
        None,
        None,
        false,
        None,
        |_session_id, _cwd: &std::path::Path, _selfdev, provider_key| {
            assert_eq!(provider_key, None);
            Ok(true)
        },
    )
    .expect("visible spawn preparation should succeed");

    assert!(launched);
    let session = crate::session::Session::load(&session_id).expect("prepared session should save");
    assert_eq!(session.model.as_deref(), Some("openai/gpt-5.4@OpenAI"));
    assert_eq!(session.provider_key, None);

    crate::env::remove_var("JCODE_HOME");
}

#[test]
fn prepare_visible_spawn_session_persists_requested_effort() {
    let _guard = crate::storage::lock_test_env();
    let temp_home = tempfile::TempDir::new().expect("temp home");
    crate::env::set_var("JCODE_HOME", temp_home.path());

    let worktree = tempfile::TempDir::new().expect("temp worktree");
    let (session_id, launched) = prepare_visible_spawn_session(
        Some(worktree.path().to_str().expect("utf8 worktree path")),
        Some("gpt-5.5"),
        None,
        None,
        Some("low"),
        false,
        None,
        |_session_id, _cwd: &std::path::Path, _selfdev, _provider_key| Ok(true),
    )
    .expect("visible spawn preparation should succeed");

    assert!(launched);
    let session = crate::session::Session::load(&session_id).expect("prepared session should save");
    assert_eq!(session.model.as_deref(), Some("gpt-5.5"));
    assert_eq!(
        session.reasoning_effort.as_deref(),
        Some("low"),
        "requested effort should persist so the headed client restores it"
    );

    crate::env::remove_var("JCODE_HOME");
}

#[test]
fn prepare_visible_spawn_session_prefers_parent_provider_key_over_model_guess() {
    let _guard = crate::storage::lock_test_env();
    let temp_home = tempfile::TempDir::new().expect("temp home");
    crate::env::set_var("JCODE_HOME", temp_home.path());

    let worktree = tempfile::TempDir::new().expect("temp worktree");
    let (session_id, launched) = prepare_visible_spawn_session(
        Some(worktree.path().to_str().expect("utf8 worktree path")),
        Some("gpt-5.4"),
        Some("ollama"),
        None,
        None,
        false,
        None,
        |_session_id, _cwd: &std::path::Path, _selfdev, provider_key| {
            assert_eq!(provider_key, Some("ollama"));
            Ok(true)
        },
    )
    .expect("visible spawn preparation should succeed");

    assert!(launched);
    let session = crate::session::Session::load(&session_id).expect("prepared session should save");
    assert_eq!(session.model.as_deref(), Some("gpt-5.4"));
    assert_eq!(session.provider_key.as_deref(), Some("ollama"));

    crate::env::remove_var("JCODE_HOME");
}

#[test]
fn explicit_visible_launch_false_fails_closed_without_headless_fallback() {
    let error = resolve_swarm_spawn_creation(
        SwarmSpawnMode::Visible,
        Some(Ok(("prepared-visible-session".to_string(), false))),
    )
    .expect_err("explicit visible should fail closed when no client launched");

    let message = error.to_string();
    assert!(message.contains("visible swarm spawn requested"));
    assert!(message.contains("prepared-visible-session"));
}

#[test]
fn explicit_visible_launch_error_fails_closed_without_headless_fallback() {
    let error = resolve_swarm_spawn_creation(
        SwarmSpawnMode::Visible,
        Some(Err(anyhow::anyhow!("terminal unavailable"))),
    )
    .expect_err("explicit visible should surface launch errors");

    let message = error.to_string();
    assert!(message.contains("visible swarm spawn failed"));
    assert!(message.contains("terminal unavailable"));
}

#[test]
fn auto_visible_failure_allows_labeled_headless_fallback() {
    let creation = resolve_swarm_spawn_creation(
        SwarmSpawnMode::Auto,
        Some(Err(anyhow::anyhow!("terminal unavailable"))),
    )
    .expect("auto may fallback after preserving visible failure");

    let SwarmSpawnCreation::Headless {
        fallback_detail: Some(fallback_detail),
    } = creation
    else {
        panic!("auto visible failure should choose labeled headless fallback");
    };
    assert!(fallback_detail.contains("auto fallback"));
    assert!(fallback_detail.contains("requested Auto -> resolved Headless"));
    assert!(fallback_detail.contains("terminal unavailable"));

    let fallback_label = auto_fallback_member_label(Some("verify worker"), &fallback_detail);
    assert!(fallback_label.starts_with("verify worker"));
    assert!(fallback_label.contains("terminal unavailable"));
}

fn coordinator_identity(
    model: Option<&str>,
    provider_key: Option<&str>,
    route_api_method: Option<&str>,
) -> CoordinatorSpawnIdentity {
    CoordinatorSpawnIdentity {
        model: model.map(str::to_string),
        provider_key: provider_key.map(str::to_string),
        route_api_method: route_api_method.map(str::to_string),
        is_canary: false,
    }
}

fn resolved_spawn_selection(
    requested_model: Option<String>,
    configured_swarm_model: Option<String>,
    coordinator: &CoordinatorSpawnIdentity,
) -> SwarmSpawnSelection {
    resolve_swarm_spawn_selection(
        requested_model,
        configured_swarm_model,
        coordinator,
        &[],
        &[],
    )
    .expect("spawn model should resolve")
}

fn catalog_route(model: &str, provider: &str, api_method: &str, available: bool) -> ModelRoute {
    ModelRoute {
        model: model.to_string(),
        provider: provider.to_string(),
        api_method: api_method.to_string(),
        available,
        detail: if available {
            String::new()
        } else {
            "requires extra usage".to_string()
        },
        cheapness: None,
    }
}

#[test]
fn resolve_swarm_spawn_model_accepts_any_listed_catalog_model() {
    // Every name `swarm list_models` prints must spawn, and must land on the
    // route it was listed under (the list/resolve asymmetry incident).
    let routes = [
        catalog_route("k3", "Kimi Code", "openai-compatible:kimi", true),
        catalog_route(
            "grok-4.5",
            "Grok Direct",
            "openai-compatible:grok-direct",
            true,
        ),
        catalog_route(
            "bridge/gemini-3-flash-agent",
            "OpenAI-compatible",
            "openai-compatible:openai-compatible",
            true,
        ),
    ];
    let coordinator = coordinator_identity(
        Some("claude-opus-4-8"),
        Some("claude-oauth"),
        Some("claude-oauth"),
    );

    for route in &routes {
        let selection = resolve_swarm_spawn_selection(
            Some(route.model.clone()),
            None,
            &coordinator,
            &routes,
            &[],
        )
        .expect("listed model must resolve");
        assert_eq!(selection.model.as_deref(), Some(route.model.as_str()));
        assert_eq!(
            selection.route_api_method.as_deref(),
            Some(route.api_method.as_str()),
            "listed model must spawn on its listed route"
        );
    }
}

#[test]
fn resolve_swarm_spawn_model_reports_unavailable_catalog_routes() {
    let routes = [catalog_route(
        "claude-opus-4-6[1m]",
        "Anthropic",
        "claude-oauth",
        false,
    )];
    let error = resolve_swarm_spawn_selection(
        Some("claude-opus-4-6[1m]".to_string()),
        None,
        &coordinator_identity(None, None, None),
        &routes,
        &[],
    )
    .expect_err("unavailable-only catalog model must fail");
    let message = error.to_string();
    assert!(message.contains("currently unavailable"));
    assert!(message.contains("requires extra usage"));
}

#[test]
fn resolve_swarm_spawn_model_rejects_uncataloged_slash_names() {
    // A slash-form id used to auto-classify as OpenRouter and spawn
    // "successfully" with provider_key=openrouter route=none, then run on
    // whatever runtime the openrouter slot held (the claude-sonnet-4
    // misidentity incident). Without a catalog entry it must now fail closed.
    let error = resolve_swarm_spawn_selection(
        Some("bridge/gemini-3-flash-agent".to_string()),
        None,
        &coordinator_identity(
            Some("claude-opus-4-8"),
            Some("claude-oauth"),
            Some("claude-oauth"),
        ),
        &[],
        &[],
    )
    .expect_err("uncataloged slash-form model must fail closed");
    assert!(error.to_string().contains("swarm list_models"));
}

#[test]
fn resolve_swarm_spawn_model_rejects_unknown_per_spawn_model() {
    // This replaces the old lines 90-112 behavior that accepted the same
    // unrecognized prefix as `provider_key=None`.
    let requested = "unknown:gpt-5.5";
    let error = resolve_swarm_spawn_selection(
        Some(requested.to_string()),
        None,
        &coordinator_identity(
            Some("claude-opus-4-8"),
            Some("claude-oauth"),
            Some("claude-oauth"),
        ),
        &[],
        &[],
    )
    .expect_err("unknown per-spawn model must fail closed");

    let message = error.to_string();
    assert!(message.contains(requested));
    assert!(message.contains("swarm list_models"));
    assert!(message.contains("openai-oauth:"));
}

#[test]
fn resolve_swarm_spawn_model_rejects_unknown_configured_model() {
    let configured = "definitely-unknown-swarm-model";
    let error = resolve_swarm_spawn_selection(
        None,
        Some(configured.to_string()),
        &coordinator_identity(
            Some("claude-opus-4-8"),
            Some("claude-oauth"),
            Some("claude-oauth"),
        ),
        &[],
        &[],
    )
    .expect_err("unknown agents.swarm_model must fail closed");

    assert!(error.to_string().contains(configured));
}

#[test]
fn concrete_spawn_validation_allows_only_an_inheritance_or_resolved_route_escape() {
    let unresolved = validate_concrete_spawn_selection(
        "definitely-unknown-swarm-model",
        Some("coordinator-model"),
        None,
        None,
        None,
    );
    assert!(unresolved.is_err());

    assert!(
        validate_concrete_spawn_selection(
            "coordinator-model",
            Some("coordinator-model"),
            None,
            None,
            None,
        )
        .is_ok()
    );
    assert!(
        validate_concrete_spawn_selection(
            "profile:model",
            Some("coordinator-model"),
            Some("profile"),
            None,
            None,
        )
        .is_ok()
    );
    assert!(
        validate_concrete_spawn_selection(
            "resolved-model",
            Some("coordinator-model"),
            None,
            Some("provider"),
            None,
        )
        .is_ok()
    );
}

#[test]
fn resolve_swarm_spawn_model_prefers_configured_model_over_coordinator_model() {
    // The configured pin uses a catalog-listed model: bare OpenRouter-style
    // `model@provider` pins fail closed now that the passthrough heuristic is
    // gone, so the pin must name a route the catalog actually serves.
    let routes = [catalog_route(
        "MiniMax-M3",
        "MiniMax",
        "openai-compatible:minimax",
        true,
    )];
    let selection = resolve_swarm_spawn_selection(
        None,
        Some("MiniMax-M3".to_string()),
        &coordinator_identity(
            Some("nvidia/llama-3.3-nemotron-super-49b-v1"),
            Some("nvidia"),
            Some("openai-compatible:nvidia-nim"),
        ),
        &routes,
        &[],
    )
    .expect("configured catalog model should resolve");

    assert_eq!(selection.model.as_deref(), Some("MiniMax-M3"));
    // A different configured model must not inherit the coordinator's route.
    assert_eq!(
        selection.route_api_method.as_deref(),
        Some("openai-compatible:minimax")
    );
}

#[test]
fn resolve_swarm_spawn_model_empty_policy_preserves_existing_resolution() {
    let coordinator = coordinator_identity(
        Some("claude-opus-4-8"),
        Some("claude-oauth"),
        Some("claude-oauth"),
    );
    let selection = resolve_swarm_spawn_selection(
        Some("openai-api:gpt-5.5".to_string()),
        None,
        &coordinator,
        &[],
        &[],
    )
    .expect("an empty policy must preserve existing resolution");

    assert_eq!(selection.model.as_deref(), Some("gpt-5.5"));
    assert_eq!(selection.provider_key.as_deref(), Some("openai-api-key"));
    assert_eq!(
        selection.route_api_method.as_deref(),
        Some("openai-api-key")
    );
}

#[test]
fn resolve_swarm_spawn_model_denied_identity_returns_policy_error() {
    let denied = vec!["cursor:gpt-5.6-sol-high".to_string()];
    let coordinator = coordinator_identity(
        Some("claude-opus-4-8"),
        Some("claude-oauth"),
        Some("claude-oauth"),
    );
    let error = resolve_swarm_spawn_selection(
        Some("cursor:gpt-5.6-sol-high".to_string()),
        None,
        &coordinator,
        &[],
        &denied,
    )
    .expect_err("a denied model must be refused even when it resolves");
    let message = error.to_string();
    assert!(message.contains("denied by policy"));
    assert!(message.contains("agents.swarm_denied_models"));
    assert!(!message.contains("could not be resolved"));

    resolve_swarm_spawn_selection(
        Some("gpt-5.6-sol".to_string()),
        None,
        &coordinator,
        &[],
        &denied,
    )
    .expect("a route-prefixed deny entry must not deny a distinct bare model");
}

#[test]
fn resolve_swarm_spawn_model_inherits_coordinator_when_unconfigured() {
    let selection = resolved_spawn_selection(
        None,
        None,
        &coordinator_identity(
            Some("nvidia/llama-3.3-nemotron-super-49b-v1"),
            Some("nvidia"),
            Some("openai-compatible:nvidia-nim"),
        ),
    );

    assert_eq!(
        selection.model.as_deref(),
        Some("nvidia/llama-3.3-nemotron-super-49b-v1")
    );
    assert_eq!(selection.provider_key.as_deref(), Some("nvidia"));
    assert_eq!(
        selection.route_api_method.as_deref(),
        Some("openai-compatible:nvidia-nim")
    );
}

#[test]
fn resolve_swarm_spawn_model_inherits_coordinator_auth_route_for_oauth_vs_api() {
    // Regression: a coordinator on the Claude API route must spawn agents on
    // the same API route, not Claude OAuth (the config default).
    let selection = resolved_spawn_selection(
        None,
        None,
        &coordinator_identity(
            Some("claude-opus-4-6"),
            Some("claude-api"),
            Some("claude-api"),
        ),
    );

    assert_eq!(selection.model.as_deref(), Some("claude-opus-4-6"));
    assert_eq!(selection.provider_key.as_deref(), Some("claude-api"));
    assert_eq!(selection.route_api_method.as_deref(), Some("claude-api"));
}

#[test]
fn resolve_swarm_spawn_model_keeps_provider_key_when_config_matches_coordinator() {
    let selection = resolved_spawn_selection(
        None,
        Some("custom-model".to_string()),
        &coordinator_identity(
            Some("custom-model"),
            Some("custom-provider"),
            Some("custom-route"),
        ),
    );

    assert_eq!(selection.model.as_deref(), Some("custom-model"));
    assert_eq!(selection.provider_key.as_deref(), Some("custom-provider"));
    assert_eq!(selection.route_api_method.as_deref(), Some("custom-route"));
}

#[test]
fn resolve_swarm_spawn_model_openai_api_prefix_pins_api_route_over_coordinator() {
    // `agents.swarm_model = "openai-api:gpt-5.5"` must spawn agents on GPT-5.5
    // via the OpenAI API key route, regardless of the coordinator's model/auth.
    let selection = resolved_spawn_selection(
        None,
        Some("openai-api:gpt-5.5".to_string()),
        &coordinator_identity(
            Some("claude-opus-4-8"),
            Some("claude-oauth"),
            Some("claude-oauth"),
        ),
    );

    assert_eq!(selection.model.as_deref(), Some("gpt-5.5"));
    assert_eq!(selection.provider_key.as_deref(), Some("openai-api-key"));
    assert_eq!(
        selection.route_api_method.as_deref(),
        Some("openai-api-key")
    );
}

#[test]
fn resolve_swarm_spawn_model_auth_route_prefixes_pin_expected_routes() {
    for (configured, expected_model, expected_key) in [
        ("openai-api:gpt-5.5", "gpt-5.5", "openai-api-key"),
        ("openai-oauth:gpt-5.5", "gpt-5.5", "openai-oauth"),
        (
            "claude-api:claude-opus-4-8",
            "claude-opus-4-8",
            "anthropic-api-key",
        ),
        (
            "claude-oauth:claude-opus-4-8",
            "claude-opus-4-8",
            "claude-oauth",
        ),
    ] {
        let selection = resolved_spawn_selection(
            None,
            Some(configured.to_string()),
            &coordinator_identity(
                Some("some-other-model"),
                Some("some-key"),
                Some("some-route"),
            ),
        );
        assert_eq!(
            selection.model.as_deref(),
            Some(expected_model),
            "configured {configured:?} model",
        );
        assert_eq!(
            selection.provider_key.as_deref(),
            Some(expected_key),
            "configured {configured:?} provider_key",
        );
        assert_eq!(
            selection.route_api_method.as_deref(),
            Some(expected_key),
            "configured {configured:?} route_api_method",
        );
    }
}

#[test]
fn resolve_swarm_spawn_model_inherit_sentinel_uses_coordinator_model() {
    for sentinel in ["inherit", "INHERIT", "coordinator", " inherit ", ""] {
        let selection = resolved_spawn_selection(
            None,
            Some(sentinel.to_string()),
            &coordinator_identity(
                Some("nvidia/llama-3.3-nemotron-super-49b-v1"),
                Some("nvidia"),
                Some("openai-compatible:nvidia-nim"),
            ),
        );

        assert_eq!(
            selection.model.as_deref(),
            Some("nvidia/llama-3.3-nemotron-super-49b-v1"),
            "sentinel {sentinel:?} should inherit coordinator model",
        );
        assert_eq!(
            selection.provider_key.as_deref(),
            Some("nvidia"),
            "sentinel {sentinel:?} should inherit coordinator provider key",
        );
        assert_eq!(
            selection.route_api_method.as_deref(),
            Some("openai-compatible:nvidia-nim"),
            "sentinel {sentinel:?} should inherit coordinator auth route",
        );
    }
}

#[test]
fn resolve_swarm_spawn_model_requested_model_overrides_configured_pin() {
    // A per-spawn requested model must beat the agents.swarm_model config pin.
    let selection = resolved_spawn_selection(
        Some("openai-api:gpt-5.5".to_string()),
        Some("claude-oauth:claude-opus-4-8".to_string()),
        &coordinator_identity(
            Some("claude-fable-5"),
            Some("claude-oauth"),
            Some("claude-oauth"),
        ),
    );

    assert_eq!(selection.model.as_deref(), Some("gpt-5.5"));
    assert_eq!(selection.provider_key.as_deref(), Some("openai-api-key"));
    assert_eq!(
        selection.route_api_method.as_deref(),
        Some("openai-api-key")
    );
}

#[test]
fn resolve_swarm_spawn_model_requested_inherit_overrides_configured_pin() {
    // An explicit `inherit` request must force coordinator inheritance even
    // when the config pins a different model.
    let selection = resolved_spawn_selection(
        Some("inherit".to_string()),
        Some("openai-api:gpt-5.5".to_string()),
        &coordinator_identity(
            Some("claude-fable-5"),
            Some("claude-api"),
            Some("claude-api"),
        ),
    );

    assert_eq!(selection.model.as_deref(), Some("claude-fable-5"));
    assert_eq!(selection.provider_key.as_deref(), Some("claude-api"));
    assert_eq!(selection.route_api_method.as_deref(), Some("claude-api"));
}

#[test]
fn resolve_swarm_spawn_model_requested_matching_coordinator_model_keeps_route() {
    // Requesting the coordinator's own model keeps its provider key and route.
    let selection = resolved_spawn_selection(
        Some("custom-model".to_string()),
        None,
        &coordinator_identity(
            Some("custom-model"),
            Some("custom-provider"),
            Some("custom-route"),
        ),
    );

    assert_eq!(selection.model.as_deref(), Some("custom-model"));
    assert_eq!(selection.provider_key.as_deref(), Some("custom-provider"));
    assert_eq!(selection.route_api_method.as_deref(), Some("custom-route"));
}

#[test]
fn resolve_swarm_spawn_model_blank_requested_model_falls_back_to_config() {
    // A whitespace-only requested model is treated as "not provided".
    let selection = resolved_spawn_selection(
        Some("   ".to_string()),
        Some("openai-api:gpt-5.5".to_string()),
        &coordinator_identity(
            Some("claude-fable-5"),
            Some("claude-oauth"),
            Some("claude-oauth"),
        ),
    );

    assert_eq!(selection.model.as_deref(), Some("gpt-5.5"));
    assert_eq!(selection.provider_key.as_deref(), Some("openai-api-key"));
}

#[tokio::test]
async fn coordinator_identity_uses_live_agent_when_lock_is_available() {
    let agent = test_agent_with_working_dir("coord", "/tmp/coord").await;
    let live_model = agent.lock().await.provider_model();
    let sessions = Arc::new(RwLock::new(HashMap::new()));
    sessions
        .write()
        .await
        .insert("coord".to_string(), Arc::clone(&agent));

    let identity = resolve_coordinator_spawn_identity("coord", &sessions).await;
    assert_eq!(identity.model.as_deref(), Some(live_model.as_str()));
}

#[tokio::test]
async fn coordinator_identity_falls_back_to_persisted_session_when_agent_busy() {
    let _guard = crate::storage::lock_test_env();
    let temp_home = tempfile::TempDir::new().expect("temp home");
    crate::env::set_var("JCODE_HOME", temp_home.path());

    let agent = test_agent_with_working_dir("coord_busy", "/tmp/coord").await;

    // Persist a coordinator session that records a concrete model + auth route.
    // Persist after the agent is built so it reflects the authoritative on-disk
    // snapshot the spawn path will read when the agent lock is unavailable.
    let mut session = crate::session::Session::create_with_id("coord_busy".to_string(), None, None);
    session.model = Some("claude-opus-4-6".to_string());
    session.provider_key = Some("claude-api".to_string());
    session.route_api_method = Some("claude-api".to_string());
    session.save().expect("persist coordinator session");

    // Hold the agent lock to simulate a coordinator mid-turn: the spawn path
    // must not block and must read the persisted identity instead of defaults.
    let _held = agent.lock().await;
    let sessions = Arc::new(RwLock::new(HashMap::new()));
    sessions
        .write()
        .await
        .insert("coord_busy".to_string(), Arc::clone(&agent));

    let identity = resolve_coordinator_spawn_identity("coord_busy", &sessions).await;
    assert_eq!(identity.model.as_deref(), Some("claude-opus-4-6"));
    assert_eq!(identity.provider_key.as_deref(), Some("claude-api"));
    assert_eq!(identity.route_api_method.as_deref(), Some("claude-api"));

    crate::env::remove_var("JCODE_HOME");
}

#[tokio::test]
async fn spawn_bootstraps_coordinator_when_swarm_has_none() {
    let swarm_members = Arc::new(RwLock::new(HashMap::new()));
    let swarms_by_id = Arc::new(RwLock::new(HashMap::from([(
        "swarm-1".to_string(),
        HashSet::from(["req".to_string()]),
    )])));
    let swarm_coordinators = Arc::new(RwLock::new(HashMap::new()));
    let swarm_plans = Arc::new(RwLock::new(HashMap::<String, VersionedPlan>::new()));
    let (req_member, _req_rx) = member("req", Some("swarm-1"), "agent");
    swarm_members
        .write()
        .await
        .insert("req".to_string(), req_member);
    let (client_event_tx, mut client_event_rx) = mpsc::unbounded_channel();

    let swarm_id = ensure_spawn_coordinator_swarm(
        1,
        "req",
        &client_event_tx,
        &swarm_members,
        &swarms_by_id,
        &swarm_coordinators,
        &swarm_plans,
    )
    .await;

    assert_eq!(swarm_id.as_deref(), Some("swarm-1"));
    assert_eq!(
        swarm_coordinators
            .read()
            .await
            .get("swarm-1")
            .map(String::as_str),
        Some("req")
    );
    assert_eq!(
        swarm_members
            .read()
            .await
            .get("req")
            .map(|member| member.role.as_str()),
        Some("coordinator")
    );
    assert!(matches!(
        client_event_rx.recv().await,
        Some(ServerEvent::Notification {
            notification_type: NotificationType::Message { .. },
            message,
            ..
        }) if message == "You are the coordinator for this swarm."
    ));
}

#[tokio::test]
async fn nested_agent_can_spawn_while_live_coordinator_exists() {
    // Recursive spawning (option A): a spawned child (depth 1, owned by `coord`)
    // may spawn its own children even though a live swarm-level coordinator
    // exists. It must not steal the swarm-level coordinator slot.
    let swarm_members = Arc::new(RwLock::new(HashMap::new()));
    let swarms_by_id = Arc::new(RwLock::new(HashMap::from([(
        "swarm-1".to_string(),
        HashSet::from(["child".to_string(), "coord".to_string()]),
    )])));
    let swarm_coordinators = Arc::new(RwLock::new(HashMap::from([(
        "swarm-1".to_string(),
        "coord".to_string(),
    )])));
    let swarm_plans = Arc::new(RwLock::new(HashMap::<String, VersionedPlan>::new()));
    let (mut child_member, _child_rx) = member("child", Some("swarm-1"), "agent");
    child_member.report_back_to_session_id = Some("coord".to_string());
    let (coord_member, _coord_rx) = member("coord", Some("swarm-1"), "coordinator");
    let mut members = swarm_members.write().await;
    members.insert("child".to_string(), child_member);
    members.insert("coord".to_string(), coord_member);
    drop(members);
    let (client_event_tx, mut client_event_rx) = mpsc::unbounded_channel();

    let swarm_id = ensure_spawn_coordinator_swarm(
        2,
        "child",
        &client_event_tx,
        &swarm_members,
        &swarms_by_id,
        &swarm_coordinators,
        &swarm_plans,
    )
    .await;

    assert_eq!(swarm_id.as_deref(), Some("swarm-1"));
    // The swarm-level coordinator slot is untouched.
    assert_eq!(
        swarm_coordinators
            .read()
            .await
            .get("swarm-1")
            .map(String::as_str),
        Some("coord")
    );
    // The child keeps its agent role; it coordinates its own subtree via
    // report-back ownership, not the swarm-level coordinator slot.
    assert_eq!(
        swarm_members
            .read()
            .await
            .get("child")
            .map(|member| member.role.as_str()),
        Some("agent")
    );
    assert!(client_event_rx.try_recv().is_err());
}

#[tokio::test]
async fn spawn_allowed_at_arbitrary_depth_without_depth_cap() {
    // Build a deep chain root -> a -> b -> c -> d -> e -> f. There is no depth
    // cap anymore, so even a deeply nested agent may still spawn.
    let swarm_members = Arc::new(RwLock::new(HashMap::new()));
    let swarms_by_id = Arc::new(RwLock::new(HashMap::new()));
    let swarm_coordinators = Arc::new(RwLock::new(HashMap::from([(
        "swarm-1".to_string(),
        "root".to_string(),
    )])));
    let swarm_plans = Arc::new(RwLock::new(HashMap::<String, VersionedPlan>::new()));
    {
        let mut members = swarm_members.write().await;
        let (root, _rx) = member("root", Some("swarm-1"), "coordinator");
        members.insert("root".to_string(), root);
        let chain = [
            ("a", "root"),
            ("b", "a"),
            ("c", "b"),
            ("d", "c"),
            ("e", "d"),
            ("f", "e"),
        ];
        for (id, parent) in chain {
            let (mut m, _rx) = member(id, Some("swarm-1"), "agent");
            m.report_back_to_session_id = Some(parent.to_string());
            members.insert(id.to_string(), m);
        }
    }
    let (client_event_tx, _client_event_rx) = mpsc::unbounded_channel();

    // `f` is deeply nested but the swarm is far below the member cap, so spawning
    // is allowed.
    let allowed = ensure_spawn_coordinator_swarm(
        7,
        "f",
        &client_event_tx,
        &swarm_members,
        &swarms_by_id,
        &swarm_coordinators,
        &swarm_plans,
    )
    .await;
    assert_eq!(allowed.as_deref(), Some("swarm-1"));
}

#[tokio::test]
async fn spawn_rejected_when_member_limit_reached() {
    use crate::server::swarm::MAX_SWARM_MEMBERS;

    // Fill the swarm to the member cap; the next spawn must be refused.
    let swarm_members = Arc::new(RwLock::new(HashMap::new()));
    let swarms_by_id = Arc::new(RwLock::new(HashMap::new()));
    let swarm_coordinators = Arc::new(RwLock::new(HashMap::from([(
        "swarm-1".to_string(),
        "root".to_string(),
    )])));
    let swarm_plans = Arc::new(RwLock::new(HashMap::<String, VersionedPlan>::new()));
    {
        let mut members = swarm_members.write().await;
        let (root, _rx) = member("root", Some("swarm-1"), "coordinator");
        members.insert("root".to_string(), root);
        // Add filler members so the swarm holds exactly MAX_SWARM_MEMBERS total.
        for idx in 1..MAX_SWARM_MEMBERS {
            let id = format!("agent-{idx}");
            let (mut m, _rx) = member(&id, Some("swarm-1"), "agent");
            m.report_back_to_session_id = Some("root".to_string());
            members.insert(id, m);
        }
    }
    let (client_event_tx, mut client_event_rx) = mpsc::unbounded_channel();

    let refused = ensure_spawn_coordinator_swarm(
        7,
        "root",
        &client_event_tx,
        &swarm_members,
        &swarms_by_id,
        &swarm_coordinators,
        &swarm_plans,
    )
    .await;
    assert!(refused.is_none());
    assert!(matches!(
        client_event_rx.recv().await,
        Some(ServerEvent::Error { message, .. })
            if message.contains("Swarm member limit reached")
    ));
}

#[tokio::test]
async fn terminal_members_do_not_consume_spawn_capacity() {
    use crate::server::swarm::MAX_SWARM_MEMBERS;

    let swarm_members = Arc::new(RwLock::new(HashMap::new()));
    let swarms_by_id = Arc::new(RwLock::new(HashMap::new()));
    let swarm_coordinators = Arc::new(RwLock::new(HashMap::from([(
        "swarm-1".to_string(),
        "root".to_string(),
    )])));
    let swarm_plans = Arc::new(RwLock::new(HashMap::<String, VersionedPlan>::new()));
    {
        let mut members = swarm_members.write().await;
        let (root, _rx) = member("root", Some("swarm-1"), "coordinator");
        members.insert("root".to_string(), root);
        for idx in 0..MAX_SWARM_MEMBERS {
            let id = format!("historical-{idx}");
            let (mut historical, _rx) = member(&id, Some("swarm-1"), "agent");
            historical.status = if idx % 2 == 0 {
                "completed".to_string()
            } else {
                "stopped".to_string()
            };
            historical.latest_completion_report = Some(format!("report {idx}"));
            historical.report_back_to_session_id = Some("root".to_string());
            members.insert(id, historical);
        }
    }
    let (client_event_tx, _client_event_rx) = mpsc::unbounded_channel();

    let allowed = ensure_spawn_coordinator_swarm(
        7,
        "root",
        &client_event_tx,
        &swarm_members,
        &swarms_by_id,
        &swarm_coordinators,
        &swarm_plans,
    )
    .await;

    assert_eq!(allowed.as_deref(), Some("swarm-1"));
}
