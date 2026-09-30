fn cache_timer_app(provider: &str, model: &str) -> App {
    let mut app = create_test_app();
    app.is_remote = true;
    app.runtime_mode = AppRuntimeMode::RemoteClient;
    app.remote_provider_name = Some(provider.to_string());
    app.remote_provider_model = Some(model.to_string());
    app.remote_resolved_credential = Some(jcode_provider_core::ResolvedCredential::ApiKey);
    app.display_messages.push(DisplayMessage::user("first"));
    app.begin_kv_cache_request(&[Message::user("first")], &[], "system", "");
    app.streaming.streaming_input_tokens = 42_000;
    app.streaming.streaming_cache_read_tokens = Some(40_000);
    app.record_completed_stream_cache_usage();
    app
}

#[test]
fn cache_timer_openai_estimate_never_pushes_cold_warning() {
    let mut app = cache_timer_app("openai", "gpt-4.1");
    app.kv_cache
        .kv_cache_baseline
        .as_mut()
        .unwrap()
        .completed_at = Instant::now() - Duration::from_secs(100_000);
    assert!(!app.maybe_push_idle_cold_cache_warning());
    let status = app.cache_ttl_status().unwrap();
    assert!(status.is_estimate);
    let mut warm_estimate = status;
    warm_estimate.is_cold = false;
    assert!(
        crate::tui::detect_kv_cache_problem(
            "openai",
            None,
            3,
            42_000,
            Some(0),
            Some(40_000),
            Some(&warm_estimate),
        )
        .is_none()
    );
    app.display_messages.push(DisplayMessage::user("second"));
    let count = app.display_messages.len();
    app.begin_kv_cache_request(&[Message::user("second")], &[], "system", "");
    assert_eq!(app.display_messages.len(), count);
    let request = app.kv_cache.pending_kv_cache_request.as_ref().unwrap();
    assert_ne!(
        app.classify_kv_cache_miss_reason(request, request.baseline.as_ref().unwrap(), 0, 0),
        KvCacheMissReason::Expired
    );
}

#[test]
fn cache_warning_does_not_leak_anthropic_expiry_into_openai_route() {
    let mut app = cache_timer_app("anthropic", "claude-opus-4-6");
    app.kv_cache
        .kv_cache_baseline
        .as_mut()
        .unwrap()
        .completed_at = Instant::now() - Duration::from_secs(100_000);
    app.remote_provider_name = Some("openai".to_string());
    app.remote_provider_model = Some("gpt-4.1".to_string());
    assert!(!app.maybe_push_idle_cold_cache_warning());
    assert!(app.cache_ttl_status().is_none());
}

#[test]
fn cache_timer_snapshots_retention_at_request_start() {
    let _guard = super::test_support::lock_test_env();
    crate::provider::anthropic::set_cache_ttl_1h(true);
    let mut app = cache_timer_app("anthropic", "claude-opus-4-6");
    app.begin_kv_cache_request(&[Message::user("second")], &[], "system", "");
    crate::provider::anthropic::set_cache_ttl_1h(false);
    app.record_completed_stream_cache_usage();
    let status = app.cache_ttl_status();
    crate::provider::anthropic::set_cache_ttl_1h(true);
    let status = status.expect("completed request has a cache timer");
    assert_eq!(status.ttl_secs, 3600);
    assert_eq!(status.cached_tokens, Some(82_000));
    app.remote_provider_model = Some("claude-sonnet-4-6".to_string());
    assert!(app.cache_ttl_status().is_none());
}
