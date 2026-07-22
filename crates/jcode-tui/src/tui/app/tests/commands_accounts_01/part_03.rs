#[test]
fn test_fast_on_while_processing_mentions_next_request_locally() {
    let mut app = create_fast_test_app();
    app.is_processing = true;
    app.input = "/fast on".to_string();

    app.submit_input();

    let last = app
        .display_messages()
        .last()
        .expect("missing fast mode response");
    assert_eq!(last.role, "system");
    assert_eq!(
        last.content,
        "✓ Fast mode on (Fast)\nApplies to the next request/turn. The current in-flight request keeps its existing tier."
    );
    assert_eq!(
        app.status_notice(),
        Some("Fast: on (next request)".to_string())
    );
}
