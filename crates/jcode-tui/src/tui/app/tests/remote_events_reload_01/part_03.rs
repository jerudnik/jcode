#[test]
fn test_pending_startup_notice_survives_history_bootstrap_for_fresh_session() {
    let mut app = create_test_app();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = rt.enter();
    let mut remote = crate::tui::backend::RemoteConnection::dummy();

    // A fresh client has no remote session yet; the startup notice card is
    // pushed before the History bootstrap arrives.
    app.remote_session_id = None;
    app.set_pending_startup_notice("Launch hotkeys", "cmd+; -> home\ncmd+' -> last project");
    assert!(
        app.display_messages()
            .iter()
            .any(|m| m.content.contains("cmd+;")),
        "card should be visible before bootstrap"
    );

    // The bootstrap for a brand-new session clears the transcript.
    app.handle_server_event(
        crate::protocol::ServerEvent::History {
            id: 1,
            session_id: "session_new".to_string(),
            messages: vec![],
            images: vec![],
            provider_name: Some("claude".to_string()),
            provider_model: Some("claude-sonnet-4-20250514".to_string()),
            subagent_model: None,
            autoreview_enabled: None,
            autojudge_enabled: None,
            available_models: vec![],
            available_model_routes: vec![],
            mcp_servers: vec![],
            skills: vec![],
            total_tokens: None,
            token_usage_totals: None,
            all_sessions: vec![],
            client_count: None,
            is_canary: None,
            reload_recovery: None,
            server_version: None,
            server_name: None,
            server_icon: None,
            server_has_update: None,
            was_interrupted: None,
            connection_type: None,
            status_detail: None,
            upstream_provider: None,
            resolved_credential: None,
            reasoning_effort: None,
            service_tier: None,
            compaction_mode: crate::config::CompactionMode::Reactive,
            activity: None,
            side_panel: crate::side_panel::SidePanelSnapshot::default(),
        },
        &mut remote,
    );

    // The card must still be present on the idle screen after the bootstrap.
    let card_count = app
        .display_messages()
        .iter()
        .filter(|m| m.content.contains("cmd+;"))
        .count();
    assert_eq!(
        card_count, 1,
        "startup notice should be re-applied exactly once after bootstrap"
    );
}

/// A minimal session-changing `History` payload for `session_id`.
fn history_event_for_session_change(session_id: &str) -> crate::protocol::ServerEvent {
    crate::protocol::ServerEvent::History {
        id: 1,
        session_id: session_id.to_string(),
        messages: vec![],
        images: vec![],
        provider_name: Some("claude".to_string()),
        provider_model: Some("claude-sonnet-4-20250514".to_string()),
        subagent_model: None,
        autoreview_enabled: None,
        autojudge_enabled: None,
        available_models: vec![],
        available_model_routes: vec![],
        mcp_servers: vec![],
        skills: vec![],
        total_tokens: None,
        token_usage_totals: None,
        all_sessions: vec![],
        client_count: None,
        is_canary: None,
        reload_recovery: None,
        server_version: None,
        server_name: None,
        server_icon: None,
        server_has_update: None,
        was_interrupted: None,
        connection_type: None,
        status_detail: None,
        upstream_provider: None,
        resolved_credential: None,
        reasoning_effort: None,
        service_tier: None,
        compaction_mode: crate::config::CompactionMode::Reactive,
        activity: None,
        side_panel: crate::side_panel::SidePanelSnapshot::default(),
    }
}

/// A committed (non-preview) diagram must not survive a session switch either.
///
/// The streaming-preview test above covers the ephemeral in-flight diagram,
/// which the handler clears outright. A diagram from a message that finished
/// rendering lives in the process-global ACTIVE_DIAGRAMS registry instead, and
/// that registry had no session binding: session A's diagram stayed in the
/// pinned pane counter, in Ctrl+arrow cycling, and in `get_active_diagrams`
/// (the Margin info widget source) after switching to session B, with no
/// transcript message behind it.
///
/// It is hidden by scope rather than dropped, so switching back re-reveals it
/// without a re-render; body-cache prefix reuse skips re-rendering retained
/// messages, so a cleared entry would never re-register.
#[test]
fn test_handle_server_event_history_session_change_hides_committed_diagram() {
    let _render_lock = scroll_render_test_lock();
    let mut app = create_test_app();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = rt.enter();
    let mut remote = crate::tui::backend::RemoteConnection::dummy();

    crate::tui::mermaid::clear_active_diagrams();

    // Session A renders a diagram. `draw` binds the registry to the drawn
    // session; this test registers directly, so bind session A as draw would.
    app.remote_session_id = Some("session_diagram_a".to_string());
    crate::tui::mermaid::bind_diagram_scope(Some("session_diagram_a"));
    let committed_hash: u64 = 0xDEAD_BEEF_5EAF_0100;
    crate::tui::mermaid::register_active_diagram(committed_hash, 320, 240, None);
    assert!(
        crate::tui::mermaid::get_active_diagrams()
            .iter()
            .any(|d| d.hash == committed_hash),
        "test setup: session A's diagram is visible before the switch"
    );

    app.handle_server_event(
        history_event_for_session_change("session_diagram_b"),
        &mut remote,
    );

    assert_eq!(app.remote_session_id.as_deref(), Some("session_diagram_b"));
    assert!(
        !crate::tui::mermaid::get_active_diagrams()
            .iter()
            .any(|d| d.hash == committed_hash),
        "committed diagram leaked across a session-changing History event"
    );

    // Switching back restores it, with nothing re-rendering it in between.
    app.handle_server_event(
        history_event_for_session_change("session_diagram_a"),
        &mut remote,
    );
    assert!(
        crate::tui::mermaid::get_active_diagrams()
            .iter()
            .any(|d| d.hash == committed_hash),
        "switching back must restore session A's diagram without a re-render"
    );

    crate::tui::mermaid::clear_active_diagrams();
    crate::tui::mermaid::bind_diagram_scope(None);
}
