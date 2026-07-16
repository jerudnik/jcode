use crate::provider_catalog;
use std::sync::atomic::{AtomicBool, Ordering};

pub const JCODE_API_KEY_ENV: &str = "JCODE_API_KEY";
pub const JCODE_API_BASE_ENV: &str = "JCODE_API_BASE";
pub const JCODE_ACCOUNT_ID_ENV: &str = "JCODE_ACCOUNT_ID";
pub const JCODE_ACCOUNT_EMAIL_ENV: &str = "JCODE_ACCOUNT_EMAIL";
pub const JCODE_TIER_ENV: &str = "JCODE_TIER";
pub const JCODE_ENV_FILE: &str = "jcode-subscription.env";
pub const JCODE_CACHE_NAMESPACE: &str = "jcode-subscription";
pub const JCODE_SUBSCRIPTION_ACTIVE_ENV: &str = "JCODE_SUBSCRIPTION_ACTIVE";
pub const DEFAULT_JCODE_API_BASE: &str = "https://api.jcode.sh/v1";
pub const JCODE_PRICING_URL: &str = "https://jcode.sh/pricing";

static LIVE_TIER_DENIED: AtomicBool = AtomicBool::new(false);

#[cfg(test)]
static FORCE_TIER_STORE_ERROR: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum JcodeTier {
    Plus,
    Flagship,
}

impl JcodeTier {
    pub const ALL: &'static [JcodeTier] = &[JcodeTier::Plus, JcodeTier::Flagship];

    pub fn retail_price_usd(self) -> u32 {
        match self {
            Self::Plus => 10,
            Self::Flagship => 1000,
        }
    }

    pub fn usable_budget_usd(self) -> f64 {
        match self {
            Self::Plus => 18.00,
            Self::Flagship => 3000.00,
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Plus => "Plus",
            Self::Flagship => "Flagship",
        }
    }

    /// Stable machine identifier used for wire values and local persistence.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Plus => "plus",
            Self::Flagship => "flagship",
        }
    }

    /// Parse a tier from a wire/persisted value (case-insensitive).
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "plus" => Some(Self::Plus),
            "flagship" => Some(Self::Flagship),
            _ => None,
        }
    }

    /// Whether an account on this tier may use a model gated at `required`.
    pub fn allows(self, required: JcodeTier) -> bool {
        self >= required
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubscriptionTierFreshness {
    /// A successful `/v1/me` response reported an accepted product-owned tier.
    Live,
    /// Local cache provides the best known accepted tier because no live probe ran.
    Cache,
    /// No live or cached accepted tier is available, so use the lowest safe tier.
    Default,
    /// A live response was present, but entitlement truth was unknown or contradictory.
    UnknownDenied,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubscriptionTierSnapshot {
    pub effective_tier: JcodeTier,
    pub parsed_tier: Option<JcodeTier>,
    pub freshness: SubscriptionTierFreshness,
    pub reason: &'static str,
}

impl SubscriptionTierSnapshot {
    fn live(tier: JcodeTier) -> Self {
        Self {
            effective_tier: tier,
            parsed_tier: Some(tier),
            freshness: SubscriptionTierFreshness::Live,
            reason: "live accepted tier",
        }
    }

    fn unknown_denied(reason: &'static str) -> Self {
        Self {
            effective_tier: JcodeTier::Plus,
            parsed_tier: None,
            freshness: SubscriptionTierFreshness::UnknownDenied,
            reason,
        }
    }
}

pub fn is_active_subscription_status(status: &str) -> bool {
    status.trim().eq_ignore_ascii_case("active")
}

/// Product-owned entitlement truth for a successful `/v1/me` response.
///
/// The fork only accepts the launched `plus` and `flagship` wire tiers. Any
/// absent, blank, unknown, malformed, or inactive/contradictory live response
/// is fail-closed: stale cached Flagship is cleared and model admission falls
/// back to the lowest safe tier.
pub fn classify_live_tier(
    raw_tier: Option<&str>,
    account_status: &str,
) -> SubscriptionTierSnapshot {
    if !is_active_subscription_status(account_status) {
        return SubscriptionTierSnapshot::unknown_denied("inactive subscription status");
    }

    let Some(raw_tier) = raw_tier.map(str::trim).filter(|tier| !tier.is_empty()) else {
        return SubscriptionTierSnapshot::unknown_denied("missing subscription tier");
    };

    match JcodeTier::parse(raw_tier) {
        Some(tier) => SubscriptionTierSnapshot::live(tier),
        None => SubscriptionTierSnapshot::unknown_denied("unknown subscription tier"),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpstreamRoutingPolicy {
    /// Routing is decided server-side by the jcode router (model -> provider +
    /// org key). The client does not pick upstreams; this is the only policy for
    /// the managed subscription.
    ServerManaged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CuratedModel {
    pub id: &'static str,
    pub display_name: &'static str,
    pub aliases: &'static [&'static str],
    pub default_enabled: bool,
    pub routing_policy: UpstreamRoutingPolicy,
    /// Minimum subscription tier that may use this model.
    pub min_tier: JcodeTier,
    pub note: &'static str,
}

pub const CURATED_MODELS: &[CuratedModel] = &[
    CuratedModel {
        id: "claude-opus-4-8",
        display_name: "Claude Opus 4.8",
        aliases: &["claude-opus-4-8", "opus-4-8", "opus 4.8", "claude opus 4.8"],
        default_enabled: true,
        routing_policy: UpstreamRoutingPolicy::ServerManaged,
        min_tier: JcodeTier::Plus,
        note: "Frontier model; routed server-side to Anthropic by the jcode router.",
    },
    CuratedModel {
        id: "gpt-5.5",
        display_name: "GPT-5.5",
        aliases: &["gpt-5.5", "gpt-5-5", "gpt 5.5"],
        default_enabled: false,
        routing_policy: UpstreamRoutingPolicy::ServerManaged,
        min_tier: JcodeTier::Plus,
        note: "Frontier model; routed server-side to OpenAI by the jcode router.",
    },
    CuratedModel {
        id: "claude-fable-5",
        display_name: "Claude Fable 5",
        aliases: &["claude-fable-5", "fable-5", "fable 5", "claude fable 5"],
        default_enabled: false,
        routing_policy: UpstreamRoutingPolicy::ServerManaged,
        min_tier: JcodeTier::Flagship,
        note: "Flagship-tier model; routed server-side to Anthropic by the jcode router.",
    },
    CuratedModel {
        id: "gpt-5.6-sol",
        display_name: "GPT-5.6 Sol",
        aliases: &["gpt-5.6-sol", "gpt 5.6 sol", "sol"],
        default_enabled: false,
        routing_policy: UpstreamRoutingPolicy::ServerManaged,
        min_tier: JcodeTier::Flagship,
        note: "Flagship-tier model; routed server-side to OpenAI by the jcode router.",
    },
];

pub fn curated_models() -> &'static [CuratedModel] {
    CURATED_MODELS
}

pub fn default_model() -> &'static CuratedModel {
    CURATED_MODELS
        .iter()
        .find(|model| model.default_enabled)
        .unwrap_or(&CURATED_MODELS[0])
}

/// Normalize a model id for curated-catalog matching: strips any `@provider`
/// routing suffix, the `[1m]` long-context suffix, and lowercases.
fn normalize_model_key(model: &str) -> String {
    let base = model.trim().split('@').next().unwrap_or("").trim();
    jcode_provider_core::model_id::canonical(base)
}

pub fn find_curated_model(model: &str) -> Option<&'static CuratedModel> {
    let normalized = normalize_model_key(model);
    CURATED_MODELS.iter().find(|candidate| {
        candidate.id.eq_ignore_ascii_case(&normalized)
            || candidate
                .aliases
                .iter()
                .any(|alias| alias.eq_ignore_ascii_case(&normalized))
    })
}

pub fn canonical_model_id(model: &str) -> Option<&'static str> {
    find_curated_model(model).map(|model| model.id)
}

pub fn is_curated_model(model: &str) -> bool {
    canonical_model_id(model).is_some()
}

/// The effective subscription tier for gating decisions.
pub fn effective_tier() -> JcodeTier {
    effective_tier_snapshot().effective_tier
}

pub fn effective_tier_snapshot() -> SubscriptionTierSnapshot {
    if LIVE_TIER_DENIED.load(Ordering::SeqCst) {
        return SubscriptionTierSnapshot::unknown_denied("live tier denied");
    }

    if let Some(tier) = cached_tier() {
        return SubscriptionTierSnapshot {
            effective_tier: tier,
            parsed_tier: Some(tier),
            freshness: SubscriptionTierFreshness::Cache,
            reason: "cached accepted tier",
        };
    }

    if provider_catalog::load_env_value_from_config_file(JCODE_TIER_ENV, JCODE_ENV_FILE).is_some() {
        return SubscriptionTierSnapshot::unknown_denied("cached tier is not accepted");
    }

    SubscriptionTierSnapshot {
        effective_tier: JcodeTier::Plus,
        parsed_tier: None,
        freshness: SubscriptionTierFreshness::Default,
        reason: "no cached tier",
    }
}

/// The last tier reported by the backend, if any was persisted.
pub fn cached_tier() -> Option<JcodeTier> {
    // Tier entitlement is product truth, not ambient configuration. Do not let
    // a process-level `JCODE_TIER=flagship` override the last successful
    // `/v1/me` result or resurrect a cache cleared by unknown-denied live truth.
    provider_catalog::load_env_value_from_config_file(JCODE_TIER_ENV, JCODE_ENV_FILE)
        .as_deref()
        .and_then(JcodeTier::parse)
}

/// Persist the last-known tier reported by the backend (`None` clears it).
pub fn store_cached_tier(tier: Option<JcodeTier>) -> anyhow::Result<()> {
    #[cfg(test)]
    if FORCE_TIER_STORE_ERROR.load(Ordering::SeqCst) {
        anyhow::bail!("forced tier cache write failure");
    }

    provider_catalog::save_env_value_to_env_file(
        JCODE_TIER_ENV,
        JCODE_ENV_FILE,
        tier.map(JcodeTier::as_str),
    )?;
    if tier.is_some() {
        LIVE_TIER_DENIED.store(false, Ordering::SeqCst);
    }
    Ok(())
}

pub fn deny_live_tier_truth(reason: &'static str) -> anyhow::Result<SubscriptionTierSnapshot> {
    let snapshot = SubscriptionTierSnapshot::unknown_denied(reason);
    LIVE_TIER_DENIED.store(true, Ordering::SeqCst);
    store_cached_tier(None)?;
    Ok(snapshot)
}

pub fn apply_live_tier_truth(
    raw_tier: Option<&str>,
    account_status: &str,
) -> anyhow::Result<SubscriptionTierSnapshot> {
    let snapshot = classify_live_tier(raw_tier, account_status);
    match snapshot.freshness {
        SubscriptionTierFreshness::Live => {
            store_cached_tier(snapshot.parsed_tier)?;
            LIVE_TIER_DENIED.store(false, Ordering::SeqCst);
        }
        SubscriptionTierFreshness::UnknownDenied => {
            LIVE_TIER_DENIED.store(true, Ordering::SeqCst);
            store_cached_tier(None)?;
        }
        SubscriptionTierFreshness::Cache | SubscriptionTierFreshness::Default => {}
    }
    Ok(snapshot)
}

#[cfg(test)]
fn reset_live_tier_denial_for_tests() {
    LIVE_TIER_DENIED.store(false, Ordering::SeqCst);
    FORCE_TIER_STORE_ERROR.store(false, Ordering::SeqCst);
}

#[cfg(test)]
fn force_tier_store_error_for_tests(enabled: bool) {
    FORCE_TIER_STORE_ERROR.store(enabled, Ordering::SeqCst);
}

/// Whether the current (cached) tier is allowed to use `model`.
/// Non-curated models return `false`.
pub fn is_model_allowed_for_current_tier(model: &str) -> bool {
    find_curated_model(model)
        .map(|curated| effective_tier().allows(curated.min_tier))
        .unwrap_or(false)
}

pub fn routing_policy_detail(model: &CuratedModel) -> String {
    match model.routing_policy {
        UpstreamRoutingPolicy::ServerManaged => {
            "jcode subscription routing · managed server-side".to_string()
        }
    }
}

pub fn configured_api_key() -> Option<String> {
    provider_catalog::load_env_value_from_env_or_config(JCODE_API_KEY_ENV, JCODE_ENV_FILE)
}

pub fn configured_api_base() -> Option<String> {
    provider_catalog::load_env_value_from_env_or_config(JCODE_API_BASE_ENV, JCODE_ENV_FILE)
}

pub fn has_credentials() -> bool {
    configured_api_key().is_some()
}

pub fn has_router_base() -> bool {
    configured_api_base().is_some()
}

pub fn is_runtime_mode_enabled() -> bool {
    std::env::var(JCODE_SUBSCRIPTION_ACTIVE_ENV)
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes"
            )
        })
        .unwrap_or(false)
}

pub fn apply_runtime_env() {
    crate::env::set_var(JCODE_SUBSCRIPTION_ACTIVE_ENV, "1");
    crate::env::set_var(
        "JCODE_OPENROUTER_API_BASE",
        configured_api_base().unwrap_or_else(|| DEFAULT_JCODE_API_BASE.to_string()),
    );
    crate::env::set_var("JCODE_OPENROUTER_API_KEY_NAME", JCODE_API_KEY_ENV);
    crate::env::set_var("JCODE_OPENROUTER_ENV_FILE", JCODE_ENV_FILE);
    crate::env::set_var("JCODE_OPENROUTER_CACHE_NAMESPACE", JCODE_CACHE_NAMESPACE);
    crate::env::set_var("JCODE_OPENROUTER_PROVIDER_FEATURES", "0");
    crate::env::set_var("JCODE_OPENROUTER_TRANSPORT_STATE", "jcode-subscription");
    crate::env::remove_var("JCODE_OPENROUTER_ALLOW_NO_AUTH");
    crate::env::remove_var("JCODE_OPENROUTER_PROVIDER");
    crate::env::remove_var("JCODE_OPENROUTER_NO_FALLBACK");
}

pub fn clear_runtime_env() {
    crate::env::remove_var(JCODE_SUBSCRIPTION_ACTIVE_ENV);
    crate::env::remove_var("JCODE_OPENROUTER_API_BASE");
    crate::env::remove_var("JCODE_OPENROUTER_API_KEY_NAME");
    crate::env::remove_var("JCODE_OPENROUTER_ENV_FILE");
    crate::env::remove_var("JCODE_OPENROUTER_CACHE_NAMESPACE");
    crate::env::remove_var("JCODE_OPENROUTER_PROVIDER_FEATURES");
    crate::env::remove_var("JCODE_OPENROUTER_TRANSPORT_STATE");
    crate::env::remove_var("JCODE_OPENROUTER_ALLOW_NO_AUTH");
    crate::env::remove_var("JCODE_OPENROUTER_PROVIDER");
    crate::env::remove_var("JCODE_OPENROUTER_NO_FALLBACK");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curated_model_aliases_resolve_to_canonical_ids() {
        assert_eq!(canonical_model_id("opus 4.8"), Some("claude-opus-4-8"));
        assert_eq!(
            canonical_model_id("Claude Opus 4.8"),
            Some("claude-opus-4-8")
        );
        assert_eq!(canonical_model_id("gpt-5.5"), Some("gpt-5.5"));
        assert_eq!(canonical_model_id("GPT 5.5"), Some("gpt-5.5"));
        assert_eq!(canonical_model_id("fable-5"), Some("claude-fable-5"));
        assert_eq!(canonical_model_id("Claude Fable 5"), Some("claude-fable-5"));
        assert_eq!(canonical_model_id("sol"), Some("gpt-5.6-sol"));
        assert_eq!(canonical_model_id("GPT 5.6 Sol"), Some("gpt-5.6-sol"));
        assert_eq!(canonical_model_id("unknown-model"), None);
    }

    #[test]
    fn curated_model_lookup_ignores_provider_pin_suffix() {
        assert_eq!(
            canonical_model_id("claude-opus-4-8@anthropic"),
            Some("claude-opus-4-8")
        );
        assert_eq!(canonical_model_id("gpt-5.5@openai"), Some("gpt-5.5"));
    }

    #[test]
    fn default_model_is_opus() {
        assert_eq!(default_model().id, "claude-opus-4-8");
    }

    #[test]
    fn tier_pricing_matches_launched_plans() {
        assert_eq!(JcodeTier::Plus.retail_price_usd(), 10);
        assert_eq!(JcodeTier::Plus.usable_budget_usd(), 18.00);
        assert_eq!(JcodeTier::Plus.display_name(), "Plus");
        assert_eq!(JcodeTier::Flagship.retail_price_usd(), 1000);
        assert_eq!(JcodeTier::Flagship.usable_budget_usd(), 3000.00);
        assert_eq!(JcodeTier::Flagship.display_name(), "Flagship");
    }

    #[test]
    fn tier_parse_round_trips() {
        for tier in JcodeTier::ALL {
            assert_eq!(JcodeTier::parse(tier.as_str()), Some(*tier));
        }
        assert_eq!(JcodeTier::parse("PLUS"), Some(JcodeTier::Plus));
        assert_eq!(JcodeTier::parse(" Flagship "), Some(JcodeTier::Flagship));
        assert_eq!(JcodeTier::parse("starter"), None);
    }

    #[test]
    fn live_tier_truth_accepts_only_product_owned_tiers() {
        for tier in JcodeTier::ALL {
            let snapshot = classify_live_tier(Some(tier.as_str()), "active");
            assert_eq!(snapshot.parsed_tier, Some(*tier));
            assert_eq!(snapshot.effective_tier, *tier);
            assert_eq!(snapshot.freshness, SubscriptionTierFreshness::Live);
        }

        for (raw_tier, status) in [
            (None, "active"),
            (Some(""), "active"),
            (Some("pro"), "active"),
            (Some("max"), "active"),
            (Some("ultra"), "active"),
            (Some("flagship"), "inactive"),
        ] {
            let snapshot = classify_live_tier(raw_tier, status);
            assert_eq!(snapshot.parsed_tier, None);
            assert_eq!(snapshot.effective_tier, JcodeTier::Plus);
            assert_eq!(snapshot.freshness, SubscriptionTierFreshness::UnknownDenied);
        }
    }

    #[test]
    fn tier_gating_orders_plus_below_flagship() {
        assert!(JcodeTier::Plus.allows(JcodeTier::Plus));
        assert!(!JcodeTier::Plus.allows(JcodeTier::Flagship));
        assert!(JcodeTier::Flagship.allows(JcodeTier::Plus));
        assert!(JcodeTier::Flagship.allows(JcodeTier::Flagship));
    }

    #[test]
    fn flagship_models_are_gated_above_plus() {
        for model in CURATED_MODELS {
            match model.id {
                "claude-fable-5" | "gpt-5.6-sol" => {
                    assert_eq!(model.min_tier, JcodeTier::Flagship)
                }
                _ => assert_eq!(model.min_tier, JcodeTier::Plus),
            }
        }
    }

    #[test]
    fn effective_tier_defaults_to_plus_when_no_live_or_cached_truth() {
        let _guard = crate::storage::lock_test_env();
        reset_live_tier_denial_for_tests();
        crate::env::remove_var(JCODE_TIER_ENV);
        let temp = tempfile::tempdir().expect("temp home");
        crate::env::set_var("JCODE_HOME", temp.path().to_string_lossy().to_string());

        assert_eq!(cached_tier(), None);
        assert_eq!(effective_tier(), JcodeTier::Plus);
        assert_eq!(
            effective_tier_snapshot().freshness,
            SubscriptionTierFreshness::Default
        );
        assert!(is_model_allowed_for_current_tier("claude-opus-4-8"));
        assert!(!is_model_allowed_for_current_tier("claude-fable-5"));

        store_cached_tier(Some(JcodeTier::Flagship)).expect("persist tier");
        assert_eq!(cached_tier(), Some(JcodeTier::Flagship));
        assert_eq!(
            effective_tier_snapshot().freshness,
            SubscriptionTierFreshness::Cache
        );
        assert!(is_model_allowed_for_current_tier("claude-fable-5"));
        assert!(is_model_allowed_for_current_tier("gpt-5.6-sol"));

        store_cached_tier(None).expect("clear tier");
        assert_eq!(cached_tier(), None);

        crate::env::remove_var("JCODE_HOME");
        crate::env::remove_var(JCODE_TIER_ENV);
    }

    #[test]
    fn unaccepted_persisted_tier_is_unknown_denied_truth() {
        let _guard = crate::storage::lock_test_env();
        reset_live_tier_denial_for_tests();
        let temp = tempfile::tempdir().expect("temp home");
        crate::env::set_var("JCODE_HOME", temp.path().to_string_lossy().to_string());
        crate::env::remove_var(JCODE_TIER_ENV);

        provider_catalog::save_env_value_to_env_file(JCODE_TIER_ENV, JCODE_ENV_FILE, Some("pro"))
            .expect("persist unaccepted tier");

        assert_eq!(cached_tier(), None);
        let snapshot = effective_tier_snapshot();
        assert_eq!(snapshot.effective_tier, JcodeTier::Plus);
        assert_eq!(snapshot.parsed_tier, None);
        assert_eq!(snapshot.freshness, SubscriptionTierFreshness::UnknownDenied);
        assert_eq!(snapshot.reason, "cached tier is not accepted");
        assert!(!is_model_allowed_for_current_tier("claude-fable-5"));

        provider_catalog::save_env_value_to_env_file(JCODE_TIER_ENV, JCODE_ENV_FILE, None)
            .expect("clear unaccepted tier");
        reset_live_tier_denial_for_tests();
        crate::env::remove_var("JCODE_HOME");
        crate::env::remove_var(JCODE_TIER_ENV);
    }

    #[test]
    fn live_unknown_denied_tier_clears_stale_flagship_cache() {
        let _guard = crate::storage::lock_test_env();
        reset_live_tier_denial_for_tests();
        let temp = tempfile::tempdir().expect("temp home");
        crate::env::set_var("JCODE_HOME", temp.path().to_string_lossy().to_string());
        crate::env::remove_var(JCODE_TIER_ENV);

        for (raw_tier, status) in [
            (Some("pro"), "active"),
            (None, "active"),
            (Some("flagship"), "inactive"),
        ] {
            store_cached_tier(Some(JcodeTier::Flagship)).expect("seed stale tier");
            assert!(is_model_allowed_for_current_tier("claude-fable-5"));

            let snapshot = apply_live_tier_truth(raw_tier, status).expect("apply live truth");

            assert_eq!(snapshot.freshness, SubscriptionTierFreshness::UnknownDenied);
            assert_eq!(cached_tier(), None);
            assert_eq!(effective_tier(), JcodeTier::Plus);
            assert!(!is_model_allowed_for_current_tier("claude-fable-5"));
        }

        crate::env::remove_var("JCODE_HOME");
        crate::env::remove_var(JCODE_TIER_ENV);
    }

    #[test]
    fn denied_live_tier_overrides_stale_cache_when_durable_clear_fails() {
        let _guard = crate::storage::lock_test_env();
        reset_live_tier_denial_for_tests();
        let temp = tempfile::tempdir().expect("temp home");
        crate::env::set_var("JCODE_HOME", temp.path().to_string_lossy().to_string());
        crate::env::remove_var(JCODE_TIER_ENV);

        store_cached_tier(Some(JcodeTier::Flagship)).expect("seed stale tier");
        assert_eq!(cached_tier(), Some(JcodeTier::Flagship));
        assert!(is_model_allowed_for_current_tier("claude-fable-5"));

        force_tier_store_error_for_tests(true);
        let error = deny_live_tier_truth("malformed subscription response")
            .expect_err("forced durable clear failure should surface");
        assert!(
            error
                .to_string()
                .contains("forced tier cache write failure"),
            "{error}"
        );

        assert_eq!(cached_tier(), Some(JcodeTier::Flagship));
        assert_eq!(
            effective_tier_snapshot().freshness,
            SubscriptionTierFreshness::UnknownDenied
        );
        assert_eq!(effective_tier(), JcodeTier::Plus);
        assert!(!is_model_allowed_for_current_tier("claude-fable-5"));

        force_tier_store_error_for_tests(false);
        apply_live_tier_truth(Some("flagship"), "active")
            .expect("accepted live tier clears marker");
        assert_eq!(cached_tier(), Some(JcodeTier::Flagship));
        assert_eq!(
            effective_tier_snapshot().freshness,
            SubscriptionTierFreshness::Cache
        );
        assert!(is_model_allowed_for_current_tier("claude-fable-5"));

        store_cached_tier(None).expect("clear tier");
        reset_live_tier_denial_for_tests();
        crate::env::remove_var("JCODE_HOME");
        crate::env::remove_var(JCODE_TIER_ENV);
    }

    #[test]
    fn ambient_process_tier_env_does_not_grant_entitlement() {
        let _guard = crate::storage::lock_test_env();
        reset_live_tier_denial_for_tests();
        let temp = tempfile::tempdir().expect("temp home");
        crate::env::set_var("JCODE_HOME", temp.path().to_string_lossy().to_string());
        crate::env::set_var(JCODE_TIER_ENV, "flagship");

        assert_eq!(cached_tier(), None);
        assert_eq!(
            effective_tier_snapshot().freshness,
            SubscriptionTierFreshness::Default
        );
        assert!(!is_model_allowed_for_current_tier("claude-fable-5"));

        store_cached_tier(Some(JcodeTier::Flagship)).expect("seed accepted cache");
        assert_eq!(cached_tier(), Some(JcodeTier::Flagship));

        let snapshot = apply_live_tier_truth(Some("pro"), "active").expect("apply denied truth");
        assert_eq!(snapshot.freshness, SubscriptionTierFreshness::UnknownDenied);
        assert_eq!(cached_tier(), None);
        assert!(!is_model_allowed_for_current_tier("claude-fable-5"));

        crate::env::remove_var("JCODE_HOME");
        crate::env::remove_var(JCODE_TIER_ENV);
    }

    #[test]
    fn runtime_mode_flag_tracks_subscription_activation() {
        let _guard = crate::storage::lock_test_env();
        clear_runtime_env();
        assert!(!is_runtime_mode_enabled());

        apply_runtime_env();
        assert!(is_runtime_mode_enabled());

        clear_runtime_env();
        assert!(!is_runtime_mode_enabled());
    }
}
