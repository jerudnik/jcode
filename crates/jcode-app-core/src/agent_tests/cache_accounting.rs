use super::*;

#[tokio::test]
async fn stored_prompt_tokens_none_for_unknown_provider_with_cache_counters() {
    let _guard = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().unwrap();
    let _home = ScopedEnvVar::set("JCODE_HOME", temp.path());
    let _telemetry = ScopedEnvVar::set("JCODE_NO_TELEMETRY", "1");

    for streaming in [false, true] {
        for (read, write, expected) in [
            (Some(6_000), Some(2_000), None),
            (Some(0), Some(0), Some(10_000)),
            (None, None, Some(10_000)),
        ] {
            let mut agent = scripted_agent(vec![
                ScriptedProviderEvent::Event(StreamEvent::TextDelta("answer".to_string())),
                ScriptedProviderEvent::Event(StreamEvent::TokenUsage {
                    input_tokens: Some(10_000),
                    output_tokens: Some(3),
                    cache_read_input_tokens: read,
                    cache_creation_input_tokens: write,
                }),
                ScriptedProviderEvent::Event(StreamEvent::MessageEnd {
                    stop_reason: Some("end_turn".to_string()),
                }),
            ])
            .await;
            if streaming {
                let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
                agent
                    .run_once_streaming_mpsc("prompt", Vec::new(), None, tx)
                    .await
                    .unwrap();
            } else {
                assert_eq!(agent.run_once_capture("prompt").await.unwrap(), "answer");
            }
            let saved = crate::session::Session::load(agent.session_id()).unwrap();
            let usage = saved
                .messages
                .iter()
                .find_map(|message| message.token_usage.as_ref())
                .unwrap();
            assert_eq!(
                usage.prompt_tokens, expected,
                "streaming={streaming}, read={read:?}, write={write:?}"
            );
            assert_eq!(usage.cache_read_input_tokens, read);
            assert_eq!(usage.cache_creation_input_tokens, write);
            let persisted = serde_json::to_value(usage).unwrap();
            assert_eq!(persisted["provider"], agent.provider_name());
            assert_eq!(persisted["model"], agent.provider_model());
        }
    }
}
