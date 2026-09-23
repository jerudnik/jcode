use super::*;

#[test]
fn parses_official_version_without_inventing_a_minimum() {
    assert_eq!(parse_version(b"2.1.275 (Claude Code)\n"), Some("2.1.275"));
    assert_eq!(parse_version(b"2.1.123 (Claude Code)\r\n"), Some("2.1.123"));
    assert_eq!(parse_version(b"2.1.275 (another CLI)"), None);
}

#[test]
fn rejects_malformed_version_output() {
    assert_eq!(parse_version(b"2.1 (Claude Code)"), None);
    assert_eq!(parse_version(b"2.1.275\nInjected: yes (Claude Code)"), None);
    assert_eq!(parse_version(b"\xff (Claude Code)"), None);
}

#[test]
fn identity_fields_share_the_selected_version() {
    let identity = ClaudeCliIdentity::new("2.1.275");
    assert_eq!(identity.version, "2.1.275");
    assert_eq!(
        identity.user_agent,
        "claude-cli/2.1.275 (external, sdk-cli)"
    );
    assert_eq!(
        identity.billing_header,
        "cc_version=2.1.275; cc_entrypoint=sdk-cli; cch=33f85;"
    );
}

#[cfg(unix)]
fn shell(script: &str) -> Command {
    let mut command = Command::new("/bin/sh");
    command.args(["-c", script]);
    command
}

#[cfg(unix)]
#[tokio::test]
async fn command_success_requires_a_zero_exit_status() {
    assert_eq!(
        detect_version(
            shell("printf '2.1.275 (Claude Code)\\n'"),
            DETECTION_TIMEOUT
        )
        .await,
        Some("2.1.275".into())
    );
    assert_eq!(
        detect_version(
            shell("printf '2.1.275 (Claude Code)\\n'; exit 1"),
            DETECTION_TIMEOUT
        )
        .await,
        None
    );
    assert_eq!(
        detect_version(
            Command::new("/nonexistent/jcode-claude-version-test"),
            DETECTION_TIMEOUT
        )
        .await,
        None
    );
}

#[cfg(unix)]
#[tokio::test]
async fn command_timeout_and_output_limit_are_bounded() {
    let result = tokio::time::timeout(
        Duration::from_secs(1),
        detect_version(shell("exec sleep 10"), Duration::from_millis(30)),
    )
    .await
    .expect("detection exceeded its deadline");
    assert_eq!(result, None);
    assert_eq!(
        detect_version(
            shell("printf '2.1.275 (Claude Code)%1100s' ''"),
            DETECTION_TIMEOUT
        )
        .await,
        None
    );
}

#[cfg(unix)]
#[tokio::test]
async fn failed_detection_uses_reviewed_fallback_for_every_field() {
    let identity = detect_identity(shell("printf 'invalid version'"), DETECTION_TIMEOUT).await;
    assert_eq!(identity.version, FALLBACK_VERSION);
    assert_eq!(
        identity.user_agent,
        format!("claude-cli/{FALLBACK_VERSION} (external, sdk-cli)")
    );
    assert_eq!(
        identity.billing_header,
        format!("cc_version={FALLBACK_VERSION}; cc_entrypoint=sdk-cli; cch=33f85;")
    );
}
