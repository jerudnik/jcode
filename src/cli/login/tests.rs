use super::*;

fn set_or_clear_env(key: &str, value: Option<std::ffi::OsString>) {
    if let Some(value) = value {
        crate::env::set_var(key, value);
    } else {
        crate::env::remove_var(key);
    }
}

#[test]
fn scriptable_resume_command_matches_input_kind() {
    assert_eq!(
        scriptable_resume_command("openai", "callback_url"),
        "jcode login --provider openai --callback-url '<url-or-query>'"
    );
    assert_eq!(
        scriptable_resume_command("gemini", "auth_code"),
        "jcode login --provider gemini --auth-code '<code>'"
    );
    assert_eq!(
        scriptable_resume_command("copilot", "complete"),
        "jcode login --provider copilot --complete"
    );
}

#[test]
fn load_pending_login_removes_expired_record() {
    let _guard = crate::storage::lock_test_env();
    let temp = tempfile::TempDir::new().expect("temp dir");
    let prev_home = std::env::var_os("JCODE_HOME");
    crate::env::set_var("JCODE_HOME", temp.path());

    let path = pending_login_path("openai").expect("pending path");
    let record = PendingScriptableLoginRecord {
        expires_at_ms: current_time_ms() - 1,
        login: PendingScriptableLogin::Openai {
            account_label: "default".to_string(),
            verifier: "verifier".to_string(),
            state: "state".to_string(),
            redirect_uri: "http://localhost:1455/auth/callback".to_string(),
        },
    };
    crate::storage::write_json_secret(&path, &record).expect("write pending login");

    let err = load_pending_login(&path, "openai").expect_err("expected expired state");
    assert!(err.to_string().contains("expired"));
    assert!(!path.exists(), "expired pending login should be removed");

    set_or_clear_env("JCODE_HOME", prev_home);
}

#[test]
fn load_pending_login_accepts_legacy_format() {
    let _guard = crate::storage::lock_test_env();
    let temp = tempfile::TempDir::new().expect("temp dir");
    let prev_home = std::env::var_os("JCODE_HOME");
    crate::env::set_var("JCODE_HOME", temp.path());

    let path = pending_login_path("gemini").expect("pending path");
    let legacy = PendingScriptableLogin::Gemini {
        verifier: "verifier".to_string(),
        redirect_uri: auth::gemini::GEMINI_MANUAL_REDIRECT_URI.to_string(),
    };
    crate::storage::write_json_secret(&path, &legacy).expect("write legacy pending login");

    let loaded = load_pending_login(&path, "gemini").expect("load legacy pending login");
    match loaded {
        PendingScriptableLogin::Gemini {
            verifier,
            redirect_uri,
        } => {
            assert_eq!(verifier, "verifier");
            assert_eq!(redirect_uri, auth::gemini::GEMINI_MANUAL_REDIRECT_URI);
        }
        other => panic!("unexpected login variant: {:?}", other),
    }

    set_or_clear_env("JCODE_HOME", prev_home);
}

#[test]
fn uses_scriptable_flow_detects_dash_input_without_consuming_stdin() {
    let options = LoginOptions {
        callback_url: Some("-".to_string()),
        ..LoginOptions::default()
    };
    assert!(
        options
            .uses_scriptable_flow()
            .expect("uses scriptable flow")
    );
    assert!(options.has_provided_input());
}

#[test]
fn auto_scriptable_flow_reason_prefers_non_interactive_for_oauth_provider() {
    let provider =
        crate::provider_catalog::resolve_login_provider("openai").expect("resolve openai provider");
    let reason = auto_scriptable_flow_reason(provider, &LoginOptions::default(), false);
    assert_eq!(reason, Some("non_interactive_terminal"));
}

#[test]
fn grok_direct_uses_scriptable_device_flow_without_terminal_or_browser() {
    let provider = crate::provider_catalog::resolve_login_provider("grok-direct")
        .expect("resolve Grok Direct provider");
    assert_eq!(
        auto_scriptable_flow_reason(provider, &LoginOptions::default(), false),
        Some("non_interactive_terminal")
    );
    assert_eq!(
        auto_scriptable_flow_reason(
            provider,
            &LoginOptions {
                no_browser: true,
                ..LoginOptions::default()
            },
            true,
        ),
        Some("no_browser_requested")
    );
}

#[test]
fn grok_direct_pending_login_has_independent_storage_key() {
    let pending = PendingScriptableLogin::GrokDirect {
        device_code: "device".to_string(),
        user_code: "CODE".to_string(),
        verification_uri: "https://auth.x.ai/activate".to_string(),
        verification_uri_complete: None,
        expires_in: Some(600),
        interval: 5,
    };
    assert_eq!(pending.key(), "grok-direct");
    assert_ne!(pending.key(), "kimi");
    let serialized = serde_json::to_value(&pending).expect("serialize pending Grok Direct login");
    assert_eq!(serialized["provider"], "grok-direct");
}

#[test]
fn grok_direct_terminal_poll_failures_are_distinguished_from_transient_errors() {
    for message in [
        "Grok Direct device authorization was denied",
        "Grok Direct device authorization expired before login completed",
        "Grok Direct device authorization was cancelled",
    ] {
        assert!(grok_direct_poll_error_is_terminal(&anyhow::anyhow!(
            message
        )));
    }
    assert!(!grok_direct_poll_error_is_terminal(&anyhow::anyhow!(
        "failed to poll Grok Direct device authorization: connection reset"
    )));
}

#[test]
fn auto_scriptable_flow_reason_uses_no_browser_reason_when_requested() {
    let provider =
        crate::provider_catalog::resolve_login_provider("claude").expect("resolve claude provider");
    let reason = auto_scriptable_flow_reason(
        provider,
        &LoginOptions {
            no_browser: true,
            ..LoginOptions::default()
        },
        true,
    );
    assert_eq!(reason, Some("no_browser_requested"));
}

#[test]
fn auto_scriptable_flow_reason_skips_api_key_only_provider() {
    let provider = crate::provider_catalog::resolve_login_provider("openrouter")
        .expect("resolve openrouter provider");
    let reason = auto_scriptable_flow_reason(provider, &LoginOptions::default(), false);
    assert_eq!(reason, None);
}

#[test]
fn auto_scriptable_flow_reason_skips_when_scriptable_input_already_explicit() {
    let provider =
        crate::provider_catalog::resolve_login_provider("openai").expect("resolve openai provider");
    let reason = auto_scriptable_flow_reason(
        provider,
        &LoginOptions {
            print_auth_url: true,
            ..LoginOptions::default()
        },
        false,
    );
    assert_eq!(reason, None);
}

#[test]
fn grok_build_no_browser_selects_cli_device_auth() {
    assert_eq!(grok_build_login_args(false), ["login"]);
    assert_eq!(grok_build_login_args(true), ["login", "--device-auth"]);
}

#[test]
fn minimax_region_choices_are_canonicalized() {
    assert_eq!(
        canonical_minimax_region("international").unwrap(),
        "international"
    );
    assert_eq!(canonical_minimax_region("global").unwrap(), "international");
    assert_eq!(canonical_minimax_region("io").unwrap(), "international");
    assert_eq!(canonical_minimax_region("china").unwrap(), "china");
    assert_eq!(canonical_minimax_region("cn").unwrap(), "china");
    assert!(canonical_minimax_region("auto").is_err());
}
