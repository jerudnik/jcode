//! Typed client for the jcode subscription backend account endpoint.
//!
//! `GET /v1/me` is the source of truth for the account's tier and usage. The
//! last-known tier is persisted via
//! [`crate::subscription_catalog::store_cached_tier`] so model gating works
//! offline. Live unknown/absent/malformed tier truth is fail-closed and clears
//! any stale cached tier.

use crate::subscription_catalog::{self, JcodeTier, SubscriptionTierFreshness};
use anyhow::{Context, Result};
use reqwest::StatusCode;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::time::Duration;

/// Timeout for the short, non-blocking status fetch used by the TUI.
pub const ME_FETCH_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionUsage {
    pub used_usd: f64,
    pub budget_usd: f64,
    /// RFC 3339 timestamp for when the usage window resets.
    #[serde(default)]
    pub resets_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionMe {
    pub account_id: String,
    pub email: String,
    /// Wire tier value, e.g. "plus" or "flagship". Missing or non-string
    /// values are preserved as `None` so the live response can be denied and
    /// stale cached entitlement can be cleared deterministically.
    #[serde(default, deserialize_with = "deserialize_optional_tier")]
    pub tier: Option<String>,
    pub status: String,
    pub usage: SubscriptionUsage,
}

impl SubscriptionMe {
    pub fn parsed_tier(&self) -> Option<JcodeTier> {
        self.tier.as_deref().and_then(JcodeTier::parse)
    }

    pub fn tier_truth(&self) -> subscription_catalog::SubscriptionTierSnapshot {
        subscription_catalog::classify_live_tier(self.tier.as_deref(), &self.status)
    }
}

fn deserialize_optional_tier<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<Value>::deserialize(deserializer)?;
    Ok(match value {
        Some(Value::String(tier)) if !tier.trim().is_empty() => Some(tier),
        _ => None,
    })
}

/// The `/v1/me` endpoint URL for the configured (or default) API base.
pub fn me_endpoint_url() -> String {
    let base = subscription_catalog::configured_api_base()
        .unwrap_or_else(|| subscription_catalog::DEFAULT_JCODE_API_BASE.to_string());
    format!("{}/me", base.trim_end_matches('/'))
}

/// Fetch the subscription account status from the backend using the
/// configured `JCODE_API_KEY` / `JCODE_API_BASE`.
///
/// On success, persists the reported tier as the last-known tier so offline
/// model gating stays accurate.
pub async fn fetch_subscription_me() -> Result<SubscriptionMe> {
    let api_key = subscription_catalog::configured_api_key()
        .context("no jcode subscription credential configured (run /login jcode)")?;

    let client = crate::provider::shared_http_client();
    let response = client
        .get(me_endpoint_url())
        .bearer_auth(api_key)
        .timeout(ME_FETCH_TIMEOUT)
        .send()
        .await
        .context("failed to reach the jcode subscription API")?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        if is_authoritative_denial_status(status) {
            record_jcode_validation(
                false,
                "jcode subscription /me denied entitlement authorization",
            );
            subscription_catalog::deny_live_tier_truth("subscription authorization denied")
                .context("failed to clear jcode subscription tier cache after denied /me response")?;
        }
        anyhow::bail!(
            "jcode subscription API returned {}: {}",
            status,
            body.chars().take(200).collect::<String>()
        );
    }

    let body = response
        .text()
        .await
        .context("failed to read jcode subscription /me response")?;
    let me: SubscriptionMe = match serde_json::from_str(&body) {
        Ok(me) => me,
        Err(error) => {
            record_jcode_validation(false, "jcode subscription /me response was malformed");
            subscription_catalog::deny_live_tier_truth("malformed subscription response").context(
                "failed to clear jcode subscription tier cache after malformed /me response",
            )?;
            return Err(error).context("failed to parse jcode subscription /me response");
        }
    };

    let tier_truth = subscription_catalog::apply_live_tier_truth(me.tier.as_deref(), &me.status)
        .context("failed to update jcode subscription tier cache")?;
    match tier_truth.freshness {
        SubscriptionTierFreshness::Live => {
            record_jcode_validation(true, "jcode subscription /me returned accepted entitlement")
        }
        SubscriptionTierFreshness::UnknownDenied => record_jcode_validation(
            false,
            "jcode subscription /me returned unknown entitlement and was denied",
        ),
        SubscriptionTierFreshness::Cache | SubscriptionTierFreshness::Default => {}
    }

    Ok(me)
}

fn is_authoritative_denial_status(status: StatusCode) -> bool {
    matches!(status, StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN)
}

fn record_jcode_validation(success: bool, summary: &str) {
    let _ = crate::auth::validation::save(
        "jcode",
        crate::auth::validation::ProviderValidationRecord {
            checked_at_ms: chrono::Utc::now().timestamp_millis(),
            success,
            provider_smoke_ok: Some(success),
            tool_smoke_ok: None,
            summary: summary.to_string(),
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    #[test]
    fn subscription_me_parses_expected_shape() {
        let json = r#"{
            "account_id": "acct_123",
            "email": "dev@example.com",
            "tier": "flagship",
            "status": "active",
            "usage": {
                "used_usd": 12.5,
                "budget_usd": 3000.0,
                "resets_at": "2026-08-01T00:00:00Z"
            }
        }"#;
        let me: SubscriptionMe = serde_json::from_str(json).expect("parse SubscriptionMe");
        assert_eq!(me.account_id, "acct_123");
        assert_eq!(me.email, "dev@example.com");
        assert_eq!(me.tier.as_deref(), Some("flagship"));
        assert_eq!(me.parsed_tier(), Some(JcodeTier::Flagship));
        assert_eq!(me.tier_truth().freshness, SubscriptionTierFreshness::Live);
        assert_eq!(me.status, "active");
        assert_eq!(me.usage.used_usd, 12.5);
        assert_eq!(me.usage.budget_usd, 3000.0);
        assert_eq!(me.usage.resets_at.as_deref(), Some("2026-08-01T00:00:00Z"));
    }

    #[test]
    fn subscription_me_tolerates_missing_resets_at_and_unknown_tier() {
        let json = r#"{
            "account_id": "acct_9",
            "email": "x@example.com",
            "tier": "mystery",
            "status": "active",
            "usage": { "used_usd": 0.0, "budget_usd": 18.0 }
        }"#;
        let me: SubscriptionMe = serde_json::from_str(json).expect("parse SubscriptionMe");
        assert_eq!(me.parsed_tier(), None);
        assert_eq!(
            me.tier_truth().freshness,
            SubscriptionTierFreshness::UnknownDenied
        );
        assert!(me.usage.resets_at.is_none());
    }

    #[test]
    fn subscription_me_treats_absent_and_malformed_tier_as_denied() {
        for tier_fragment in ["", r#""tier": 123,"#] {
            let json = format!(
                r#"{{
                    "account_id": "acct_9",
                    "email": "x@example.com",
                    {tier_fragment}
                    "status": "active",
                    "usage": {{ "used_usd": 0.0, "budget_usd": 18.0 }}
                }}"#
            );
            let me: SubscriptionMe = serde_json::from_str(&json).expect("parse SubscriptionMe");
            assert_eq!(me.tier, None);
            assert_eq!(
                me.tier_truth().freshness,
                SubscriptionTierFreshness::UnknownDenied
            );
        }
    }

    #[test]
    fn me_endpoint_url_appends_me_to_configured_base() {
        let _guard = crate::storage::lock_test_env();
        crate::env::set_var(
            subscription_catalog::JCODE_API_BASE_ENV,
            "https://api.jcode.sh/v1/",
        );
        assert_eq!(me_endpoint_url(), "https://api.jcode.sh/v1/me");
        crate::env::remove_var(subscription_catalog::JCODE_API_BASE_ENV);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn local_me_fixture_accepts_each_product_owned_tier_and_validates_auth_readiness() {
        let _guard = crate::storage::lock_test_env();
        let temp_home = tempfile::tempdir().expect("temp home");
        crate::env::set_var("JCODE_HOME", temp_home.path().to_string_lossy().to_string());
        crate::env::set_var(subscription_catalog::JCODE_API_KEY_ENV, "fixture-key");
        crate::env::remove_var(subscription_catalog::JCODE_TIER_ENV);
        crate::auth::AuthStatus::invalidate_cache();

        let before = crate::auth::AuthStatus::check()
            .assessment_for_provider(crate::provider_catalog::JCODE_LOGIN_PROVIDER)
            .readiness;
        assert_eq!(before, crate::auth::AuthReadinessLevel::CredentialPresent);

        for tier in JcodeTier::ALL.iter().copied() {
            subscription_catalog::store_cached_tier(None).expect("clear tier");
            let api_base = serve_me_once(me_json(Some(tier.as_str()), "active")).await;
            crate::env::set_var(subscription_catalog::JCODE_API_BASE_ENV, api_base);

            let me = fetch_subscription_me().await.expect("fetch fixture /me");

            assert_eq!(me.parsed_tier(), Some(tier));
            assert_eq!(subscription_catalog::cached_tier(), Some(tier));
            assert_eq!(me.tier_truth().freshness, SubscriptionTierFreshness::Live);
            assert!(
                crate::auth::validation::get("jcode")
                    .expect("validation record")
                    .success
            );
        }

        crate::auth::AuthStatus::invalidate_cache();
        let after = crate::auth::AuthStatus::check()
            .assessment_for_provider(crate::provider_catalog::JCODE_LOGIN_PROVIDER)
            .readiness;
        assert_eq!(after, crate::auth::AuthReadinessLevel::RequestValid);

        crate::env::remove_var(subscription_catalog::JCODE_API_BASE_ENV);
        crate::env::remove_var(subscription_catalog::JCODE_API_KEY_ENV);
        crate::env::remove_var(subscription_catalog::JCODE_TIER_ENV);
        crate::env::remove_var("JCODE_HOME");
        crate::auth::AuthStatus::invalidate_cache();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn local_me_fixture_unknown_absent_malformed_and_contradictory_tier_clear_stale_flagship()
    {
        let _guard = crate::storage::lock_test_env();
        let temp_home = tempfile::tempdir().expect("temp home");
        crate::env::set_var("JCODE_HOME", temp_home.path().to_string_lossy().to_string());
        crate::env::set_var(subscription_catalog::JCODE_API_KEY_ENV, "fixture-key");
        crate::env::remove_var(subscription_catalog::JCODE_TIER_ENV);
        crate::auth::AuthStatus::invalidate_cache();

        let fixtures = [
            ("unknown", me_json(Some("pro"), "active")),
            ("absent", me_json(None, "active")),
            ("malformed", me_json_with_raw_tier("123", "active")),
            ("contradictory", me_json(Some("flagship"), "inactive")),
        ];

        for (name, body) in fixtures {
            subscription_catalog::store_cached_tier(Some(JcodeTier::Flagship))
                .expect("seed stale flagship");
            assert!(subscription_catalog::is_model_allowed_for_current_tier(
                "claude-fable-5"
            ));

            let api_base = serve_me_once(body).await;
            crate::env::set_var(subscription_catalog::JCODE_API_BASE_ENV, api_base);
            let me = fetch_subscription_me()
                .await
                .unwrap_or_else(|error| panic!("{name} fixture should parse: {error}"));

            assert_eq!(
                me.tier_truth().freshness,
                SubscriptionTierFreshness::UnknownDenied,
                "{name} fixture"
            );
            assert_eq!(subscription_catalog::cached_tier(), None, "{name} fixture");
            assert_eq!(subscription_catalog::effective_tier(), JcodeTier::Plus);
            assert!(
                !subscription_catalog::is_model_allowed_for_current_tier("claude-fable-5"),
                "{name} fixture must deny stale flagship-only model"
            );
            assert!(
                !crate::auth::validation::get("jcode")
                    .expect("validation record")
                    .success
            );
        }

        crate::env::remove_var(subscription_catalog::JCODE_API_BASE_ENV);
        crate::env::remove_var(subscription_catalog::JCODE_API_KEY_ENV);
        crate::env::remove_var(subscription_catalog::JCODE_TIER_ENV);
        crate::env::remove_var("JCODE_HOME");
        crate::auth::AuthStatus::invalidate_cache();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn local_me_fixture_malformed_json_clears_stale_flagship_cache() {
        let _guard = crate::storage::lock_test_env();
        let temp_home = tempfile::tempdir().expect("temp home");
        crate::env::set_var("JCODE_HOME", temp_home.path().to_string_lossy().to_string());
        crate::env::set_var(subscription_catalog::JCODE_API_KEY_ENV, "fixture-key");
        crate::env::remove_var(subscription_catalog::JCODE_TIER_ENV);
        subscription_catalog::store_cached_tier(Some(JcodeTier::Flagship))
            .expect("seed stale flagship");
        assert!(subscription_catalog::is_model_allowed_for_current_tier(
            "claude-fable-5"
        ));

        let api_base =
            serve_me_once(r#"{"account_id":"acct_fixture","tier":"flagship""#.to_string()).await;
        crate::env::set_var(subscription_catalog::JCODE_API_BASE_ENV, api_base);

        let error = fetch_subscription_me()
            .await
            .expect_err("malformed JSON should fail fetch");

        assert!(
            error
                .to_string()
                .contains("failed to parse jcode subscription /me response"),
            "{error}"
        );
        assert_eq!(subscription_catalog::cached_tier(), None);
        assert!(!subscription_catalog::is_model_allowed_for_current_tier(
            "claude-fable-5"
        ));
        assert!(
            !crate::auth::validation::get("jcode")
                .expect("validation record")
                .success
        );

        crate::env::remove_var(subscription_catalog::JCODE_API_BASE_ENV);
        crate::env::remove_var(subscription_catalog::JCODE_API_KEY_ENV);
        crate::env::remove_var(subscription_catalog::JCODE_TIER_ENV);
        crate::env::remove_var("JCODE_HOME");
        crate::auth::AuthStatus::invalidate_cache();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn local_me_fixture_http_401_403_clear_stale_flagship_and_downgrade_auth_readiness() {
        let _guard = crate::storage::lock_test_env();
        let temp_home = tempfile::tempdir().expect("temp home");
        crate::env::set_var("JCODE_HOME", temp_home.path().to_string_lossy().to_string());
        crate::env::set_var(subscription_catalog::JCODE_API_KEY_ENV, "fixture-key");
        crate::env::remove_var(subscription_catalog::JCODE_TIER_ENV);

        for status in [StatusCode::UNAUTHORIZED, StatusCode::FORBIDDEN] {
            record_jcode_validation(true, "fixture prior /me validation succeeded");
            crate::auth::AuthStatus::invalidate_cache();
            let before = crate::auth::AuthStatus::check()
                .assessment_for_provider(crate::provider_catalog::JCODE_LOGIN_PROVIDER)
                .readiness;
            assert_eq!(before, crate::auth::AuthReadinessLevel::RequestValid);

            subscription_catalog::store_cached_tier(Some(JcodeTier::Flagship))
                .expect("seed stale flagship");
            assert!(subscription_catalog::is_model_allowed_for_current_tier(
                "claude-fable-5"
            ));

            let api_base = serve_me_once_with_status(
                status,
                r#"{"error":"subscription denied"}"#.to_string(),
            )
            .await;
            crate::env::set_var(subscription_catalog::JCODE_API_BASE_ENV, api_base);

            let error = fetch_subscription_me()
                .await
                .expect_err("authoritative HTTP denial should fail fetch");

            assert!(
                error.to_string().contains(&format!(
                    "jcode subscription API returned {}",
                    status
                )),
                "{error}"
            );
            assert_eq!(subscription_catalog::cached_tier(), None);
            assert_eq!(subscription_catalog::effective_tier(), JcodeTier::Plus);
            assert_eq!(
                subscription_catalog::effective_tier_snapshot().freshness,
                SubscriptionTierFreshness::UnknownDenied
            );
            assert!(!subscription_catalog::is_model_allowed_for_current_tier(
                "claude-fable-5"
            ));
            assert!(
                !crate::auth::validation::get("jcode")
                    .expect("validation record")
                    .success
            );

            crate::auth::AuthStatus::invalidate_cache();
            let after = crate::auth::AuthStatus::check()
                .assessment_for_provider(crate::provider_catalog::JCODE_LOGIN_PROVIDER)
                .readiness;
            assert_eq!(after, crate::auth::AuthReadinessLevel::CredentialPresent);
        }

        subscription_catalog::apply_live_tier_truth(Some("plus"), "active")
            .expect("accepted live truth clears denial marker");
        subscription_catalog::store_cached_tier(None).expect("clear tier");
        crate::env::remove_var(subscription_catalog::JCODE_API_BASE_ENV);
        crate::env::remove_var(subscription_catalog::JCODE_API_KEY_ENV);
        crate::env::remove_var(subscription_catalog::JCODE_TIER_ENV);
        crate::env::remove_var("JCODE_HOME");
        crate::auth::AuthStatus::invalidate_cache();
    }

    fn me_json(tier: Option<&str>, status: &str) -> String {
        let tier_field = tier
            .map(|tier| format!(r#""tier": "{tier}","#))
            .unwrap_or_default();
        format!(
            r#"{{
                "account_id": "acct_fixture",
                "email": "fixture@example.com",
                {tier_field}
                "status": "{status}",
                "usage": {{ "used_usd": 1.0, "budget_usd": 18.0 }}
            }}"#
        )
    }

    fn me_json_with_raw_tier(raw_tier: &str, status: &str) -> String {
        format!(
            r#"{{
                "account_id": "acct_fixture",
                "email": "fixture@example.com",
                "tier": {raw_tier},
                "status": "{status}",
                "usage": {{ "used_usd": 1.0, "budget_usd": 18.0 }}
            }}"#
        )
    }

    async fn serve_me_once(body: String) -> String {
        serve_me_once_with_status(StatusCode::OK, body).await
    }

    async fn serve_me_once_with_status(status: StatusCode, body: String) -> String {
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("bind fixture server");
        let addr = listener.local_addr().expect("fixture local addr");
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept fixture request");
            let mut request = [0_u8; 4096];
            let _ = stream.read(&mut request).await;
            let response = format!(
                "HTTP/1.1 {} {}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                status.as_u16(),
                status.canonical_reason().unwrap_or("status"),
                body.len(),
                body
            );
            stream
                .write_all(response.as_bytes())
                .await
                .expect("write fixture response");
        });
        format!("http://{addr}/v1")
    }
}
