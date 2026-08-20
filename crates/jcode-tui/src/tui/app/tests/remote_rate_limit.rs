#[test]
fn test_rate_limit_retry_grows_and_stops_at_cap_preserving_message() {
    let mut app = create_test_app();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = rt.enter();
    let mut remote = crate::tui::backend::RemoteConnection::dummy();
    let message = "preserve this rate-limited prompt";
    app.rate_limit_pending_message = Some(PendingRemoteMessage {
        content: message.to_string(),
        images: vec![],
        is_system: false,
        system_reminder: None,
        auto_retry: false,
        retry_attempts: 0,
        retry_at: None,
    });
    app.is_processing = true;
    app.status = ProcessingStatus::Streaming;

    for attempt in 1..=8 {
        app.handle_server_event(
            crate::protocol::ServerEvent::Error {
                id: attempt,
                message: "rate limit exceeded".to_string(),
                retry_after_secs: Some(1),
            },
            &mut remote,
        );

        let pending = app
            .rate_limit_pending_message
            .as_ref()
            .expect("rate-limit retry should remain pending before the cap");
        assert_eq!(pending.retry_attempts, attempt as u8);
        assert!(pending.retry_at.is_some());
        assert!(pending.auto_retry);
        assert_eq!(pending.content, message);
        app.is_processing = true;
    }

    app.handle_server_event(
        crate::protocol::ServerEvent::Error {
            id: 9,
            message: "rate limit exceeded".to_string(),
            retry_after_secs: Some(1),
        },
        &mut remote,
    );

    let pending = app
        .rate_limit_pending_message
        .as_ref()
        .expect("the capped message must remain available for manual retry");
    assert_eq!(pending.retry_attempts, 8);
    assert_eq!(pending.content, message);
    assert!(!pending.auto_retry);
    assert!(app.rate_limit_reset.is_none());
    assert!(app.display_messages().iter().any(|m| {
        m.role == "error"
            && m.content.contains("Auto-retry limit reached")
            && m.content.contains(message)
    }));
}
