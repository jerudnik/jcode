use crate::{ModelRoute, normalize_copilot_model_name};
use std::borrow::Cow;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ActiveProvider {
    Claude,
    OpenAI,
    Copilot,
    Antigravity,
    Gemini,
    Cursor,
    Bedrock,
    OpenRouter,
}

impl ActiveProvider {
    pub const ALL: [Self; 8] = [
        Self::Claude,
        Self::OpenAI,
        Self::Copilot,
        Self::Antigravity,
        Self::Gemini,
        Self::Cursor,
        Self::Bedrock,
        Self::OpenRouter,
    ];

    /// Stable machine-facing key for this built-in provider.
    pub const fn key(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::OpenAI => "openai",
            Self::Copilot => "copilot",
            Self::Antigravity => "antigravity",
            Self::Gemini => "gemini",
            Self::Cursor => "cursor",
            Self::Bedrock => "bedrock",
            Self::OpenRouter => "openrouter",
        }
    }

    /// Accepted human/config aliases in addition to [`ActiveProvider::key`].
    pub const fn aliases(self) -> &'static [&'static str] {
        match self {
            Self::Claude => &["anthropic"],
            Self::OpenAI => &[],
            Self::Copilot => &["github copilot"],
            Self::Antigravity => &[],
            Self::Gemini => &["google gemini"],
            Self::Cursor => &[],
            Self::Bedrock => &["aws-bedrock", "aws_bedrock"],
            Self::OpenRouter => &[],
        }
    }

    /// Parse a canonical key or alias, ignoring surrounding whitespace and
    /// ASCII case.
    pub fn from_key_or_alias(value: &str) -> Option<Self> {
        let normalized = value.trim().to_ascii_lowercase();
        Self::ALL.into_iter().find(|provider| {
            normalized == provider.key()
                || provider.aliases().iter().any(|alias| normalized == *alias)
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct ProviderAvailability {
    pub openai: bool,
    pub claude: bool,
    pub copilot: bool,
    pub antigravity: bool,
    pub gemini: bool,
    pub cursor: bool,
    pub bedrock: bool,
    pub openrouter: bool,
    pub copilot_premium_zero: bool,
}

impl ProviderAvailability {
    pub fn is_configured(self, provider: ActiveProvider) -> bool {
        match provider {
            ActiveProvider::Claude => self.claude,
            ActiveProvider::OpenAI => self.openai,
            ActiveProvider::Copilot => self.copilot,
            ActiveProvider::Antigravity => self.antigravity,
            ActiveProvider::Gemini => self.gemini,
            ActiveProvider::Cursor => self.cursor,
            ActiveProvider::Bedrock => self.bedrock,
            ActiveProvider::OpenRouter => self.openrouter,
        }
    }
}

pub fn auto_default_provider(availability: ProviderAvailability) -> ActiveProvider {
    if availability.copilot_premium_zero && availability.copilot {
        ActiveProvider::Copilot
    } else if availability.openai {
        ActiveProvider::OpenAI
    } else if availability.claude {
        ActiveProvider::Claude
    } else if availability.copilot {
        ActiveProvider::Copilot
    } else if availability.antigravity {
        ActiveProvider::Antigravity
    } else if availability.gemini {
        ActiveProvider::Gemini
    } else if availability.cursor {
        ActiveProvider::Cursor
    } else if availability.bedrock {
        ActiveProvider::Bedrock
    } else if availability.openrouter {
        ActiveProvider::OpenRouter
    } else {
        ActiveProvider::Claude
    }
}

pub fn parse_provider_hint(value: &str) -> Option<ActiveProvider> {
    ActiveProvider::from_key_or_alias(value)
}

pub fn provider_label(provider: ActiveProvider) -> &'static str {
    match provider {
        ActiveProvider::Claude => "Anthropic",
        ActiveProvider::OpenAI => "OpenAI",
        ActiveProvider::Copilot => "GitHub Copilot",
        ActiveProvider::Antigravity => "Antigravity",
        ActiveProvider::Gemini => "Gemini",
        ActiveProvider::Cursor => "Cursor",
        ActiveProvider::Bedrock => "AWS Bedrock",
        ActiveProvider::OpenRouter => "OpenRouter",
    }
}

pub fn provider_key(provider: ActiveProvider) -> &'static str {
    provider.key()
}

pub fn provider_from_model_key(key: &str) -> Option<ActiveProvider> {
    ActiveProvider::from_key_or_alias(key)
}

/// Translate a persisted session/runtime provider key (the `RuntimeKey`
/// stable-id or `ModelRouteApiMethod` vocabulary, e.g. `anthropic-api-key`,
/// `claude-oauth`, `openai-api-key`) into the CLI `--provider` argument value
/// (the `ProviderChoice` vocabulary, e.g. `anthropic-api`, `claude`,
/// `openai-api`).
///
/// These two vocabularies overlap but are NOT identical: the runtime key
/// distinguishes auth method (`anthropic-api-key` vs `claude-oauth`) while the
/// CLI `--provider` enum uses `anthropic-api` / `claude`. Passing a raw runtime
/// key straight to `--provider` makes clap reject it (`invalid value
/// 'anthropic-api-key'`) and the spawned process exits immediately.
///
/// Returns `None` when there is no clean, unambiguous CLI provider to pass; in
/// that case callers should omit the flag entirely and rely on the persisted
/// session (model + provider_key + route_api_method) to reconstruct the exact
/// route on resume.
pub fn cli_provider_arg_for_session_key(key: &str) -> Option<&'static str> {
    let normalized = key.trim().to_ascii_lowercase();
    let base = normalized
        .split_once(':')
        .map(|(prefix, _rest)| prefix)
        .unwrap_or(normalized.as_str());
    // Dual-auth (Anthropic/OpenAI OAuth-vs-API) keys share one canonical alias
    // table, so the CLI arg never drifts from the route/runtime vocabularies.
    if let Some(route) = crate::auth_mode::AuthRoute::parse(base) {
        return Some(route.cli_provider_arg());
    }
    if let Some(provider) = ActiveProvider::from_key_or_alias(base) {
        return Some(provider.key());
    }
    match base {
        "code-assist-oauth" | "google" => Some("google"),
        // openai-compatible / custom profiles, remote-catalog, current, and any
        // unknown key have no clean standalone CLI provider value (they need a
        // profile too), so omit the flag and let the persisted session route.
        _ => None,
    }
}

pub fn explicit_model_provider_prefix(model: &str) -> Option<(ActiveProvider, &'static str, &str)> {
    const PREFIXES: &[(&str, ActiveProvider)] = &[
        ("claude-api:", ActiveProvider::Claude),
        ("claude-oauth:", ActiveProvider::Claude),
        ("claude:", ActiveProvider::Claude),
        ("anthropic:", ActiveProvider::Claude),
        ("openai-api:", ActiveProvider::OpenAI),
        ("openai-oauth:", ActiveProvider::OpenAI),
        ("openai:", ActiveProvider::OpenAI),
        ("copilot:", ActiveProvider::Copilot),
        ("antigravity:", ActiveProvider::Antigravity),
        ("gemini:", ActiveProvider::Gemini),
        ("cursor:", ActiveProvider::Cursor),
        ("bedrock:", ActiveProvider::Bedrock),
        ("openrouter:", ActiveProvider::OpenRouter),
    ];

    for &(prefix, builtin_provider) in PREFIXES {
        if let Some(rest) = model.strip_prefix(prefix) {
            let provider = crate::auth_mode::AuthRoute::parse_explicit_credential_prefix(prefix)
                .map(crate::auth_mode::AuthRoute::active_provider)
                .unwrap_or(builtin_provider);
            return Some((provider, prefix, rest));
        }
    }
    None
}

pub fn model_name_for_provider(provider: ActiveProvider, model: &str) -> Cow<'_, str> {
    if matches!(provider, ActiveProvider::Claude)
        && let Some(canonical) = normalize_copilot_model_name(model)
    {
        return Cow::Borrowed(canonical);
    }
    Cow::Borrowed(model)
}

pub fn dedupe_model_routes(routes: Vec<ModelRoute>) -> Vec<ModelRoute> {
    use std::collections::HashMap;

    let mut deduped: Vec<ModelRoute> = Vec::with_capacity(routes.len());
    // Bucket candidate duplicates by (provider, model). The api_method match is
    // fuzzy (generic vs profile openai-compatible), so buckets keep a linear
    // scan, but each bucket only holds the handful of routes for one model.
    // The previous full `deduped.iter().position(..)` scan was O(n^2) over
    // 2000+ routes and showed up in server connect-burst profiles.
    let mut buckets: HashMap<(String, String), Vec<usize>> = HashMap::with_capacity(routes.len());

    for route in routes {
        let key = (route.provider.clone(), route.model.clone());
        let bucket = buckets.entry(key).or_default();

        if let Some(existing_idx) = bucket
            .iter()
            .copied()
            .find(|&idx| duplicate_route_api_method(&deduped[idx].api_method, &route.api_method))
        {
            if should_replace_duplicate_route(&deduped[existing_idx], &route) {
                deduped[existing_idx] = route;
            }
            continue;
        }

        bucket.push(deduped.len());
        deduped.push(route);
    }

    deduped
}

#[cfg(test)]
fn duplicate_model_route(existing: &ModelRoute, candidate: &ModelRoute) -> bool {
    existing.provider == candidate.provider
        && existing.model == candidate.model
        && duplicate_route_api_method(&existing.api_method, &candidate.api_method)
}

/// Reference O(n^2) dedupe used to prove the bucketed implementation above is
/// behavior-identical (see `bucketed_dedupe_matches_reference` test).
#[cfg(test)]
fn dedupe_model_routes_reference(routes: Vec<ModelRoute>) -> Vec<ModelRoute> {
    let mut deduped: Vec<ModelRoute> = Vec::with_capacity(routes.len());
    for route in routes {
        if let Some(existing_idx) = deduped
            .iter()
            .position(|existing| duplicate_model_route(existing, &route))
        {
            if should_replace_duplicate_route(&deduped[existing_idx], &route) {
                deduped[existing_idx] = route;
            }
            continue;
        }
        deduped.push(route);
    }
    deduped
}

fn duplicate_route_api_method(existing: &str, candidate: &str) -> bool {
    existing == candidate
        || (is_generic_openai_compatible_route(existing)
            && is_profile_openai_compatible_route(candidate))
        || (is_profile_openai_compatible_route(existing)
            && is_generic_openai_compatible_route(candidate))
}

fn is_generic_openai_compatible_route(api_method: &str) -> bool {
    api_method == "openai-compatible"
}

fn is_profile_openai_compatible_route(api_method: &str) -> bool {
    api_method.starts_with("openai-compatible:")
}

fn should_replace_duplicate_route(existing: &ModelRoute, candidate: &ModelRoute) -> bool {
    // A direct OpenAI-compatible provider can briefly appear twice in merged
    // catalogs: once as the generic transport and once as the named profile
    // transport. Keep the profile-scoped route so selection writes
    // `profile:model` rather than falling back to ambiguous generic routing.
    let existing_profile_scoped = is_profile_openai_compatible_route(&existing.api_method);
    let candidate_profile_scoped = is_profile_openai_compatible_route(&candidate.api_method);
    !existing_profile_scoped && candidate_profile_scoped
}

pub fn fallback_sequence(active: ActiveProvider) -> Vec<ActiveProvider> {
    match active {
        ActiveProvider::Claude => vec![
            ActiveProvider::Claude,
            ActiveProvider::OpenAI,
            ActiveProvider::Copilot,
            ActiveProvider::Gemini,
            ActiveProvider::Cursor,
            ActiveProvider::Bedrock,
            ActiveProvider::OpenRouter,
        ],
        ActiveProvider::OpenAI => vec![
            ActiveProvider::OpenAI,
            ActiveProvider::Claude,
            ActiveProvider::Copilot,
            ActiveProvider::Gemini,
            ActiveProvider::Cursor,
            ActiveProvider::Bedrock,
            ActiveProvider::OpenRouter,
        ],
        ActiveProvider::Copilot => vec![
            ActiveProvider::Copilot,
            ActiveProvider::Claude,
            ActiveProvider::OpenAI,
            ActiveProvider::Antigravity,
            ActiveProvider::Gemini,
            ActiveProvider::Cursor,
            ActiveProvider::Bedrock,
            ActiveProvider::OpenRouter,
        ],
        ActiveProvider::Antigravity => vec![
            ActiveProvider::Antigravity,
            ActiveProvider::Claude,
            ActiveProvider::OpenAI,
            ActiveProvider::Copilot,
            ActiveProvider::Gemini,
            ActiveProvider::Cursor,
            ActiveProvider::Bedrock,
            ActiveProvider::OpenRouter,
        ],
        ActiveProvider::Gemini => vec![
            ActiveProvider::Gemini,
            ActiveProvider::Claude,
            ActiveProvider::OpenAI,
            ActiveProvider::Antigravity,
            ActiveProvider::Copilot,
            ActiveProvider::Cursor,
            ActiveProvider::Bedrock,
            ActiveProvider::OpenRouter,
        ],
        ActiveProvider::Cursor => vec![
            ActiveProvider::Cursor,
            ActiveProvider::Claude,
            ActiveProvider::OpenAI,
            ActiveProvider::Copilot,
            ActiveProvider::Antigravity,
            ActiveProvider::Gemini,
            ActiveProvider::OpenRouter,
        ],
        ActiveProvider::Bedrock => vec![
            ActiveProvider::Bedrock,
            ActiveProvider::Claude,
            ActiveProvider::OpenAI,
            ActiveProvider::Copilot,
            ActiveProvider::Antigravity,
            ActiveProvider::Gemini,
            ActiveProvider::Cursor,
            ActiveProvider::OpenRouter,
        ],
        ActiveProvider::OpenRouter => vec![
            ActiveProvider::OpenRouter,
            ActiveProvider::Claude,
            ActiveProvider::OpenAI,
            ActiveProvider::Copilot,
            ActiveProvider::Antigravity,
            ActiveProvider::Gemini,
            ActiveProvider::Cursor,
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_provider_owns_all_canonical_keys_and_aliases() {
        let expected = [
            (ActiveProvider::Claude, "claude", &["anthropic"][..]),
            (ActiveProvider::OpenAI, "openai", &[][..]),
            (ActiveProvider::Copilot, "copilot", &["github copilot"][..]),
            (ActiveProvider::Antigravity, "antigravity", &[][..]),
            (ActiveProvider::Gemini, "gemini", &["google gemini"][..]),
            (ActiveProvider::Cursor, "cursor", &[][..]),
            (
                ActiveProvider::Bedrock,
                "bedrock",
                &["aws-bedrock", "aws_bedrock"][..],
            ),
            (ActiveProvider::OpenRouter, "openrouter", &[][..]),
        ];

        assert_eq!(ActiveProvider::ALL.len(), expected.len());
        for (provider, key, aliases) in expected {
            assert_eq!(provider.key(), key);
            assert_eq!(provider.aliases(), aliases);
            assert_eq!(provider_key(provider), key);
            assert_eq!(ActiveProvider::from_key_or_alias(key), Some(provider));
            assert_eq!(provider_from_model_key(key), Some(provider));
            assert_eq!(
                parse_provider_hint(&format!("  {}  ", key.to_uppercase())),
                Some(provider)
            );

            for alias in aliases {
                assert_eq!(ActiveProvider::from_key_or_alias(alias), Some(provider));
                assert_eq!(parse_provider_hint(alias), Some(provider));
            }
        }

        assert_eq!(parse_provider_hint("unknown"), None);
        assert_eq!(ActiveProvider::from_key_or_alias(""), None);
    }

    #[test]
    fn cli_provider_arg_translates_runtime_keys() {
        // Anthropic API key (the regression: this is NOT a valid --provider
        // value verbatim; it must map to `anthropic-api`).
        assert_eq!(
            cli_provider_arg_for_session_key("anthropic-api-key"),
            Some("anthropic-api")
        );
        assert_eq!(
            cli_provider_arg_for_session_key("claude-api"),
            Some("anthropic-api")
        );
        // Anthropic OAuth -> claude.
        assert_eq!(
            cli_provider_arg_for_session_key("claude-oauth"),
            Some("claude")
        );
        assert_eq!(cli_provider_arg_for_session_key("claude"), Some("claude"));
        // OpenAI variants.
        assert_eq!(
            cli_provider_arg_for_session_key("openai-oauth"),
            Some("openai")
        );
        assert_eq!(
            cli_provider_arg_for_session_key("openai-api-key"),
            Some("openai-api")
        );
        // Passthrough providers.
        assert_eq!(
            cli_provider_arg_for_session_key("openrouter"),
            Some("openrouter")
        );
        assert_eq!(cli_provider_arg_for_session_key("copilot"), Some("copilot"));
        assert_eq!(cli_provider_arg_for_session_key("gemini"), Some("gemini"));
        assert_eq!(cli_provider_arg_for_session_key("bedrock"), Some("bedrock"));
        assert_eq!(
            cli_provider_arg_for_session_key("aws-bedrock"),
            Some("bedrock")
        );
        // Case-insensitive and whitespace tolerant.
        assert_eq!(
            cli_provider_arg_for_session_key("  Anthropic-API-Key "),
            Some("anthropic-api")
        );
        // Profile-scoped openai-compatible keys have no clean standalone CLI
        // value, so we omit the flag and let the persisted session route.
        assert_eq!(
            cli_provider_arg_for_session_key("openai-compatible:zai"),
            None
        );
        assert_eq!(cli_provider_arg_for_session_key("openai-compatible"), None);
        assert_eq!(cli_provider_arg_for_session_key("remote-catalog"), None);
        assert_eq!(cli_provider_arg_for_session_key("current"), None);
        assert_eq!(cli_provider_arg_for_session_key("totally-unknown"), None);
    }

    #[test]
    fn parses_model_provider_prefixes() {
        assert_eq!(
            provider_from_model_key("gemini"),
            Some(ActiveProvider::Gemini)
        );
        assert_eq!(provider_from_model_key("missing"), None);

        for (raw, expected_provider, expected_prefix, expected_model) in [
            (
                "claude-api:sonnet",
                ActiveProvider::Claude,
                "claude-api:",
                "sonnet",
            ),
            (
                "claude-oauth:sonnet",
                ActiveProvider::Claude,
                "claude-oauth:",
                "sonnet",
            ),
            ("claude:sonnet", ActiveProvider::Claude, "claude:", "sonnet"),
            (
                "anthropic:sonnet",
                ActiveProvider::Claude,
                "anthropic:",
                "sonnet",
            ),
            ("openai:gpt-5", ActiveProvider::OpenAI, "openai:", "gpt-5"),
            (
                "openai-oauth:gpt-5",
                ActiveProvider::OpenAI,
                "openai-oauth:",
                "gpt-5",
            ),
            (
                "openai-api:gpt-5",
                ActiveProvider::OpenAI,
                "openai-api:",
                "gpt-5",
            ),
            (
                "copilot:gpt-5",
                ActiveProvider::Copilot,
                "copilot:",
                "gpt-5",
            ),
            (
                "antigravity:default",
                ActiveProvider::Antigravity,
                "antigravity:",
                "default",
            ),
            (
                "gemini:gemini-2.5-pro",
                ActiveProvider::Gemini,
                "gemini:",
                "gemini-2.5-pro",
            ),
            (
                "cursor:composer-1.5",
                ActiveProvider::Cursor,
                "cursor:",
                "composer-1.5",
            ),
            (
                "bedrock:anthropic.claude",
                ActiveProvider::Bedrock,
                "bedrock:",
                "anthropic.claude",
            ),
            (
                "openrouter:meta/llama",
                ActiveProvider::OpenRouter,
                "openrouter:",
                "meta/llama",
            ),
        ] {
            let (provider, prefix, model) = explicit_model_provider_prefix(raw).unwrap();
            assert_eq!(provider, expected_provider, "{raw}");
            assert_eq!(prefix, expected_prefix, "{raw}");
            assert_eq!(model, expected_model, "{raw}");
        }
        for raw in [
            "anthropic-api:sonnet",
            "anthropic-oauth:sonnet",
            "anthropic-api-key:sonnet",
            "claude-api-key:sonnet",
            "anthropic-key:sonnet",
            "claude-key:sonnet",
            "api-key:sonnet",
            "openai-api-key:gpt-5",
            "openai-key:gpt-5",
            "openai-apikey:gpt-5",
            "openai-platform:gpt-5",
            "platform-openai:gpt-5",
            "unknown:sonnet",
            "Claude:sonnet",
        ] {
            assert_eq!(explicit_model_provider_prefix(raw), None, "{raw}");
        }
    }

    #[test]
    fn dedupes_model_routes_by_route_identity() {
        let routes = vec![
            ModelRoute {
                model: "m".to_string(),
                provider: "p".to_string(),
                api_method: "a".to_string(),
                available: true,
                detail: String::new(),
                cheapness: None,
            },
            ModelRoute {
                model: "m".to_string(),
                provider: "p".to_string(),
                api_method: "a".to_string(),
                available: false,
                detail: "duplicate".to_string(),
                cheapness: None,
            },
            ModelRoute {
                model: "m".to_string(),
                provider: "p".to_string(),
                api_method: "b".to_string(),
                available: true,
                detail: String::new(),
                cheapness: None,
            },
        ];

        let deduped = dedupe_model_routes(routes);
        assert_eq!(deduped.len(), 2);
        assert_eq!(deduped[0].detail, "");
    }

    #[test]
    fn dedupes_openai_compatible_generic_and_profile_aliases() {
        let routes = vec![
            ModelRoute {
                model: "qwen".to_string(),
                provider: "Cerebras".to_string(),
                api_method: "openai-compatible".to_string(),
                available: true,
                detail: "generic transport".to_string(),
                cheapness: None,
            },
            ModelRoute {
                model: "qwen".to_string(),
                provider: "Cerebras".to_string(),
                api_method: "openai-compatible:cerebras".to_string(),
                available: true,
                detail: "profile transport".to_string(),
                cheapness: None,
            },
            ModelRoute {
                model: "qwen".to_string(),
                provider: "OtherDirect".to_string(),
                api_method: "openai-compatible:other".to_string(),
                available: true,
                detail: "different provider".to_string(),
                cheapness: None,
            },
            ModelRoute {
                model: "qwen".to_string(),
                provider: "Cerebras".to_string(),
                api_method: "openai-compatible:cerebras-alt".to_string(),
                available: true,
                detail: "distinct profile route".to_string(),
                cheapness: None,
            },
        ];

        let deduped = dedupe_model_routes(routes);
        assert_eq!(deduped.len(), 3);
        let cerebras = deduped
            .iter()
            .find(|route| route.provider == "Cerebras")
            .expect("Cerebras route remains");
        assert_eq!(cerebras.api_method, "openai-compatible:cerebras");
        assert_eq!(cerebras.detail, "profile transport");
        assert!(deduped.iter().any(|route| {
            route.provider == "Cerebras" && route.api_method == "openai-compatible:cerebras-alt"
        }));
    }

    /// State-space equivalence: the bucketed O(n) dedupe must produce exactly
    /// the same output (content and order) as the original O(n^2) reference for
    /// a pseudo-random mix of providers/models/api-methods, including the fuzzy
    /// generic-vs-profile openai-compatible collisions.
    #[test]
    fn bucketed_dedupe_matches_reference() {
        let providers = ["Anthropic", "OpenAI", "Cerebras", "auto"];
        let models = ["m1", "m2", "m3", "qwen", "claude-x"];
        let api_methods = [
            "claude-oauth",
            "claude-api",
            "openrouter",
            "openai-compatible",
            "openai-compatible:cerebras",
            "openai-compatible:other",
        ];

        // Deterministic pseudo-random stream, dense enough to hit every
        // provider/model/api-method combination and repeated duplicates.
        let mut seed = 0x9e37_79b9_u64;
        let mut routes = Vec::new();
        for i in 0..600 {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let p = providers[(seed >> 7) as usize % providers.len()];
            let m = models[(seed >> 17) as usize % models.len()];
            let a = api_methods[(seed >> 27) as usize % api_methods.len()];
            routes.push(ModelRoute {
                model: m.to_string(),
                provider: p.to_string(),
                api_method: a.to_string(),
                available: seed & 1 == 0,
                detail: format!("route-{i}"),
                cheapness: None,
            });
        }

        let expected = dedupe_model_routes_reference(routes.clone());
        let actual = dedupe_model_routes(routes);
        assert_eq!(actual, expected);
    }

    #[test]
    fn auto_default_prefers_copilot_zero_mode() {
        let provider = auto_default_provider(ProviderAvailability {
            openai: true,
            copilot: true,
            copilot_premium_zero: true,
            ..ProviderAvailability::default()
        });
        assert_eq!(provider, ActiveProvider::Copilot);
    }

    #[test]
    fn fallback_sequence_keeps_active_first() {
        let sequence = fallback_sequence(ActiveProvider::OpenRouter);
        assert_eq!(sequence.first(), Some(&ActiveProvider::OpenRouter));
        assert!(sequence.contains(&ActiveProvider::Claude));
        assert!(sequence.contains(&ActiveProvider::Cursor));
    }
}
