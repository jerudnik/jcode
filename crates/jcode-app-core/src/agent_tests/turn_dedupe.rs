use super::*;

fn user_message_count(agent: &Agent, text: &str) -> usize {
    agent
        .messages()
        .iter()
        .filter(|message| {
            message.role == Role::User
                && message.content.iter().any(|block| {
                    matches!(block, ContentBlock::Text { text: value, .. } if value == text)
                })
        })
        .count()
}

#[tokio::test]
async fn existing_user_message_reuses_only_matching_rate_limited_turn() {
    let _guard = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("temp JCODE_HOME");
    let _home = ScopedEnvVar::set("JCODE_HOME", temp.path());
    let _telemetry = ScopedEnvVar::set("JCODE_NO_TELEMETRY", "1");

    let prompt = "same rate-limited prompt";
    let mut agent = scripted_agent(vec![
        ScriptedProviderEvent::Event(StreamEvent::TextDelta("answer".to_string())),
        ScriptedProviderEvent::Event(StreamEvent::MessageEnd {
            stop_reason: Some("end_turn".to_string()),
        }),
    ])
    .await;
    agent.add_message(
        Role::User,
        vec![ContentBlock::Text {
            text: prompt.to_string(),
            cache_control: None,
        }],
    );
    let (event_tx, _event_rx) = tokio::sync::mpsc::unbounded_channel();

    agent
        .run_once_streaming_mpsc_with_existing_user_message(
            prompt,
            Vec::new(),
            None,
            event_tx.clone(),
            true,
        )
        .await
        .expect("matching retry should succeed");
    assert_eq!(
        user_message_count(&agent, prompt),
        1,
        "matching retry must reuse the stored user turn"
    );

    agent
        .run_once_streaming_mpsc_with_existing_user_message(
            prompt,
            Vec::new(),
            None,
            event_tx.clone(),
            false,
        )
        .await
        .expect("explicit non-reuse should succeed");
    assert_eq!(
        user_message_count(&agent, prompt),
        2,
        "reuse=false must append a new user turn"
    );

    agent
        .run_once_streaming_mpsc_with_existing_user_message(
            "different prompt",
            Vec::new(),
            None,
            event_tx,
            true,
        )
        .await
        .expect("different prompt should succeed");
    assert_eq!(
        user_message_count(&agent, "different prompt"),
        1,
        "a different body must append even when reuse is enabled"
    );
}
