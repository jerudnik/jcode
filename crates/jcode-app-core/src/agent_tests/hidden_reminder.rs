use super::*;

/// A hidden continuation (post-reload resume) carries its payload in the system
/// reminder and passes an empty user message, so it must never persist an empty
/// user text block that would reach the provider as a blank user turn.
struct HiddenReminderTurn {
    _home: tempfile::TempDir,
    agent: Agent,
}

impl HiddenReminderTurn {
    async fn new() -> Self {
        let temp = tempfile::tempdir().expect("temp JCODE_HOME");
        let agent = scripted_agent(vec![
            ScriptedProviderEvent::Event(StreamEvent::TextDelta("continuing".to_string())),
            ScriptedProviderEvent::Event(StreamEvent::MessageEnd {
                stop_reason: Some("end_turn".to_string()),
            }),
        ])
        .await;
        Self { _home: temp, agent }
    }

    /// Run the continuation exactly as the reload path does: empty user message,
    /// no images, payload in the system reminder.
    async fn run(&mut self, reminder: &str) {
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        self.agent
            .run_once_streaming_mpsc("", Vec::new(), Some(reminder.to_string()), tx)
            .await
            .expect("hidden reminder turn should succeed");
    }

    fn user_messages(&self) -> impl Iterator<Item = &jcode_session_types::StoredMessage> {
        self.agent
            .messages()
            .iter()
            .filter(|message| message.role == crate::message::Role::User)
    }
}

#[tokio::test]
async fn hidden_reminder_turn_does_not_store_empty_user_text_block() {
    let _guard = crate::storage::lock_test_env();
    let _telemetry = ScopedEnvVar::set("JCODE_NO_TELEMETRY", "1");
    let mut turn = HiddenReminderTurn::new().await;
    let _home = ScopedEnvVar::set("JCODE_HOME", turn._home.path());

    turn.run("Reload complete - continue your work.").await;

    let empty_blocks = turn
        .user_messages()
        .flat_map(|message| message.content.iter())
        .filter(|block| matches!(block, ContentBlock::Text { text, .. } if text.is_empty()))
        .count();
    assert_eq!(
        empty_blocks, 0,
        "hidden reminder turn must not store an empty user text block"
    );
    assert!(
        turn.agent
            .messages()
            .iter()
            .any(|message| message.role == crate::message::Role::Assistant),
        "the turn must still run and produce an assistant reply"
    );
}

/// Dropping the empty text block must not leave a user message with no content:
/// providers reject an empty content array.
#[tokio::test]
async fn hidden_reminder_turn_stores_no_contentless_user_message() {
    let _guard = crate::storage::lock_test_env();
    let _telemetry = ScopedEnvVar::set("JCODE_NO_TELEMETRY", "1");
    let mut turn = HiddenReminderTurn::new().await;
    let _home = ScopedEnvVar::set("JCODE_HOME", turn._home.path());

    turn.run("continue").await;

    assert_eq!(
        turn.user_messages()
            .filter(|message| message.content.is_empty())
            .count(),
        0,
        "a user message with an empty content array would be rejected by providers"
    );
}

/// Resuming while idle leaves the transcript ending on an assistant message.
/// Anthropic reads a trailing assistant message as prefill and continues it
/// instead of starting a new turn, so the continuation must still send a user
/// message; it carries the reminder text rather than a blank string.
#[tokio::test]
async fn hidden_reminder_after_assistant_message_sends_reminder_as_user_text() {
    let _guard = crate::storage::lock_test_env();
    let _telemetry = ScopedEnvVar::set("JCODE_NO_TELEMETRY", "1");
    let mut turn = HiddenReminderTurn::new().await;
    let _home = ScopedEnvVar::set("JCODE_HOME", turn._home.path());
    turn.agent.add_message(
        crate::message::Role::Assistant,
        vec![ContentBlock::Text {
            text: "All done, let me know what is next.".to_string(),
            cache_control: None,
        }],
    );
    let before = turn.agent.messages().len();

    turn.run("Reload complete - continue your work.").await;

    let user_after: Vec<_> = turn
        .agent
        .messages()
        .iter()
        .skip(before)
        .filter(|message| message.role == crate::message::Role::User)
        .collect();
    assert_eq!(
        user_after.len(),
        1,
        "a user message must terminate the assistant turn, or the model treats it as prefill"
    );
    let text = user_after[0]
        .content
        .iter()
        .find_map(|block| match block {
            ContentBlock::Text { text, .. } => Some(text.clone()),
            _ => None,
        })
        .expect("the user message should carry text");
    assert!(
        text.contains("Reload complete"),
        "the user text should carry the reminder payload, got {text:?}"
    );
}

/// Resuming mid-turn leaves the transcript ending on a user tool_result. There
/// the user message is not load-bearing, so no extra user turn is invented.
#[tokio::test]
async fn hidden_reminder_after_user_message_adds_no_user_turn() {
    let _guard = crate::storage::lock_test_env();
    let _telemetry = ScopedEnvVar::set("JCODE_NO_TELEMETRY", "1");
    let mut turn = HiddenReminderTurn::new().await;
    let _home = ScopedEnvVar::set("JCODE_HOME", turn._home.path());
    turn.agent.add_message(
        crate::message::Role::User,
        vec![ContentBlock::Text {
            text: "tool output stands in for a tool_result here".to_string(),
            cache_control: None,
        }],
    );
    let before = turn.agent.messages().len();

    turn.run("Reload complete - continue your work.").await;

    assert_eq!(
        turn.agent
            .messages()
            .iter()
            .skip(before)
            .filter(|message| message.role == crate::message::Role::User)
            .count(),
        0,
        "the transcript already ends on a user message, so none should be added"
    );
}
