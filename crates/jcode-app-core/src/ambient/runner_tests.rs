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
        matches!(record.source, crate::ambient_scheduler::UsageSource::Ambient),
        "a cycle run by the ambient runner must be attributed to Ambient, not the user"
    );
    assert_eq!(record.tokens_input, 4321, "input tokens must reach the log");
    assert_eq!(
        record.tokens_output, 1234,
        "output tokens must reach the log"
    );
}

