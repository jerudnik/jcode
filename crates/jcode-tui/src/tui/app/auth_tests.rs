use super::test_support::with_temp_jcode_home;
use super::{
    App, antigravity_input_requires_state_validation, jcode_subscription_tier_label,
    save_tui_openai_compatible_api_base, save_tui_openai_compatible_key,
};

#[test]
fn antigravity_auto_callback_code_skips_manual_callback_parser() {
    assert!(!antigravity_input_requires_state_validation(
        "raw_authorization_code",
        Some("expected_state")
    ));
}

#[test]
fn antigravity_manual_callback_url_keeps_state_validation() {
    assert!(antigravity_input_requires_state_validation(
        "http://127.0.0.1:51121/oauth-callback?code=abc&state=expected_state",
        Some("expected_state")
    ));
}

#[test]
fn jcode_subscription_tier_label_uses_tier_truth_not_raw_tier() {
    let denied = crate::subscription_catalog::classify_live_tier(Some("flagship"), "inactive");
    assert_eq!(
        jcode_subscription_tier_label(&denied),
        "unknown-denied (inactive subscription status)"
    );

    let accepted = crate::subscription_catalog::classify_live_tier(Some("flagship"), "active");
    assert_eq!(jcode_subscription_tier_label(&accepted), "Flagship");
}

#[test]
fn oauth_preflight_mentions_browser_fallback_and_doctor() {
    let message = App::record_oauth_preflight("openai", false, Some("localhost:1455"), Some(true));
    assert!(message.contains("could not open a browser"));
    assert!(message.contains("auth doctor openai"));
}

#[test]
fn oauth_preflight_mentions_manual_safe_callback_mode() {
    let message = App::record_oauth_preflight(
        "gemini",
        true,
        Some("http://127.0.0.1:0/oauth2callback"),
        Some(false),
    );
    assert!(message.contains("manual-safe paste completion"));
    assert!(message.contains("oauth2callback"));
}

#[test]
fn tui_openai_compatible_api_base_accepts_localhost_override() -> anyhow::Result<()> {
    with_temp_jcode_home(|| {
        let resolved = save_tui_openai_compatible_api_base("http://localhost:11434/v1")?;
        assert_eq!(resolved.api_base, "http://localhost:11434/v1");
        assert!(!resolved.requires_api_key);
        Ok(())
    })
}

#[test]
fn tui_openai_compatible_api_base_keeps_jcode_docs_and_remote_endpoint() -> anyhow::Result<()> {
    with_temp_jcode_home(|| {
        let resolved = save_tui_openai_compatible_api_base("https://api.deepseek.com/")?;
        assert_eq!(resolved.api_base, "https://api.deepseek.com");
        assert!(resolved.requires_api_key);
        assert!(resolved.setup_url.contains("github.com/1jehuang/jcode"));
        assert!(!resolved.setup_url.contains("opencode.ai"));
        Ok(())
    })
}

#[test]
fn tui_openai_compatible_key_save_persists_key_for_current_session() -> anyhow::Result<()> {
    with_temp_jcode_home(|| {
        let resolved = save_tui_openai_compatible_api_base("https://api.example.com/v1")?;
        let resolved = save_tui_openai_compatible_key(
            crate::provider_catalog::OPENAI_COMPAT_PROFILE,
            " sk-test-tui-login ",
        )
        .map(|_| resolved)?;

        assert!(
            crate::provider_catalog::openai_compatible_profile_is_configured(
                crate::provider_catalog::OPENAI_COMPAT_PROFILE,
            )
        );
        assert_eq!(
            crate::provider_catalog::load_api_key(
                &crate::provider_catalog::ApiKeyCredentialSource::from_resolved_catalog_profile(
                    &resolved,
                ),
            )
            .as_deref(),
            Some("sk-test-tui-login")
        );
        Ok(())
    })
}

#[test]
fn tui_api_key_logout_clears_saved_key_and_process_env() -> anyhow::Result<()> {
    with_temp_jcode_home(|| {
        let resolved = save_tui_openai_compatible_api_base("https://api.example.com/v1")?;
        let resolved = save_tui_openai_compatible_key(
            crate::provider_catalog::OPENAI_COMPAT_PROFILE,
            " sk-test-tui-login ",
        )
        .map(|_| resolved)?;

        assert_eq!(
            std::env::var(&resolved.api_key_env).as_deref(),
            Ok("sk-test-tui-login")
        );

        App::clear_api_key_login(&resolved.api_key_env, &resolved.env_file)?;

        assert!(std::env::var_os(&resolved.api_key_env).is_none());
        assert!(
            crate::provider_catalog::load_api_key(
                &crate::provider_catalog::ApiKeyCredentialSource::from_resolved_catalog_profile(
                    &resolved,
                ),
            )
            .is_none()
        );
        Ok(())
    })
}

#[test]
fn tui_jcode_subscription_logout_clears_key_and_base() -> anyhow::Result<()> {
    with_temp_jcode_home(|| {
        crate::provider_catalog::save_env_value_to_env_file(
            crate::subscription_catalog::JCODE_API_KEY_ENV,
            crate::subscription_catalog::JCODE_ENV_FILE,
            Some("test-jcode-key"),
        )?;
        crate::provider_catalog::save_env_value_to_env_file(
            crate::subscription_catalog::JCODE_API_BASE_ENV,
            crate::subscription_catalog::JCODE_ENV_FILE,
            Some("https://subscription.example/v1"),
        )?;
        crate::provider_catalog::save_env_value_to_env_file(
            crate::subscription_catalog::JCODE_ACCOUNT_ID_ENV,
            crate::subscription_catalog::JCODE_ENV_FILE,
            Some("acct_test"),
        )?;
        crate::provider_catalog::save_env_value_to_env_file(
            crate::subscription_catalog::JCODE_ACCOUNT_EMAIL_ENV,
            crate::subscription_catalog::JCODE_ENV_FILE,
            Some("user@example.com"),
        )?;

        App::clear_api_key_login(
            crate::subscription_catalog::JCODE_API_KEY_ENV,
            crate::subscription_catalog::JCODE_ENV_FILE,
        )?;
        for env_key in [
            crate::subscription_catalog::JCODE_API_BASE_ENV,
            crate::subscription_catalog::JCODE_ACCOUNT_ID_ENV,
            crate::subscription_catalog::JCODE_ACCOUNT_EMAIL_ENV,
            crate::subscription_catalog::JCODE_TIER_ENV,
        ] {
            crate::provider_catalog::save_env_value_to_env_file(
                env_key,
                crate::subscription_catalog::JCODE_ENV_FILE,
                None,
            )?;
        }

        assert!(std::env::var_os(crate::subscription_catalog::JCODE_API_KEY_ENV).is_none());
        assert!(std::env::var_os(crate::subscription_catalog::JCODE_API_BASE_ENV).is_none());
        assert!(std::env::var_os(crate::subscription_catalog::JCODE_ACCOUNT_ID_ENV).is_none());
        assert!(std::env::var_os(crate::subscription_catalog::JCODE_ACCOUNT_EMAIL_ENV).is_none());
        assert!(crate::subscription_catalog::configured_api_key().is_none());
        for env_key in [
            crate::subscription_catalog::JCODE_API_BASE_ENV,
            crate::subscription_catalog::JCODE_ACCOUNT_ID_ENV,
            crate::subscription_catalog::JCODE_ACCOUNT_EMAIL_ENV,
        ] {
            assert!(
                crate::provider_catalog::load_env_value_from_env_or_config(
                    env_key,
                    crate::subscription_catalog::JCODE_ENV_FILE,
                )
                .is_none()
            );
        }
        Ok(())
    })
}

#[test]
fn tui_openai_compatible_local_key_save_allows_empty_key() -> anyhow::Result<()> {
    with_temp_jcode_home(|| {
        let resolved = save_tui_openai_compatible_key(crate::provider_catalog::OLLAMA_PROFILE, "")?;
        assert_eq!(resolved.api_base, "http://localhost:11434/v1");
        assert!(
            crate::provider_catalog::openai_compatible_profile_is_configured(
                crate::provider_catalog::OLLAMA_PROFILE
            )
        );
        assert!(
            crate::provider_catalog::load_api_key(
                &crate::provider_catalog::ApiKeyCredentialSource::from_resolved_catalog_profile(
                    &resolved,
                ),
            )
            .is_none()
        );
        Ok(())
    })
}
