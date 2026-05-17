use super::*;
use crate::bus::{
    BackgroundTaskCompleted, BackgroundTaskProgress, BackgroundTaskProgressEvent,
    BackgroundTaskProgressKind, BackgroundTaskProgressSource, BackgroundTaskStatus, BusEvent,
    ClientMaintenanceAction, InputShellCompleted, SessionUpdateStatus,
};
use crate::tui::TuiState;
use ratatui::backend::Backend;
use ratatui::layout::Rect;
use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc as StdArc, Mutex as StdMutex};
use std::time::{Duration, Instant};

fn cleanup_background_task_files(task_id: &str) {
    let task_dir = std::env::temp_dir().join("jcode-bg-tasks");
    let _ = std::fs::remove_file(task_dir.join(format!("{}.status.json", task_id)));
    let _ = std::fs::remove_file(task_dir.join(format!("{}.output", task_id)));
}

pub(super) fn cleanup_reload_context_file(session_id: &str) {
    if let Ok(path) = crate::tool::selfdev::ReloadContext::path_for_session(session_id) {
        let _ = std::fs::remove_file(path);
    }
}

// Mock provider for testing
struct MockProvider;

#[derive(Clone)]
struct RefreshSummaryProvider {
    summary: crate::provider::ModelCatalogRefreshSummary,
}

#[derive(Clone)]
struct OpenRouterSpecCaptureProvider {
    set_model_calls: StdArc<StdMutex<Vec<String>>>,
}

#[async_trait::async_trait]
impl Provider for MockProvider {
    async fn complete(
        &self,
        _messages: &[Message],
        _tools: &[crate::message::ToolDefinition],
        _system: &str,
        _resume_session_id: Option<&str>,
    ) -> Result<crate::provider::EventStream> {
        unimplemented!("Mock provider")
    }

    fn name(&self) -> &str {
        "mock"
    }

    fn fork(&self) -> Arc<dyn Provider> {
        Arc::new(MockProvider)
    }
}

#[async_trait::async_trait]
impl Provider for RefreshSummaryProvider {
    async fn complete(
        &self,
        _messages: &[Message],
        _tools: &[crate::message::ToolDefinition],
        _system: &str,
        _resume_session_id: Option<&str>,
    ) -> Result<crate::provider::EventStream> {
        unimplemented!("RefreshSummaryProvider")
    }

    fn name(&self) -> &str {
        "refresh-summary"
    }

    fn fork(&self) -> Arc<dyn Provider> {
        Arc::new(self.clone())
    }

    async fn refresh_model_catalog(&self) -> Result<crate::provider::ModelCatalogRefreshSummary> {
        Ok(self.summary.clone())
    }
}

#[async_trait::async_trait]
impl Provider for OpenRouterSpecCaptureProvider {
    async fn complete(
        &self,
        _messages: &[Message],
        _tools: &[crate::message::ToolDefinition],
        _system: &str,
        _resume_session_id: Option<&str>,
    ) -> Result<crate::provider::EventStream> {
        unimplemented!("OpenRouterSpecCaptureProvider")
    }

    fn name(&self) -> &str {
        "openrouter-spec-capture"
    }

    fn model(&self) -> String {
        "gpt-5.4".to_string()
    }

    fn model_routes(&self) -> Vec<crate::provider::ModelRoute> {
        vec![crate::provider::ModelRoute {
            model: "gpt-5.4".to_string(),
            provider: "OpenAI".to_string(),
            api_method: "openrouter".to_string(),
            available: true,
            detail: "cached route".to_string(),
            cheapness: None,
        }]
    }

    fn available_providers_for_model(&self, model: &str) -> Vec<String> {
        if model == "gpt-5.4" || model == "openai/gpt-5.4" {
            vec!["auto".to_string(), "OpenAI".to_string()]
        } else {
            Vec::new()
        }
    }

    fn available_efforts(&self) -> Vec<&'static str> {
        vec!["high"]
    }

    fn reasoning_effort(&self) -> Option<String> {
        Some("high".to_string())
    }

    fn set_reasoning_effort(&self, _effort: &str) -> Result<()> {
        Ok(())
    }

    fn set_model(&self, model: &str) -> Result<()> {
        self.set_model_calls.lock().unwrap().push(model.to_string());
        Ok(())
    }

    fn fork(&self) -> Arc<dyn Provider> {
        Arc::new(self.clone())
    }
}

fn create_test_app() -> TestApp {
    let env = TestEnvHandle::acquire();

    let provider: Arc<dyn Provider> = Arc::new(MockProvider);
    let rt = tokio::runtime::Runtime::new().unwrap();
    let registry = rt.block_on(crate::tool::Registry::new(provider.clone()));
    let mut app = App::new_for_test_harness(provider, registry);
    app.queue_mode = false;
    app.diff_mode = crate::config::DiffDisplayMode::Inline;
    TestApp { app, _env: env }
}

fn wait_for_model_picker_load(app: &mut App) {
    let start = Instant::now();
    while app.pending_model_picker_load.is_some() {
        app.poll_model_picker_load();
        assert!(
            start.elapsed() < Duration::from_secs(2),
            "timed out waiting for async model picker load"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn create_refresh_summary_test_app(summary: crate::provider::ModelCatalogRefreshSummary) -> TestApp {
    let env = TestEnvHandle::acquire();

    let provider: Arc<dyn Provider> = Arc::new(RefreshSummaryProvider { summary });
    let rt = tokio::runtime::Runtime::new().unwrap();
    let registry = rt.block_on(crate::tool::Registry::new(provider.clone()));
    let mut app = App::new_for_test_harness(provider, registry);
    app.queue_mode = false;
    app.diff_mode = crate::config::DiffDisplayMode::Inline;
    TestApp { app, _env: env }
}

fn create_openrouter_spec_capture_test_app() -> (TestApp, StdArc<StdMutex<Vec<String>>>) {
    let env = TestEnvHandle::acquire();

    let set_model_calls = StdArc::new(StdMutex::new(Vec::new()));
    let provider: Arc<dyn Provider> = Arc::new(OpenRouterSpecCaptureProvider {
        set_model_calls: set_model_calls.clone(),
    });
    let rt = tokio::runtime::Runtime::new().unwrap();
    let registry = rt.block_on(crate::tool::Registry::new(provider.clone()));
    let mut app = App::new_for_test_harness(provider, registry);
    app.queue_mode = false;
    app.diff_mode = crate::config::DiffDisplayMode::Inline;
    (TestApp { app, _env: env }, set_model_calls)
}

#[test]
fn local_add_provider_message_does_not_retain_local_provider_copy() {
    let mut app = create_test_app();
    app.add_provider_message(Message::user("hello"));
    assert!(app.messages.is_empty());
}

#[test]
fn remote_add_provider_message_retains_remote_provider_copy() {
    let mut app = create_test_app();
    app.is_remote = true;
    app.ensure_provider_messages_hydrated();
    let before = app.messages.len();
    app.add_provider_message(Message::user("hello"));
    assert_eq!(app.messages.len(), before + 1);
}

#[test]
fn debug_memory_profile_includes_app_owned_summary_for_large_client_state() {
    let mut app = create_test_app();
    app.remote_side_pane_images
        .push(crate::session::RenderedImage {
            media_type: "image/png".to_string(),
            data: "x".repeat(32 * 1024),
            label: Some("preview.png".to_string()),
            source: crate::session::RenderedImageSource::UserInput,
        });
    app.observe_page_markdown = "# observe\n".repeat(256);
    app.input_undo_stack.push(("draft ".repeat(256), 12));

    let profile = app.debug_memory_profile();
    let app_owned = &profile["app_owned"];
    let summary = &profile["summary"];

    assert!(app_owned.is_object());
    assert!(summary.is_object());
    assert!(
        app_owned["images_and_views"]["remote_side_pane_images_bytes"]
            .as_u64()
            .unwrap_or(0)
            >= 32 * 1024
    );
    assert!(
        app_owned["input_history"]["undo_stack_bytes"]
            .as_u64()
            .unwrap_or(0)
            > 0
    );
    assert!(
        summary["total_app_owned_estimate_bytes"]
            .as_u64()
            .unwrap_or(0)
            > 0
    );
    assert!(
        !summary["top_buckets"]
            .as_array()
            .unwrap_or(&Vec::new())
            .is_empty()
    );
}

fn test_side_panel_snapshot(page_id: &str, title: &str) -> crate::side_panel::SidePanelSnapshot {
    crate::side_panel::SidePanelSnapshot {
        focused_page_id: Some(page_id.to_string()),
        pages: vec![crate::side_panel::SidePanelPage {
            id: page_id.to_string(),
            title: title.to_string(),
            file_path: format!("/tmp/{page_id}.md"),
            format: crate::side_panel::SidePanelPageFormat::Markdown,
            source: crate::side_panel::SidePanelPageSource::Managed,
            content: format!("# {title}"),
            updated_at_ms: 1,
        }],
    }
}

/// Wrapper around `App` that owns an isolated test environment for its
/// lifetime. Auto-derefs to `App` so most call sites stay unchanged.
struct TestApp {
    app: App,
    _env: TestEnvHandle,
}

impl std::ops::Deref for TestApp {
    type Target = App;

    fn deref(&self) -> &App {
        &self.app
    }
}

impl std::ops::DerefMut for TestApp {
    fn deref_mut(&mut self) -> &mut App {
        &mut self.app
    }
}
#[allow(clippy::too_many_lines)]
impl crate::tui::TuiState for TestApp {
    fn display_messages(&self) -> &[crate::tui::DisplayMessage]  { <App as crate::tui::TuiState>::display_messages(&self.app) }
    fn display_user_message_count(&self) -> usize  { <App as crate::tui::TuiState>::display_user_message_count(&self.app) }
    fn has_display_edit_tool_messages(&self) -> bool  { <App as crate::tui::TuiState>::has_display_edit_tool_messages(&self.app) }
    fn side_pane_images(&self) -> Vec<crate::session::RenderedImage>  { <App as crate::tui::TuiState>::side_pane_images(&self.app) }
    fn display_messages_version(&self) -> u64  { <App as crate::tui::TuiState>::display_messages_version(&self.app) }
    fn streaming_text(&self) -> &str  { <App as crate::tui::TuiState>::streaming_text(&self.app) }
    fn input(&self) -> &str  { <App as crate::tui::TuiState>::input(&self.app) }
    fn cursor_pos(&self) -> usize  { <App as crate::tui::TuiState>::cursor_pos(&self.app) }
    fn is_processing(&self) -> bool  { <App as crate::tui::TuiState>::is_processing(&self.app) }
    fn queued_messages(&self) -> &[String]  { <App as crate::tui::TuiState>::queued_messages(&self.app) }
    fn interleave_message(&self) -> Option<&str>  { <App as crate::tui::TuiState>::interleave_message(&self.app) }
    fn pending_soft_interrupts(&self) -> &[String]  { <App as crate::tui::TuiState>::pending_soft_interrupts(&self.app) }
    fn scroll_offset(&self) -> usize  { <App as crate::tui::TuiState>::scroll_offset(&self.app) }
    fn auto_scroll_paused(&self) -> bool  { <App as crate::tui::TuiState>::auto_scroll_paused(&self.app) }
    fn provider_name(&self) -> String  { <App as crate::tui::TuiState>::provider_name(&self.app) }
    fn provider_model(&self) -> String  { <App as crate::tui::TuiState>::provider_model(&self.app) }
    fn upstream_provider(&self) -> Option<String>  { <App as crate::tui::TuiState>::upstream_provider(&self.app) }
    fn connection_type(&self) -> Option<String>  { <App as crate::tui::TuiState>::connection_type(&self.app) }
    fn status_detail(&self) -> Option<String>  { <App as crate::tui::TuiState>::status_detail(&self.app) }
    fn mcp_servers(&self) -> Vec<(String, usize)>  { <App as crate::tui::TuiState>::mcp_servers(&self.app) }
    fn available_skills(&self) -> Vec<String>  { <App as crate::tui::TuiState>::available_skills(&self.app) }
    fn streaming_tokens(&self) -> (u64, u64)  { <App as crate::tui::TuiState>::streaming_tokens(&self.app) }
    fn streaming_cache_tokens(&self) -> (Option<u64>, Option<u64>)  { <App as crate::tui::TuiState>::streaming_cache_tokens(&self.app) }
    fn output_tps(&self) -> Option<f32>  { <App as crate::tui::TuiState>::output_tps(&self.app) }
    fn streaming_tool_calls(&self) -> Vec<crate::message::ToolCall>  { <App as crate::tui::TuiState>::streaming_tool_calls(&self.app) }
    fn elapsed(&self) -> Option<std::time::Duration>  { <App as crate::tui::TuiState>::elapsed(&self.app) }
    fn status(&self) -> crate::tui::ProcessingStatus  { <App as crate::tui::TuiState>::status(&self.app) }
    fn command_suggestions(&self) -> Vec<(String, &'static str)>  { <App as crate::tui::TuiState>::command_suggestions(&self.app) }
    fn active_skill(&self) -> Option<String>  { <App as crate::tui::TuiState>::active_skill(&self.app) }
    fn subagent_status(&self) -> Option<String>  { <App as crate::tui::TuiState>::subagent_status(&self.app) }
    fn batch_progress(&self) -> Option<crate::bus::BatchProgress>  { <App as crate::tui::TuiState>::batch_progress(&self.app) }
    fn time_since_activity(&self) -> Option<std::time::Duration>  { <App as crate::tui::TuiState>::time_since_activity(&self.app) }
    fn stream_message_ended(&self) -> bool  { <App as crate::tui::TuiState>::stream_message_ended(&self.app) }
    fn total_session_tokens(&self) -> Option<(u64, u64)>  { <App as crate::tui::TuiState>::total_session_tokens(&self.app) }
    fn is_remote_mode(&self) -> bool  { <App as crate::tui::TuiState>::is_remote_mode(&self.app) }
    fn is_canary(&self) -> bool  { <App as crate::tui::TuiState>::is_canary(&self.app) }
    fn is_replay(&self) -> bool  { <App as crate::tui::TuiState>::is_replay(&self.app) }
    fn diff_mode(&self) -> crate::config::DiffDisplayMode  { <App as crate::tui::TuiState>::diff_mode(&self.app) }
    fn current_session_id(&self) -> Option<String>  { <App as crate::tui::TuiState>::current_session_id(&self.app) }
    fn session_display_name(&self) -> Option<String>  { <App as crate::tui::TuiState>::session_display_name(&self.app) }
    fn server_display_name(&self) -> Option<String>  { <App as crate::tui::TuiState>::server_display_name(&self.app) }
    fn server_display_icon(&self) -> Option<String>  { <App as crate::tui::TuiState>::server_display_icon(&self.app) }
    fn server_sessions(&self) -> Vec<String>  { <App as crate::tui::TuiState>::server_sessions(&self.app) }
    fn connected_clients(&self) -> Option<usize>  { <App as crate::tui::TuiState>::connected_clients(&self.app) }
    fn status_notice(&self) -> Option<String>  { <App as crate::tui::TuiState>::status_notice(&self.app) }
    fn active_experimental_feature_notice(&self) -> Option<String>  { <App as crate::tui::TuiState>::active_experimental_feature_notice(&self.app) }
    fn remote_startup_phase_active(&self) -> bool  { <App as crate::tui::TuiState>::remote_startup_phase_active(&self.app) }
    fn has_pending_mouse_scroll_animation(&self) -> bool  { <App as crate::tui::TuiState>::has_pending_mouse_scroll_animation(&self.app) }
    fn dictation_key_label(&self) -> Option<String>  { <App as crate::tui::TuiState>::dictation_key_label(&self.app) }
    fn animation_elapsed(&self) -> f32  { <App as crate::tui::TuiState>::animation_elapsed(&self.app) }
    fn rate_limit_remaining(&self) -> Option<std::time::Duration>  { <App as crate::tui::TuiState>::rate_limit_remaining(&self.app) }
    fn queue_mode(&self) -> bool  { <App as crate::tui::TuiState>::queue_mode(&self.app) }
    fn next_prompt_new_session_armed(&self) -> bool  { <App as crate::tui::TuiState>::next_prompt_new_session_armed(&self.app) }
    fn has_stashed_input(&self) -> bool  { <App as crate::tui::TuiState>::has_stashed_input(&self.app) }
    fn context_info(&self) -> crate::prompt::ContextInfo  { <App as crate::tui::TuiState>::context_info(&self.app) }
    fn context_limit(&self) -> Option<usize>  { <App as crate::tui::TuiState>::context_limit(&self.app) }
    fn client_update_available(&self) -> bool  { <App as crate::tui::TuiState>::client_update_available(&self.app) }
    fn server_update_available(&self) -> Option<bool>  { <App as crate::tui::TuiState>::server_update_available(&self.app) }
    fn info_widget_data(&self) -> crate::tui::info_widget::InfoWidgetData  { <App as crate::tui::TuiState>::info_widget_data(&self.app) }
    fn workspace_mode_enabled(&self) -> bool  { <App as crate::tui::TuiState>::workspace_mode_enabled(&self.app) }
    fn workspace_map_rows(&self) -> Vec<crate::tui::workspace_map::VisibleWorkspaceRow>  { <App as crate::tui::TuiState>::workspace_map_rows(&self.app) }
    fn workspace_animation_tick(&self) -> u64  { <App as crate::tui::TuiState>::workspace_animation_tick(&self.app) }
    fn render_streaming_markdown(&self, width: usize) -> Vec<ratatui::text::Line<'static>>  { <App as crate::tui::TuiState>::render_streaming_markdown(&self.app, width) }
    fn centered_mode(&self) -> bool  { <App as crate::tui::TuiState>::centered_mode(&self.app) }
    fn auth_status(&self) -> crate::auth::AuthStatus  { <App as crate::tui::TuiState>::auth_status(&self.app) }
    fn update_cost(&mut self) { <App as crate::tui::TuiState>::update_cost(&mut self.app) }
    fn diagram_mode(&self) -> crate::config::DiagramDisplayMode  { <App as crate::tui::TuiState>::diagram_mode(&self.app) }
    fn diagram_focus(&self) -> bool  { <App as crate::tui::TuiState>::diagram_focus(&self.app) }
    fn diagram_index(&self) -> usize  { <App as crate::tui::TuiState>::diagram_index(&self.app) }
    fn diagram_scroll(&self) -> (i32, i32)  { <App as crate::tui::TuiState>::diagram_scroll(&self.app) }
    fn diagram_pane_ratio(&self) -> u8  { <App as crate::tui::TuiState>::diagram_pane_ratio(&self.app) }
    fn diagram_pane_animating(&self) -> bool  { <App as crate::tui::TuiState>::diagram_pane_animating(&self.app) }
    fn diagram_pane_enabled(&self) -> bool  { <App as crate::tui::TuiState>::diagram_pane_enabled(&self.app) }
    fn diagram_pane_position(&self) -> crate::config::DiagramPanePosition  { <App as crate::tui::TuiState>::diagram_pane_position(&self.app) }
    fn diagram_zoom(&self) -> u8  { <App as crate::tui::TuiState>::diagram_zoom(&self.app) }
    fn diff_pane_scroll(&self) -> usize  { <App as crate::tui::TuiState>::diff_pane_scroll(&self.app) }
    fn diff_pane_scroll_x(&self) -> i32  { <App as crate::tui::TuiState>::diff_pane_scroll_x(&self.app) }
    fn side_panel_image_zoom_percent(&self) -> u8  { <App as crate::tui::TuiState>::side_panel_image_zoom_percent(&self.app) }
    fn diff_pane_focus(&self) -> bool  { <App as crate::tui::TuiState>::diff_pane_focus(&self.app) }
    fn side_panel(&self) -> &crate::side_panel::SidePanelSnapshot  { <App as crate::tui::TuiState>::side_panel(&self.app) }
    fn pin_images(&self) -> bool  { <App as crate::tui::TuiState>::pin_images(&self.app) }
    fn chat_native_scrollbar(&self) -> bool  { <App as crate::tui::TuiState>::chat_native_scrollbar(&self.app) }
    fn side_panel_native_scrollbar(&self) -> bool  { <App as crate::tui::TuiState>::side_panel_native_scrollbar(&self.app) }
    fn diff_line_wrap(&self) -> bool  { <App as crate::tui::TuiState>::diff_line_wrap(&self.app) }
    fn inline_interactive_state(&self) -> Option<&crate::tui::InlineInteractiveState>  { <App as crate::tui::TuiState>::inline_interactive_state(&self.app) }
    fn inline_view_state(&self) -> Option<&crate::tui::InlineViewState>  { <App as crate::tui::TuiState>::inline_view_state(&self.app) }
    fn inline_ui_state(&self) -> Option<crate::tui::InlineUiStateRef<'_>>  { <App as crate::tui::TuiState>::inline_ui_state(&self.app) }
    fn changelog_scroll(&self) -> Option<usize>  { <App as crate::tui::TuiState>::changelog_scroll(&self.app) }
    fn help_scroll(&self) -> Option<usize>  { <App as crate::tui::TuiState>::help_scroll(&self.app) }
    fn session_picker_overlay(&self) -> Option<&std::cell::RefCell<crate::tui::session_picker::SessionPicker>>  { <App as crate::tui::TuiState>::session_picker_overlay(&self.app) }
    fn login_picker_overlay(&self) -> Option<&std::cell::RefCell<crate::tui::login_picker::LoginPicker>>  { <App as crate::tui::TuiState>::login_picker_overlay(&self.app) }
    fn account_picker_overlay(&self) -> Option<&std::cell::RefCell<crate::tui::account_picker::AccountPicker>>  { <App as crate::tui::TuiState>::account_picker_overlay(&self.app) }
    fn usage_overlay(&self) -> Option<&std::cell::RefCell<crate::tui::usage_overlay::UsageOverlay>>  { <App as crate::tui::TuiState>::usage_overlay(&self.app) }
    fn working_dir(&self) -> Option<String>  { <App as crate::tui::TuiState>::working_dir(&self.app) }
    fn now_millis(&self) -> u64  { <App as crate::tui::TuiState>::now_millis(&self.app) }
    fn copy_badge_ui(&self) -> crate::tui::CopyBadgeUiState  { <App as crate::tui::TuiState>::copy_badge_ui(&self.app) }
    fn copy_selection_mode(&self) -> bool  { <App as crate::tui::TuiState>::copy_selection_mode(&self.app) }
    fn copy_selection_range(&self) -> Option<crate::tui::CopySelectionRange>  { <App as crate::tui::TuiState>::copy_selection_range(&self.app) }
    fn copy_selection_status(&self) -> Option<crate::tui::CopySelectionStatus>  { <App as crate::tui::TuiState>::copy_selection_status(&self.app) }
    fn suggestion_prompts(&self) -> Vec<(String, String)>  { <App as crate::tui::TuiState>::suggestion_prompts(&self.app) }
    fn cache_ttl_status(&self) -> Option<crate::tui::CacheTtlInfo>  { <App as crate::tui::TuiState>::cache_ttl_status(&self.app) }
    fn has_notification(&self) -> bool  { <App as crate::tui::TuiState>::has_notification(&self.app) }
}



/// Composite test-environment guard that combines `TestJcodeHome` (isolated
/// `JCODE_HOME` + global env lock) with TUI/auth cache invalidation.
///
/// Acquiring this guard:
///   1. Acquires `TestJcodeHome` (per-test tempdir + env lock, or passthrough
///      if `JCODE_HOME` is already set by an outer scope).
///   2. Clears `tui::ui` render state.
///   3. Clears ambient-info cache.
///   4. Resets per-provider auth account overrides.
///   5. Invalidates the auth-status cache.
///
/// Dropping this guard repeats the cache resets (steps 2-5) before releasing
/// the underlying `TestJcodeHome`, so a test cannot pollute the next test
/// via static caches even if `JCODE_HOME` is shared.
struct TestEnvHandle {
    _home: crate::storage::TestJcodeHome,
}

impl TestEnvHandle {
    fn acquire() -> Self {
        let home = crate::storage::TestJcodeHome::acquire();
        Self::reset_caches();
        Self { _home: home }
    }

    fn reset_caches() {
        crate::tui::ui::clear_test_render_state_for_tests();
        crate::tui::app::helpers::clear_ambient_info_cache_for_tests();
        crate::auth::claude::set_active_account_override(None);
        crate::auth::codex::set_active_account_override(None);
        crate::auth::AuthStatus::invalidate_cache();
    }
}

impl Drop for TestEnvHandle {
    fn drop(&mut self) {
        Self::reset_caches();
        // _home drops here, restoring JCODE_HOME and releasing the lock.
    }
}

/// Legacy shim retained for the inline call sites in `support_failover::part_02`.
/// Prefer creating a `TestEnvHandle` via `create_test_app()` or constructing
/// one directly in new tests.
fn ensure_test_jcode_home_if_unset() {
    use std::sync::OnceLock;

    static TEST_HOME: OnceLock<std::path::PathBuf> = OnceLock::new();

    if std::env::var_os("JCODE_HOME").is_some() {
        return;
    }

    let path = TEST_HOME.get_or_init(|| {
        let path = std::env::temp_dir().join(format!("jcode-test-home-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&path);
        path
    });
    crate::env::set_var("JCODE_HOME", path);
}

/// Legacy shim retained for the inline call sites in `support_failover::part_02`.
fn clear_persisted_test_ui_state() {
    if let Ok(home) = crate::storage::jcode_dir() {
        let ambient_dir = home.join("ambient");
        let _ = std::fs::remove_file(ambient_dir.join("queue.json"));
        let _ = std::fs::remove_file(ambient_dir.join("state.json"));
        let _ = std::fs::remove_file(ambient_dir.join("directives.json"));
        let _ = std::fs::remove_file(ambient_dir.join("visible_cycle.json"));
    }
    crate::tui::app::helpers::clear_ambient_info_cache_for_tests();
    crate::auth::AuthStatus::invalidate_cache();
}

fn with_temp_jcode_home<T>(f: impl FnOnce() -> T) -> T {
    let _env = TestEnvHandle::acquire();
    f()
}

fn create_jcode_repo_fixture() -> tempfile::TempDir {
    let temp = tempfile::TempDir::new().expect("temp repo");
    std::fs::create_dir_all(temp.path().join(".git")).expect("git dir");
    std::fs::write(
        temp.path().join("Cargo.toml"),
        "[package]\nname = \"jcode\"\nversion = \"0.1.0\"\n",
    )
    .expect("cargo toml");
    temp
}

fn create_real_git_repo_fixture() -> tempfile::TempDir {
    let temp = tempfile::tempdir().expect("tempdir");
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(temp.path())
        .output()
        .expect("git init");
    std::process::Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(temp.path())
        .output()
        .expect("git config email");
    std::process::Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(temp.path())
        .output()
        .expect("git config name");
    std::fs::write(temp.path().join("tracked.txt"), "before\n").expect("write tracked file");
    std::process::Command::new("git")
        .args(["add", "tracked.txt"])
        .current_dir(temp.path())
        .output()
        .expect("git add");
    std::process::Command::new("git")
        .args(["commit", "-m", "init"])
        .current_dir(temp.path())
        .output()
        .expect("git commit");
    temp
}

#[test]
fn test_handle_turn_error_failover_prompt_manual_mode_shows_system_notice() {
    with_temp_jcode_home(|| {
        write_test_config("[provider]\ncross_provider_failover = \"manual\"\n");
        let mut app = create_test_app();
        let prompt = crate::provider::ProviderFailoverPrompt {
            from_provider: "claude".to_string(),
            from_label: "Anthropic".to_string(),
            to_provider: "openai".to_string(),
            to_label: "OpenAI".to_string(),
            reason: "OAuth usage exhausted".to_string(),
            estimated_input_chars: 48_000,
            estimated_input_tokens: 12_000,
        };

        app.handle_turn_error(failover_error_message(&prompt));

        let last = app.display_messages.last().expect("display message");
        assert_eq!(last.role, "system");
        assert!(last.content.contains("did **not** resend your prompt"));
        assert!(last.content.contains("/model"));
        assert!(
            last.content
                .contains("cross_provider_failover = \"manual\"")
        );
        assert!(app.pending_provider_failover.is_none());
    });
}

#[test]
fn test_handle_turn_error_failover_prompt_countdown_can_switch_and_retry() {
    with_temp_jcode_home(|| {
        write_test_config("[provider]\ncross_provider_failover = \"countdown\"\n");
        let (mut app, active_provider) = create_switchable_test_app("claude");
        let prompt = crate::provider::ProviderFailoverPrompt {
            from_provider: "claude".to_string(),
            from_label: "Anthropic".to_string(),
            to_provider: "openai".to_string(),
            to_label: "OpenAI".to_string(),
            reason: "OAuth usage exhausted".to_string(),
            estimated_input_chars: 32_000,
            estimated_input_tokens: 8_000,
        };

        app.handle_turn_error(failover_error_message(&prompt));
        assert!(app.pending_provider_failover.is_some());

        if let Some(pending) = app.pending_provider_failover.as_mut() {
            pending.deadline = Instant::now() - Duration::from_secs(1);
        }
        app.maybe_progress_provider_failover_countdown();

        assert!(app.pending_provider_failover.is_none());
        assert!(app.pending_turn);
        assert_eq!(active_provider.lock().unwrap().as_str(), "openai");
        assert_eq!(app.session.model.as_deref(), Some("gpt-test"));
        let last = app.display_messages.last().expect("display message");
        assert!(
            last.content
                .contains("cross_provider_failover = \"manual\"")
        );
    });
}
