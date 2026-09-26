use super::*;
use crate::provider::{RouteSelection, RuntimeKey};
use crate::session::{Session, session_path};
use crate::storage::EnvVarGuard;

const PROVIDER_RESUME_A: &str = "provider-resume-a";
const PROVIDER_RESUME_B: &str = "provider-resume-b";

#[derive(Clone, Default)]
struct ResumeProvider {
    resumes: Arc<StdMutex<Vec<Option<String>>>>,
    switched: Arc<std::sync::atomic::AtomicBool>,
}

#[async_trait]
impl Provider for ResumeProvider {
    async fn complete(
        &self,
        _messages: &[Message],
        _tools: &[ToolDefinition],
        _system: &str,
        resume_session_id: Option<&str>,
    ) -> Result<EventStream> {
        self.resumes
            .lock()
            .unwrap()
            .push(resume_session_id.map(str::to_string));
        let resume_id = if self.name() == "gemini" {
            PROVIDER_RESUME_A
        } else {
            PROVIDER_RESUME_B
        };
        Ok(Box::pin(stream! {
            yield Ok(StreamEvent::TextDelta("answer".to_string()));
            yield Ok(StreamEvent::MessageEnd {
                stop_reason: Some("end_turn".to_string()),
            });
            // Claude CLI can emit its resume handle after MessageEnd.
            yield Ok(StreamEvent::SessionId(resume_id.to_string()));
        }))
    }

    fn name(&self) -> &str {
        if self.switched.load(std::sync::atomic::Ordering::Relaxed) {
            "grok-build"
        } else {
            "gemini"
        }
    }

    fn model(&self) -> String {
        format!("{}-fixture", self.name())
    }

    fn available_models(&self) -> Vec<&'static str> {
        vec!["gemini-fixture", "grok-build-fixture"]
    }

    fn set_model(&self, _model: &str) -> Result<()> {
        Ok(())
    }

    fn set_route_selection(&self, _selection: &RouteSelection) -> Result<()> {
        self.switched
            .store(true, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    fn fork(&self) -> Arc<dyn Provider> {
        Arc::new(self.clone())
    }
}

fn isolated_home() -> (tempfile::TempDir, EnvVarGuard, EnvVarGuard) {
    let home = tempfile::tempdir().unwrap();
    let home_guard = EnvVarGuard::set("JCODE_HOME", home.path());
    let telemetry_guard = EnvVarGuard::set("JCODE_NO_TELEMETRY", "1");
    (home, home_guard, telemetry_guard)
}

async fn new_agent(provider: &ResumeProvider) -> Agent {
    let provider: Arc<dyn Provider> = Arc::new(provider.clone());
    let registry = Registry::new(provider.clone()).await;
    Agent::new(provider, registry)
}

async fn streaming_turn(agent: &mut Agent) -> Vec<ServerEvent> {
    let (tx, mut rx) = mpsc::unbounded_channel();
    agent
        .run_once_streaming_mpsc("continue", Vec::new(), None, tx)
        .await
        .expect("streaming turn");
    std::iter::from_fn(|| rx.try_recv().ok()).collect()
}

fn assert_identity(agent: &Agent, local_id: &str, provider_id: &str, events: &[ServerEvent]) {
    assert_eq!(agent.session_id(), local_id);
    let stored = Session::load(local_id).expect("original storage key must still load");
    assert_eq!(stored.id, local_id);
    assert_eq!(stored.provider_session_id.as_deref(), Some(provider_id));
    assert!(
        !session_path(provider_id).unwrap().exists(),
        "provider handle must not become a local storage key"
    );
    let identity_events: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            ServerEvent::SessionId { session_id } => Some(session_id.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        identity_events,
        Vec::<&str>::new(),
        "provider resume handle must not rebind the client's local session identity"
    );
}

#[tokio::test]
async fn new_session_keeps_local_identity() {
    let _lock = crate::storage::lock_test_env();
    let (_home, _home_guard, _telemetry) = isolated_home();
    let provider = ResumeProvider::default();
    let mut agent = new_agent(&provider).await;
    let local_id = agent.session_id().to_string();
    let mut events = streaming_turn(&mut agent).await;
    events.extend(streaming_turn(&mut agent).await);
    assert_eq!(
        *provider.resumes.lock().unwrap(),
        vec![None, Some(PROVIDER_RESUME_A.to_string())]
    );
    assert_identity(&agent, &local_id, PROVIDER_RESUME_A, &events);
}

#[tokio::test]
async fn resumed_session_keeps_local_identity() {
    let _lock = crate::storage::lock_test_env();
    let (_home, _home_guard, _telemetry) = isolated_home();
    let provider = ResumeProvider::default();
    let mut original = new_agent(&provider).await;
    let local_id = original.session_id().to_string();
    original.run_once_capture("before restart").await.unwrap();
    drop(original);

    let mut resumed = new_agent(&provider).await;
    assert_eq!(
        resumed.restore_session(&local_id).unwrap(),
        crate::session::SessionStatus::Active
    );
    let events = streaming_turn(&mut resumed).await;
    assert_eq!(
        *provider.resumes.lock().unwrap(),
        vec![None, Some(PROVIDER_RESUME_A.to_string())]
    );
    assert_identity(&resumed, &local_id, PROVIDER_RESUME_A, &events);
}

#[tokio::test]
async fn split_session_keeps_child_identity_and_parent_storage() {
    let _lock = crate::storage::lock_test_env();
    let (_home, _home_guard, _telemetry) = isolated_home();
    let provider = ResumeProvider::default();
    let mut parent = new_agent(&provider).await;
    parent.run_once_capture("parent transcript").await.unwrap();
    let parent_id = parent.session_id().to_string();
    let parent_before = Session::load(&parent_id).unwrap();
    let (child_id, _) = clone_split_session(&parent_id).unwrap();
    assert_ne!(child_id, parent_id);
    assert_eq!(Session::load(&child_id).unwrap().provider_session_id, None);

    let mut child = new_agent(&provider).await;
    child.restore_session(&child_id).unwrap();
    let events = streaming_turn(&mut child).await;
    assert_eq!(*provider.resumes.lock().unwrap(), vec![None, None]);
    let child_stored = Session::load(&child_id).unwrap();
    assert_eq!(child_stored.parent_id.as_deref(), Some(parent_id.as_str()));
    let parent_after = Session::load(&parent_id).unwrap();
    assert_eq!(parent_after.id, parent_before.id);
    assert_eq!(
        serde_json::to_value(&parent_after.messages).unwrap(),
        serde_json::to_value(&parent_before.messages).unwrap()
    );
    assert_eq!(
        parent_after.provider_session_id,
        parent_before.provider_session_id
    );
    assert_identity(&child, &child_id, PROVIDER_RESUME_A, &events);
}

#[tokio::test]
async fn legacy_stored_session_keeps_local_identity() {
    let _lock = crate::storage::lock_test_env();
    let (_home, _home_guard, _telemetry) = isolated_home();
    // Literal old JSON, not a round trip through today's serializer. In
    // particular, provider_key and route_api_method did not have to be present.
    let local_id = "session_legacy_provider_resume";
    let path = session_path(local_id).unwrap();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        &path,
        r#"{
            "id": "session_legacy_provider_resume",
            "parent_id": null,
            "title": null,
            "created_at": "2025-01-01T00:00:00Z",
            "updated_at": "2025-01-01T00:00:00Z",
            "messages": [],
            "provider_session_id": "legacy-cli-resume"
        }"#,
    )
    .unwrap();
    let legacy = Session::load(local_id).unwrap();
    assert_eq!(legacy.id, local_id);
    assert_eq!(
        legacy.provider_session_id.as_deref(),
        Some("legacy-cli-resume")
    );
    assert_eq!(legacy.provider_key, None);
    assert_eq!(legacy.route_api_method, None);

    let provider = ResumeProvider::default();
    let mut agent = new_agent(&provider).await;
    agent.restore_session(local_id).unwrap();
    let events = streaming_turn(&mut agent).await;
    assert_eq!(
        *provider.resumes.lock().unwrap(),
        vec![Some("legacy-cli-resume".to_string())]
    );
    assert_identity(&agent, local_id, PROVIDER_RESUME_A, &events);
}

#[tokio::test]
async fn route_switch_keeps_local_identity_and_resets_provider_resume() {
    let _lock = crate::storage::lock_test_env();
    let (_home, _home_guard, _telemetry) = isolated_home();
    let provider = ResumeProvider::default();
    let mut original = new_agent(&provider).await;
    let local_id = original.session_id().to_string();
    original
        .run_once_capture("before route switch")
        .await
        .unwrap();
    let agent = Arc::new(Mutex::new(original));
    let selection = RouteSelection {
        model: "grok-build-fixture".to_string(),
        runtime_key: RuntimeKey::Other("grok-build".to_string()),
        api_method: "grok-build".to_string(),
        provider_label: "grok-build".to_string(),
        detail: String::new(),
    };
    let (tx, mut rx) = mpsc::unbounded_channel();
    crate::server::provider_control::handle_set_route(1, selection, &agent, &tx).await;
    assert!(matches!(
        rx.try_recv().unwrap(),
        ServerEvent::ModelChanged { error: None, .. }
    ));
    let switched = Session::load(&local_id).unwrap();
    assert_eq!(switched.provider_session_id, None);
    assert_eq!(switched.route_api_method.as_deref(), Some("grok-build"));

    let mut agent = agent.lock().await;
    let mut events = streaming_turn(&mut agent).await;
    events.extend(streaming_turn(&mut agent).await);
    assert_eq!(
        *provider.resumes.lock().unwrap(),
        vec![None, None, Some(PROVIDER_RESUME_B.to_string())]
    );
    assert_identity(&agent, &local_id, PROVIDER_RESUME_B, &events);
}
