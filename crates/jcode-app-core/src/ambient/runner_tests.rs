use super::AmbientRunnerHandle;
use crate::ambient::{Priority, ScheduleTarget, ScheduledItem};
use crate::message::{Message, Role, StreamEvent, ToolDefinition};
use crate::provider::{EventStream, Provider};
use crate::session::Session;
use anyhow::Result;
use async_stream::stream;
use async_trait::async_trait;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use crate::storage::EnvVarGuard;

struct TestProvider;

#[derive(Clone, Default)]
struct StreamingTestProvider {
    responses: Arc<StdMutex<VecDeque<Vec<StreamEvent>>>>,
}

impl StreamingTestProvider {
    fn queue_response(&self, events: Vec<StreamEvent>) {
        self.responses.lock().unwrap().push_back(events);
    }
}

#[async_trait]
impl Provider for TestProvider {
    async fn complete(
        &self,
        _messages: &[Message],
        _tools: &[ToolDefinition],
        _system: &str,
        _resume_session_id: Option<&str>,
    ) -> Result<EventStream> {
        Err(anyhow::anyhow!(
            "TestProvider should not be used for streaming completions in ambient runner tests"
        ))
    }

    fn name(&self) -> &str {
        "test"
    }

    fn fork(&self) -> Arc<dyn Provider> {
        Arc::new(TestProvider)
    }
}

#[async_trait]
impl Provider for StreamingTestProvider {
    async fn complete(
        &self,
        _messages: &[Message],
        _tools: &[ToolDefinition],
        _system: &str,
        _resume_session_id: Option<&str>,
    ) -> Result<EventStream> {
        let events = self
            .responses
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_default();
        let stream = stream! {
            for event in events {
                yield Ok(event);
            }
        };
        Ok(Box::pin(stream))
    }

    fn name(&self) -> &str {
        "test"
    }

    fn fork(&self) -> Arc<dyn Provider> {
        Arc::new(self.clone())
    }
}

#[tokio::test]
async fn runner_stays_alive_to_service_schedules_when_ambient_disabled() {
    let _guard = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let _home = EnvVarGuard::set("JCODE_HOME", temp.path());

    let provider: Arc<dyn Provider> = Arc::new(TestProvider);
    let runner = AmbientRunnerHandle::new(Arc::new(crate::safety::SafetySystem::new()));
    let task = tokio::spawn(runner.clone().run_loop(provider));

    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(
        runner.is_running().await,
        "runner should remain active for scheduled tasks even with ambient disabled"
    );

    task.abort();
    let _ = task.await;
}

#[tokio::test]
async fn spawn_target_creates_one_child_session_and_runs_task() {
    let _guard = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let _home = EnvVarGuard::set("JCODE_HOME", temp.path());

    let provider = StreamingTestProvider::default();
    provider.queue_response(vec![
        StreamEvent::TextDelta("Spawned session handled task.".to_string()),
        StreamEvent::MessageEnd { stop_reason: None },
    ]);
    let provider: Arc<dyn Provider> = Arc::new(provider);

    let mut parent = Session::create_with_id(
        "session_parent_spawn_test".to_string(),
        None,
        Some("Parent".to_string()),
    );
    parent.working_dir = Some(temp.path().display().to_string());
    parent.save().expect("save parent session");

    let item = ScheduledItem {
        id: "sched_spawn_test".to_string(),
        scheduled_for: chrono::Utc::now(),
        context: "Follow up later".to_string(),
        priority: Priority::Normal,
        target: ScheduleTarget::Spawn {
            parent_session_id: parent.id.clone(),
        },
        created_by_session: parent.id.clone(),
        created_at: chrono::Utc::now(),
        working_dir: parent.working_dir.clone(),
        task_description: Some("Follow up later".to_string()),
        relevant_files: vec!["src/lib.rs".to_string()],
        git_branch: None,
        additional_context: Some("Background: spawned schedule test".to_string()),
    };

    let runner = AmbientRunnerHandle::new(Arc::new(crate::safety::SafetySystem::new()));
    let child_session_id = runner
        .spawn_session_for_scheduled_item(&provider, &item, &parent.id)
        .await
        .expect("spawned scheduled task should succeed");

    assert_ne!(child_session_id, parent.id);

    let child = Session::load(&child_session_id).expect("load spawned child session");
    assert_eq!(child.parent_id.as_deref(), Some(parent.id.as_str()));
    assert_eq!(child.working_dir, parent.working_dir);
    assert!(child.messages.iter().any(|message| {
        message.role == Role::User
            && message.content_preview().contains("[Scheduled task]")
            && message.content_preview().contains("Follow up later")
    }));
    assert!(child.messages.iter().any(|message| {
        message.role == Role::Assistant
            && message
                .content_preview()
                .contains("Spawned session handled task.")
    }));
}

/// Wire 1: an ambient cycle must tell the scheduler's usage log what it spent.
///
/// The log is how the scheduler learns that ambient has been expensive and
/// should back off, so a cycle that spends tokens silently leaves the next
/// interval computed from nothing. Before this wire, `UsageLog::record` had no
/// non-test caller at all.
///
/// This drives the real `run_loop` with ambient enabled and reads the log back
/// off disk, the same file `UsageLog::load` reads at startup, rather than
/// inspecting the in-memory scheduler. It never writes the field it asserts on.
#[tokio::test]
async fn ambient_cycle_records_what_it_spent_in_the_usage_log() {
    let _guard = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let _home = EnvVarGuard::set("JCODE_HOME", temp.path());
    let _enabled = EnvVarGuard::set("JCODE_AMBIENT_ENABLED", "true");
    // `visible` defaults to true, which spawns a separate TUI process to run the
    // cycle; only the headless path runs an agent in-process, which is the one
    // that can report what it spent.
    let _headless = EnvVarGuard::set("JCODE_AMBIENT_VISIBLE", "false");
    crate::config::invalidate_config_cache();

    // The token counts are the assertion, so they are distinctive rather than
    // round: a stray default or a doubled accumulation would not land here.
    let provider = StreamingTestProvider::default();
    provider.queue_response(vec![
        StreamEvent::TokenUsage {
            input_tokens: Some(4321),
            output_tokens: Some(1234),
            cache_read_input_tokens: None,
            cache_creation_input_tokens: None,
        },
        StreamEvent::TextDelta("Ambient cycle did some work.".to_string()),
        StreamEvent::MessageEnd { stop_reason: None },
    ]);
    let provider: Arc<dyn Provider> = Arc::new(provider);

    let runner = AmbientRunnerHandle::new(Arc::new(crate::safety::SafetySystem::new()));
    let task = tokio::spawn(runner.clone().run_loop(Arc::clone(&provider)));

    // Without this the loop parks itself two hours out and no cycle ever runs.
    // `trigger` is the same call the `/ambient run` path uses.
    tokio::time::sleep(Duration::from_millis(100)).await;
    runner.trigger().await;

    let log_path = temp.path().join("ambient").join("usage.json");
    let mut records: Vec<crate::ambient_scheduler::UsageRecord> = Vec::new();
    for _ in 0..150 {
        if log_path.exists()
            && let Ok(found) =
                crate::storage::read_json::<Vec<crate::ambient_scheduler::UsageRecord>>(&log_path)
            && !found.is_empty()
        {
            records = found;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    task.abort();
    let _ = task.await;

    assert_eq!(
        records.len(),
        1,
        "one completed cycle must leave exactly one usage record, found {:?}",
        records
    );
    let record = &records[0];
    assert!(
        matches!(
            record.source,
            crate::ambient_scheduler::UsageSource::Ambient
        ),
        "a cycle run by the ambient runner must be attributed to Ambient, not the user"
    );
    assert_eq!(record.tokens_input, 4321, "input tokens must reach the log");
    assert_eq!(
        record.tokens_output, 1234,
        "output tokens must reach the log"
    );
}

/// Builds a provider that will attempt a tier-2 `write` to `target` on its
/// first turn, then finish.
fn provider_attempting_write(target: &std::path::Path) -> Arc<dyn Provider> {
    let provider = StreamingTestProvider::default();
    provider.queue_response(vec![
        StreamEvent::ToolUseStart {
            id: "attempted_write".to_string(),
            name: "write".to_string(),
        },
        StreamEvent::ToolInputDelta(
            serde_json::json!({
                "file_path": target.display().to_string(),
                "content": "tier-2 write by an unattended agent",
            })
            .to_string(),
        ),
        StreamEvent::ToolUseEnd,
        StreamEvent::MessageEnd {
            stop_reason: Some("tool_use".to_string()),
        },
    ]);
    provider.queue_response(vec![
        StreamEvent::TextDelta("finished".to_string()),
        StreamEvent::MessageEnd { stop_reason: None },
    ]);
    Arc::new(provider)
}

fn scheduled_item(id: &str, target: ScheduleTarget, session_id: &str, dir: &str) -> ScheduledItem {
    ScheduledItem {
        id: id.to_string(),
        scheduled_for: chrono::Utc::now(),
        context: "scheduled work".to_string(),
        priority: Priority::Normal,
        target,
        created_by_session: session_id.to_string(),
        created_at: chrono::Utc::now(),
        working_dir: Some(dir.to_string()),
        task_description: Some("scheduled work".to_string()),
        relevant_files: Vec::new(),
        git_branch: None,
        additional_context: None,
    }
}

/// A gated ambient cycle must not be able to launder a tier-2 action through
/// `schedule_ambient`.
///
/// `schedule_ambient` is deliberately tier-gate exempt (see `TIER_GATE_EXEMPT`
/// in `tool/ambient.rs`) because enqueuing is not itself a tier-2 action. That
/// is only true while the agent which later RUNS the item inherits the gate.
/// This drives the real dispatch path (`deliver_ready_direct_items`), not the
/// gate function directly, so it fails if the spawn seam stops registering.
#[tokio::test]
async fn scheduled_spawn_does_not_launder_tier_two_past_the_gate() {
    let _guard = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let _home = EnvVarGuard::set("JCODE_HOME", temp.path());

    let target = temp.path().join("must_not_exist.txt");
    let provider = provider_attempting_write(&target);

    let mut parent = Session::create_with_id(
        "session_gated_ambient_parent".to_string(),
        None,
        Some("Gated ambient parent".to_string()),
    );
    parent.working_dir = Some(temp.path().display().to_string());
    parent.save().expect("save parent session");
    let parent_guard = crate::tool::ambient::AmbientSessionGuard::new(parent.id.clone());

    // Precondition: the parent really is gated for a direct tier-2 action.
    assert!(
        crate::tool::ambient::check_ambient_action_tier(&parent.id, "write").is_err(),
        "precondition: a registered ambient session must be refused a direct tier-2 write"
    );

    let item = scheduled_item(
        "sched_no_launder",
        ScheduleTarget::Spawn {
            parent_session_id: parent.id.clone(),
        },
        &parent.id,
        &temp.path().display().to_string(),
    );

    let runner = AmbientRunnerHandle::new(Arc::new(crate::safety::SafetySystem::new()));
    runner
        .deliver_ready_direct_items(&provider, vec![item])
        .await;

    assert!(
        !target.exists(),
        "a session spawned for a scheduled item runs unattended, so it must inherit the \
         ambient tier gate; otherwise `schedule_ambient` is an escalation path"
    );
    drop(parent_guard);
}

/// The fallback resume path is unattended too: live delivery has already
/// failed, so no human is reading the session that gets resumed.
#[tokio::test]
async fn resumed_dead_session_does_not_take_tier_two_actions() {
    let _guard = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let _home = EnvVarGuard::set("JCODE_HOME", temp.path());

    let target = temp.path().join("must_not_exist_resume.txt");
    let provider = provider_attempting_write(&target);

    let mut dead = Session::create_with_id(
        "session_dead_for_resume".to_string(),
        None,
        Some("Dead session".to_string()),
    );
    dead.working_dir = Some(temp.path().display().to_string());
    dead.status = crate::session::SessionStatus::Closed;
    dead.save().expect("save dead session");

    let item = scheduled_item(
        "sched_resume_dead",
        ScheduleTarget::Session {
            session_id: dead.id.clone(),
        },
        &dead.id,
        &temp.path().display().to_string(),
    );

    // No server is listening, so live delivery fails and this falls back to the
    // headless resume path under test.
    let runner = AmbientRunnerHandle::new(Arc::new(crate::safety::SafetySystem::new()));
    runner
        .deliver_ready_direct_items(&provider, vec![item])
        .await;

    assert!(
        !target.exists(),
        "resuming a dead session to deliver a scheduled reminder is unattended, so it must \
         inherit the ambient tier gate"
    );
}

/// The gate keys on session ID, so a leaked registration would gate an
/// unrelated later session that reused the ID. The guard must clean up even
/// when the unattended run fails.
#[tokio::test]
async fn ambient_session_guard_unregisters_on_failure() {
    let _guard = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let _home = EnvVarGuard::set("JCODE_HOME", temp.path());

    let session_id = "session_guard_cleanup_probe";
    {
        let _scoped = crate::tool::ambient::AmbientSessionGuard::new(session_id);
        assert!(
            crate::tool::ambient::check_ambient_action_tier(session_id, "write").is_err(),
            "session must be gated while the guard is alive"
        );
    }
    assert!(
        crate::tool::ambient::check_ambient_action_tier(session_id, "write").is_ok(),
        "guard must unregister on drop; a leaked ID would gate a later unrelated session"
    );
}

/// The ambient registry is global, so a test that leaks an ID would gate an
/// unrelated test running later. Runs after the scheduled-dispatch tests and
/// asserts they left nothing behind.
#[tokio::test]
async fn zz_scheduled_dispatch_tests_leave_no_registered_sessions() {
    let _guard = crate::storage::lock_test_env();
    for id in [
        "session_gated_ambient_parent",
        "session_dead_for_resume",
        "session_guard_cleanup_probe",
    ] {
        assert!(
            crate::tool::ambient::check_ambient_action_tier(id, "write").is_ok(),
            "session '{id}' is still registered ambient after its test; a leaked ID gates \
             unrelated later sessions"
        );
    }
}

/// An unattended agent must not be able to launder a tier-2 action through
/// `subagent`.
///
/// This drives the real `run_subagent_worker` path rather than calling the
/// guard directly, so it fails if the inheritance is defined but never wired
/// into the spawn seam, which is exactly the defect it exists to prevent. The
/// worker runs on a session id minted by `Session::create` that nothing else
/// registers, so without the inherited guard it is ungated regardless of its
/// parent.
#[tokio::test]
async fn subagent_worker_inherits_the_gate_from_an_ambient_parent() {
    let _guard = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let _home = EnvVarGuard::set("JCODE_HOME", temp.path());

    let target = temp.path().join("subagent_must_not_write.txt");
    let provider = provider_attempting_write(&target);

    let mut parent = Session::create_with_id(
        "session_gated_subagent_parent".to_string(),
        None,
        Some("Gated subagent parent".to_string()),
    );
    parent.working_dir = Some(temp.path().display().to_string());
    parent.save().expect("save parent session");
    let _parent_guard = crate::tool::ambient::AmbientSessionGuard::new(parent.id.clone());

    // Precondition: the parent really is gated for a direct tier-2 action.
    assert!(
        crate::tool::ambient::check_ambient_action_tier(&parent.id, "write").is_err(),
        "precondition: a registered ambient session must be refused a direct tier-2 write"
    );

    let registry = crate::tool::Registry::new(provider.clone()).await;
    let subagent_parent = crate::tool::subagent::SubagentParent {
        session_id: parent.id.clone(),
        working_dir: Some(temp.path().to_path_buf()),
        model: "test-model".to_string(),
        provider_key: None,
        route_api_method: None,
    };

    let _ = crate::tool::subagent::run_subagent_worker(
        provider,
        registry,
        subagent_parent,
        "gated worker probe",
        "general-purpose",
        "attempt a write",
        None,
    )
    .await;

    assert!(
        !target.exists(),
        "a worker spawned by an unattended agent wrote {} without a human; \
         the subagent spawn seam is not inheriting the ambient tier gate",
        target.display()
    );
}

/// An overnight run is unattended by construction and must be gated.
///
/// This drives the real `run_supervisor` loop rather than calling the guard
/// directly, so it fails if the guard is defined but never wired into the
/// overnight path, which is the defect it exists to prevent. The manifest is
/// built already past its wake and grace windows so the loop takes exactly one
/// coordinator turn and then completes.
#[tokio::test]
async fn overnight_supervisor_gates_a_tier_two_tool() {
    let _guard = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let _home = EnvVarGuard::set("JCODE_HOME", temp.path());

    let target = temp.path().join("overnight_must_not_write.txt");
    let provider = provider_attempting_write(&target);

    let mut child = Session::create_with_id(
        "session_overnight_coordinator".to_string(),
        None,
        Some("Overnight coordinator".to_string()),
    );
    child.working_dir = Some(temp.path().display().to_string());
    child.save().expect("save coordinator session");

    let manifest = crate::overnight::test_manifest_past_wake(temp.path(), &child.id);
    crate::overnight::save_manifest(&manifest).expect("save manifest");

    let registry = crate::tool::Registry::new(provider.clone()).await;
    let _ = crate::overnight::run_supervisor(manifest, child, provider, registry, false).await;

    assert!(
        !target.exists(),
        "the overnight coordinator wrote {} with no human present; \
         the overnight supervisor is not registering its session with the ambient tier gate",
        target.display()
    );
}

/// Counter-check to the test above: the overnight guard must not survive the
/// run. A leaked registration is not inert, since session IDs are the gate's
/// only key and a stale ID would gate a later, unrelated session.
#[tokio::test]
async fn overnight_supervisor_unregisters_its_session_on_exit() {
    let _guard = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let _home = EnvVarGuard::set("JCODE_HOME", temp.path());

    let target = temp.path().join("overnight_cleanup_probe.txt");
    let provider = provider_attempting_write(&target);

    let mut child = Session::create_with_id(
        "session_overnight_cleanup".to_string(),
        None,
        Some("Overnight cleanup probe".to_string()),
    );
    child.working_dir = Some(temp.path().display().to_string());
    child.save().expect("save coordinator session");
    let child_id = child.id.clone();

    let manifest = crate::overnight::test_manifest_past_wake(temp.path(), &child_id);
    crate::overnight::save_manifest(&manifest).expect("save manifest");

    let registry = crate::tool::Registry::new(provider.clone()).await;
    let _ = crate::overnight::run_supervisor(manifest, child, provider, registry, false).await;

    assert!(
        crate::tool::ambient::check_ambient_action_tier(&child_id, "write").is_ok(),
        "the overnight guard leaked its registration past the run; a later session \
         reusing this id would be gated with no unattended agent present"
    );
}
