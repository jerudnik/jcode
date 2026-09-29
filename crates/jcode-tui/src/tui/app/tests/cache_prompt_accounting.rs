fn cache_accounting_openai_app() -> App {
    let mut app = create_test_app();
    app.is_remote = true;
    app.runtime_mode = AppRuntimeMode::RemoteClient;
    app.remote_provider_name = Some("OpenAI".to_string());
    app.remote_provider_model = Some("gpt-5.6".to_string());
    app.remote_resolved_credential = Some(jcode_provider_core::ResolvedCredential::ApiKey);
    app.cost.cached_price_model = Some("gpt-5.6".to_string());
    app.cost.cached_prompt_price = Some(10.0);
    app.cost.cached_completion_price = Some(40.0);
    app.cost.cached_cache_read_price = Some(1.0);
    app
}

fn cache_accounting_stats(app: &mut App) -> String {
    assert!(super::state_ui::handle_info_command(app, "/cache stats"));
    app.display_messages().last().unwrap().content.clone()
}

#[test]
fn cost_for_usage_openrouter_writes_not_double_billed() {
    let mut app = cache_accounting_openai_app();
    app.remote_provider_name = Some("openrouter".to_string());
    app.accrue_remote_call_cost(10_000, 100, 6_000, 2_000);
    let expected = (2_000.0 * 10.0 + 6_000.0 + 2_000.0 * 10.0 + 100.0 * 40.0) / 1_000_000.0;
    assert!(
        (app.cost.total_cost - expected).abs() < 0.000001,
        "{} != {expected}",
        app.cost.total_cost
    );
}

#[test]
fn cache_accounting_openai_writes_are_subsets_live_and_completed() {
    let mut app = cache_accounting_openai_app();
    app.streaming.streaming_input_tokens = 10_000;
    app.streaming.streaming_cache_read_tokens = Some(6_000);
    app.streaming.streaming_cache_creation_tokens = Some(2_000);
    let live = cache_accounting_stats(&mut app);
    assert!(
        live.contains("cache_read_pct_of_effective_prompt_including_unrecorded_live: 60%"),
        "{live}"
    );
    assert!(
        live.contains("cache_write_pct_of_effective_prompt_including_unrecorded_live: 20%"),
        "{live}"
    );
    assert!(
        live.contains("effective_prompt_tokens_including_unrecorded_live: 10k (10,000)"),
        "{live}"
    );
    assert!(app.record_completed_stream_cache_usage());
    assert_eq!(app.token_accounting.total_cache_prompt_tokens, 10_000);
    assert_eq!(
        app.kv_cache
            .kv_cache_baseline
            .as_ref()
            .unwrap()
            .input_tokens,
        10_000
    );
    let stats = cache_accounting_stats(&mut app);
    assert!(
        stats.contains("cache_read_pct_of_effective_prompt: 60%"),
        "{stats}"
    );
    assert!(
        stats.contains("cache_write_pct_of_effective_prompt: 20%"),
        "{stats}"
    );
    assert!(
        stats.contains(
            "effective_prompt_tokens (input+read+creation for split providers): 10k (10,000)"
        ),
        "{stats}"
    );
    assert!(
        stats.contains("active_route_cache_retention: 30 minutes, provider estimate/minimum"),
        "{stats}"
    );
    let info = app.info_widget_data().cache_hit_info.unwrap();
    assert_eq!(info.prompt_tokens, Some(10_000));
    assert!((info.hit_ratio().unwrap() - 0.6).abs() < 0.0001);
    assert!((info.last_ratio().unwrap() - 0.6).abs() < 0.0001);
}

#[test]
fn cache_accounting_remote_snapshots_count_openai_prompt_once() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = rt.enter();
    let mut remote = crate::tui::backend::RemoteConnection::dummy();
    let mut app = cache_accounting_openai_app();
    for (input, read, write) in [
        (8_000, 4_000, None),
        (10_000, 6_000, Some(2_000)),
        (10_000, 6_000, Some(2_000)),
    ] {
        app.handle_server_event(
            crate::protocol::ServerEvent::TokenUsage {
                input,
                output: 100,
                cache_read_input: Some(read),
                cache_creation_input: write,
            },
            &mut remote,
        );
    }
    assert_eq!(app.token_accounting.total_cache_prompt_tokens, 10_000);
    assert_eq!(
        app.token_accounting.total_cache_reported_input_tokens,
        10_000
    );
    assert_eq!(app.token_accounting.total_cache_read_tokens, 6_000);
    assert_eq!(app.token_accounting.total_cache_creation_tokens, 2_000);
    assert_eq!(app.last_turn_input_tokens, Some(10_000));
    let stats = cache_accounting_stats(&mut app);
    assert!(
        stats.contains("cache_read_pct_of_effective_prompt: 60%"),
        "{stats}"
    );
    assert!(
        stats.contains("cache_write_pct_of_effective_prompt: 20%"),
        "{stats}"
    );
    // Cost follows the final classification of the whole snapshot, not the
    // sum of per-frame deltas: 2k fresh + 6k read + 2k write + 100 output at
    // the test rates ($10 / $1 / $12.5 / $40 per 1M), each token billed once.
    let expected = (2_000.0 * 10.0 + 6_000.0 * 1.0 + 2_000.0 * 12.5 + 100.0 * 40.0) / 1_000_000.0;
    assert!(
        (app.cost.total_cost - expected).abs() < 0.000001,
        "{} != {expected}",
        app.cost.total_cost
    );
}

#[test]
fn cache_accounting_mixed_history_and_live_sum_resolved_prompts() {
    let mut app = cache_accounting_openai_app();
    app.token_accounting.cache_next_optimal_input_tokens = Some(10_000);
    // Historic Anthropic request: input 1k + read 7k + write 2k = 10k.
    app.remote_token_usage_totals = Some(crate::protocol::TokenUsageTotals {
        cache_prompt_tokens: Some(10_000),
        input_tokens: 1_000,
        cache_reported_input_tokens: 1_000,
        cache_read_input_tokens: 7_000,
        cache_creation_input_tokens: 2_000,
        ..Default::default()
    });
    app.streaming.streaming_input_tokens = 10_000;
    app.streaming.streaming_cache_read_tokens = Some(6_000);
    app.streaming.streaming_cache_creation_tokens = Some(2_000);
    assert!(app.record_completed_stream_cache_usage());
    let stats = cache_accounting_stats(&mut app);
    assert!(
        stats.contains(
            "effective_prompt_tokens (input+read+creation for split providers): 20k (20,000)"
        ),
        "{stats}"
    );
    assert!(
        stats.contains("cache_read_pct_of_effective_prompt: 65%"),
        "{stats}"
    );
    assert!(
        stats.contains("cache_write_pct_of_effective_prompt: 20%"),
        "{stats}"
    );
    assert_eq!(
        app.info_widget_data().cache_hit_info.unwrap().prompt_tokens,
        Some(20_000)
    );
    assert!(
        app.info_widget_data()
            .cache_hit_info
            .unwrap()
            .optimal_ratio()
            .is_none()
    );
    assert!(
        stats.contains("cache_read_pct_of_optimal_input: None"),
        "{stats}"
    );
}

#[test]
fn cache_accounting_legacy_history_does_not_guess_from_writes_or_current_provider() {
    let mut app = cache_accounting_openai_app();
    app.remote_token_usage_totals = Some(crate::protocol::TokenUsageTotals {
        input_tokens: 10_000,
        cache_reported_input_tokens: 10_000,
        cache_read_input_tokens: 6_000,
        cache_creation_input_tokens: 2_000,
        ..Default::default()
    });
    let stats = cache_accounting_stats(&mut app);
    assert!(
        stats.contains("cache_read_pct_of_effective_prompt: unknown"),
        "{stats}"
    );
    assert!(
        stats.contains("total_cache_read_tokens: 6k (6,000)"),
        "{stats}"
    );
    assert!(
        app.info_widget_data()
            .cache_hit_info
            .unwrap()
            .hit_ratio()
            .is_none()
    );
    app.remote_resolved_credential = Some(jcode_provider_core::ResolvedCredential::Oauth);
    let stats = cache_accounting_stats(&mut app);
    assert!(
        stats.contains("active_route_cache_retention: unknown, provider-managed retention"),
        "{stats}"
    );
}

#[test]
fn cache_accounting_fully_cached_anthropic_has_nonzero_denominator() {
    let mut app = create_test_app();
    app.is_remote = true;
    app.runtime_mode = AppRuntimeMode::RemoteClient;
    app.remote_provider_name = Some("Anthropic".to_string());
    app.streaming.streaming_input_tokens = 0;
    app.streaming.streaming_cache_read_tokens = Some(10_000);
    app.streaming.streaming_cache_creation_tokens = Some(0);
    assert!(app.record_completed_stream_cache_usage());
    assert_eq!(app.token_accounting.total_cache_prompt_tokens, 10_000);
    let stats = cache_accounting_stats(&mut app);
    assert!(
        stats.contains("cache_read_pct_of_effective_prompt: 100%"),
        "{stats}"
    );
}

#[test]
fn cache_accounting_missing_last_read_remains_unknown() {
    let mut app = cache_accounting_openai_app();
    app.streaming.streaming_input_tokens = 10_000;
    app.streaming.streaming_cache_creation_tokens = Some(2_000);
    assert!(app.record_completed_stream_cache_usage());
    assert_eq!(app.token_accounting.last_cache_read_tokens, None);
    assert_eq!(app.token_accounting.last_cache_creation_tokens, Some(2_000));
    assert!(
        app.info_widget_data()
            .cache_hit_info
            .unwrap()
            .last_ratio()
            .is_none()
    );
}

#[test]
fn cache_accounting_write_only_session_marks_read_ratio_unknown() {
    let mut app = cache_accounting_openai_app();
    app.streaming.streaming_input_tokens = 10_000;
    app.streaming.streaming_cache_creation_tokens = Some(2_000);
    assert!(
        cache_accounting_stats(&mut app).contains(super::state_ui::INCOMPLETE_CACHE_READ_RATIO)
    );
    app.record_completed_stream_cache_usage();
    let info = app.info_widget_data().cache_hit_info.unwrap();
    assert_eq!(info.hit_ratio(), None);
    assert_eq!(info.last_read_tokens, None);
    assert_eq!(info.creation_tokens, 2_000);
    assert!(
        cache_accounting_stats(&mut app).contains(super::state_ui::INCOMPLETE_CACHE_READ_RATIO)
    );

    app.begin_kv_cache_request(&[Message::user("next")], &[], "system", "");
    app.streaming.streaming_cache_read_tokens = Some(6_000);
    app.streaming.streaming_cache_creation_tokens = Some(0);
    app.record_completed_stream_cache_usage();
    let info = app.info_widget_data().cache_hit_info.unwrap();
    assert_eq!(info.hit_ratio(), None);
    assert_eq!(info.last_ratio(), Some(0.6));
    assert_eq!(info.read_tokens, 6_000);
    assert!(
        cache_accounting_stats(&mut app).contains(super::state_ui::INCOMPLETE_CACHE_READ_RATIO)
    );

    let mut known = cache_accounting_openai_app();
    known.streaming.streaming_input_tokens = 10_000;
    known.streaming.streaming_cache_read_tokens = Some(0);
    known.streaming.streaming_cache_creation_tokens = Some(2_000);
    known.record_completed_stream_cache_usage();
    let info = known.info_widget_data().cache_hit_info.unwrap();
    assert_eq!(info.hit_ratio(), Some(0.0));
    assert_eq!(info.last_read_tokens, Some(0));
    assert!(
        !cache_accounting_stats(&mut known).contains(super::state_ui::INCOMPLETE_CACHE_READ_RATIO)
    );
}

#[test]
fn cache_accounting_history_missing_reads_preserves_unknown_ratio() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = rt.enter();
    let mut app = cache_accounting_openai_app();
    app.remote_token_usage_totals = Some(
        serde_json::from_value(serde_json::json!({
            "cache_read_complete": false,
            "messages_with_token_usage": 1,
            "input_tokens": 10_000,
            "output_tokens": 100,
            "cache_prompt_tokens": 10_000,
            "cache_reported_input_tokens": 10_000,
            "cache_read_input_tokens": 0,
            "cache_creation_input_tokens": 2_000
        }))
        .unwrap(),
    );
    let info = app.info_widget_data().cache_hit_info.unwrap();
    assert_eq!(info.hit_ratio(), None);
    assert_eq!(info.creation_tokens, 2_000);
    assert!(
        cache_accounting_stats(&mut app).contains(super::state_ui::INCOMPLETE_CACHE_READ_RATIO)
    );
    app.remote_token_usage_totals = None;
    app.streaming.streaming_input_tokens = 10_000;
    app.record_completed_stream_cache_usage();
    assert!(app.info_widget_data().cache_hit_info.is_none());
    let mut remote = crate::tui::backend::RemoteConnection::dummy();
    app.handle_server_event(
        crate::protocol::ServerEvent::TokenUsage {
            input: 10_000,
            output: 100,
            cache_read_input: None,
            cache_creation_input: Some(2_000),
        },
        &mut remote,
    );
    assert_eq!(
        app.info_widget_data().cache_hit_info.unwrap().hit_ratio(),
        None
    );
}
