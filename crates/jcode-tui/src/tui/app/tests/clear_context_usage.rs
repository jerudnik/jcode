fn seed_stale_clear_usage(app: &mut App) {
    app.streaming.streaming_input_tokens = 40_000;
    app.streaming.streaming_output_tokens = 2_000;
    app.streaming.streaming_cache_read_tokens = Some(30_000);
    app.streaming.streaming_cache_creation_tokens = Some(5_000);
    app.streaming.streaming_context_stale = true;
    app.streaming.streaming_usage_call_reset_pending = true;
    app.kv_cache.current_api_usage_recorded = true;
}

fn assert_clear_usage_reset(app: &App) {
    assert_eq!(app.current_stream_context_tokens(), None);
    assert_eq!(app.streaming.streaming_input_tokens, 0);
    assert_eq!(app.streaming.streaming_output_tokens, 0);
    assert_eq!(app.streaming.streaming_cache_read_tokens, None);
    assert_eq!(app.streaming.streaming_cache_creation_tokens, None);
    assert!(!app.streaming.streaming_context_stale);
    assert!(!app.streaming.streaming_usage_call_reset_pending);
    assert!(!app.kv_cache.current_api_usage_recorded);
}

#[test]
fn local_clear_resets_provider_reported_context_usage() {
    let mut app = create_test_app();
    seed_stale_clear_usage(&mut app);

    assert!(super::commands::handle_session_command(&mut app, "/clear"));

    assert_clear_usage_reset(&app);
}

#[test]
fn remote_clear_resets_provider_reported_context_usage() {
    let mut app = create_test_app();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = rt.enter();
    let mut remote = crate::tui::backend::RemoteConnection::dummy();
    remote.mark_history_loaded();
    app.is_remote = true;
    seed_stale_clear_usage(&mut app);
    app.input = "/clear".to_string();
    app.cursor_pos = app.input.len();

    rt.block_on(app.handle_remote_key(KeyCode::Enter, KeyModifiers::empty(), &mut remote))
        .expect("remote /clear should succeed");

    assert_clear_usage_reset(&app);
}
