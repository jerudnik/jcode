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
