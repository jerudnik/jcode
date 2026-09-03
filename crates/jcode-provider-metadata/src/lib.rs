#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoginProviderAuthKind {
    OAuth,
    ApiKey,
    DeviceCode,
    Cli,
    Hybrid,
    Local,
}

impl LoginProviderAuthKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::OAuth => "OAuth",
            Self::ApiKey => "API key",
            Self::DeviceCode => "device code",
            Self::Cli => "CLI",
            Self::Hybrid => "API key / CLI",
            Self::Local => "local endpoint",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoginProviderTarget {
    AutoImport,
    Jcode,
    Claude,
    ClaudeApiKey,
    OpenAi,
    OpenAiApiKey,
    OpenRouter,
    Bedrock,
    Azure,
    OpenAiCompatible(OpenAiCompatibleProfile),
    Cursor,
    GrokBuild,
    KimiCodeAcp,
    Reasonix,
    Copilot,
    Gemini,
    Antigravity,
    Google,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoginProviderAuthStateKey {
    ExternalImport,
    Jcode,
    Anthropic,
    OpenAi,
    Azure,
    Bedrock,
    OpenRouterLike,
    Copilot,
    Gemini,
    Antigravity,
    Cursor,
    GrokDirect,
    GrokBuild,
    KimiCodeAcp,
    Reasonix,
    Google,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ManagedOAuthProvider {
    Kimi,
    GrokDirect,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenAiCompatibleAuthStrategy {
    ApiKey {
        required: bool,
    },
    ManagedOAuth {
        provider: ManagedOAuthProvider,
        api_key_fallback: bool,
    },
}

impl OpenAiCompatibleAuthStrategy {
    pub const fn requires_api_key(self) -> bool {
        match self {
            Self::ApiKey { required } => required,
            Self::ManagedOAuth {
                api_key_fallback, ..
            } => api_key_fallback,
        }
    }

    pub const fn managed_oauth_provider(self) -> Option<ManagedOAuthProvider> {
        match self {
            Self::ManagedOAuth { provider, .. } => Some(provider),
            Self::ApiKey { .. } => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoginProviderSurface {
    CliLogin,
    TuiLogin,
    ServerBootstrap,
    AutoInit,
    AuthStatus,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LoginProviderSurfaceOrder {
    pub cli_login: Option<u8>,
    pub tui_login: Option<u8>,
    pub server_bootstrap: Option<u8>,
    pub auto_init: Option<u8>,
    pub auth_status: Option<u8>,
}

impl LoginProviderSurfaceOrder {
    pub const fn new(
        cli_login: Option<u8>,
        tui_login: Option<u8>,
        server_bootstrap: Option<u8>,
        auto_init: Option<u8>,
        auth_status: Option<u8>,
    ) -> Self {
        Self {
            cli_login,
            tui_login,
            server_bootstrap,
            auto_init,
            auth_status,
        }
    }

    pub const fn for_surface(self, surface: LoginProviderSurface) -> Option<u8> {
        match surface {
            LoginProviderSurface::CliLogin => self.cli_login,
            LoginProviderSurface::TuiLogin => self.tui_login,
            LoginProviderSurface::ServerBootstrap => self.server_bootstrap,
            LoginProviderSurface::AutoInit => self.auto_init,
            LoginProviderSurface::AuthStatus => self.auth_status,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LoginProviderDescriptor {
    pub id: &'static str,
    pub display_name: &'static str,
    pub auth_kind: LoginProviderAuthKind,
    pub auth_state_key: LoginProviderAuthStateKey,
    pub auth_status_method: &'static str,
    pub aliases: &'static [&'static str],
    pub menu_detail: &'static str,
    pub recommended: bool,
    pub target: LoginProviderTarget,
    pub order: LoginProviderSurfaceOrder,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpenAiCompatibleProfile {
    pub id: &'static str,
    pub display_name: &'static str,
    pub api_base: &'static str,
    pub api_key_env: &'static str,
    /// Ordered read-only compatibility aliases for credentials. New credentials
    /// are always written under [`Self::api_key_env`].
    pub api_key_aliases: &'static [&'static str],
    pub env_file: &'static str,
    pub setup_url: &'static str,
    pub default_model: Option<&'static str>,
    pub auth_strategy: OpenAiCompatibleAuthStrategy,
    /// Compatibility projection for existing callers. New auth decisions should
    /// use [`Self::auth_strategy`].
    pub requires_api_key: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedOpenAiCompatibleProfile {
    pub id: String,
    pub display_name: String,
    pub api_base: String,
    pub api_key_env: String,
    pub api_key_aliases: Vec<String>,
    pub env_file: String,
    pub setup_url: String,
    pub default_model: Option<String>,
    pub auth_strategy: OpenAiCompatibleAuthStrategy,
    pub requires_api_key: bool,
}

mod catalog;
mod compat_profiles;

pub use catalog::*;
use catalog::{LOGIN_PROVIDERS, OPENAI_COMPAT_PROFILES};

pub fn openai_compatible_profiles() -> &'static [OpenAiCompatibleProfile] {
    &OPENAI_COMPAT_PROFILES
}

/// Providers whose model catalog is served from a different route than their
/// chat endpoint, keyed by `api_base`.
///
/// Perplexity serves chat at the bare base but its catalog under `/v1`, so one
/// `api_base` cannot express both and appending `/v1` would trade a broken
/// catalog for broken chat.
///
/// This is a sparse table rather than a field on `OpenAiCompatibleProfile`
/// deliberately. A field would force `models_path: None` onto all 36 literals
/// for the sake of one exception, which both buries the exception in noise and
/// pushes `catalog.rs` past the 1200-line oversize threshold this repository
/// treats as a review flag (1172 + 36 = 1208 at minimum).
const CATALOG_PATH_OVERRIDES: [(&str, &str); 1] = [("https://api.perplexity.ai", "/v1/models")];

/// Resolve the model-catalog URL for an OpenAI-compatible `api_base`.
///
/// Keyed on `api_base` rather than on a profile because two of the three
/// callers only ever hold a bare base string: `fetch_models_from_api` takes an
/// `api_base: String` and has five callers, none with a profile in scope.
///
/// Note that `api_base` is NOT unique across profiles (`openai-api` and
/// `openai-compatible` share one), so an override applies to every profile
/// sharing that base. That is asserted in the tests rather than left implicit.
pub fn openai_compatible_models_url(api_base: &str) -> String {
    let base = api_base.trim_end_matches('/');
    for (override_base, path) in CATALOG_PATH_OVERRIDES {
        if base == override_base.trim_end_matches('/') {
            return format!("{base}{path}");
        }
    }
    format!("{base}/models")
}

pub fn login_providers() -> &'static [LoginProviderDescriptor] {
    &LOGIN_PROVIDERS
}

fn login_providers_for_surface(surface: LoginProviderSurface) -> Vec<LoginProviderDescriptor> {
    let mut providers = login_providers()
        .iter()
        .copied()
        .filter(|provider| provider.order.for_surface(surface).is_some())
        .collect::<Vec<_>>();
    providers.sort_by_key(|provider| provider.order.for_surface(surface).unwrap_or(u8::MAX));
    providers
}

pub fn cli_login_providers() -> Vec<LoginProviderDescriptor> {
    login_providers_for_surface(LoginProviderSurface::CliLogin)
}

pub fn tui_login_providers() -> Vec<LoginProviderDescriptor> {
    login_providers_for_surface(LoginProviderSurface::TuiLogin)
}

pub fn server_bootstrap_login_providers() -> Vec<LoginProviderDescriptor> {
    login_providers_for_surface(LoginProviderSurface::ServerBootstrap)
}

pub fn auto_init_login_providers() -> Vec<LoginProviderDescriptor> {
    login_providers_for_surface(LoginProviderSurface::AutoInit)
}

pub fn auth_status_login_providers() -> Vec<LoginProviderDescriptor> {
    login_providers_for_surface(LoginProviderSurface::AuthStatus)
}

pub fn resolve_login_provider(input: &str) -> Option<LoginProviderDescriptor> {
    let normalized = normalize_provider_input(input)?;
    login_providers().iter().copied().find(|provider| {
        provider.id == normalized || provider.aliases.iter().any(|alias| *alias == normalized)
    })
}

/// Resolve a login provider by id, alias, or display name.
///
/// Login completion events carry the human-readable provider label (e.g.
/// "Anthropic API") rather than the canonical id/alias, so the stricter
/// [`resolve_login_provider`] (id/alias only) misses them. Auth-change routing
/// needs to map those labels back to a provider id; matching the display name
/// here keeps the post-login model refresh attributed to the correct provider.
pub fn resolve_login_provider_loose(input: &str) -> Option<LoginProviderDescriptor> {
    if let Some(provider) = resolve_login_provider(input) {
        return Some(provider);
    }
    let normalized = normalize_provider_input(input)?;
    login_providers()
        .iter()
        .copied()
        .find(|provider| provider.display_name.to_ascii_lowercase() == normalized)
}

pub fn resolve_login_selection(
    input: &str,
    providers: &[LoginProviderDescriptor],
) -> Option<LoginProviderDescriptor> {
    let trimmed = input.trim();
    if let Ok(index) = trimmed.parse::<usize>() {
        return index
            .checked_sub(1)
            .and_then(|idx| providers.get(idx))
            .copied();
    }

    let provider = resolve_login_provider(trimmed)?;
    providers
        .iter()
        .copied()
        .find(|candidate| candidate.id == provider.id)
}

pub fn is_safe_env_key_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

pub fn is_safe_env_file_name(name: &str) -> bool {
    !name.is_empty()
        && !name.contains('/')
        && !name.contains('\\')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
}

pub fn normalize_api_base(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    let parsed = url::Url::parse(trimmed).ok()?;
    let scheme = parsed.scheme();
    if scheme != "https" && scheme != "http" {
        return None;
    }

    if scheme == "http" {
        let host = parsed.host_str()?;
        if !allows_insecure_http_host(host) {
            return None;
        }
    }

    Some(trimmed.trim_end_matches('/').to_string())
}

fn allows_insecure_http_host(host: &str) -> bool {
    let host = host.trim();
    let host = host
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .unwrap_or(host);
    let host_lower = host.to_ascii_lowercase();
    if host_lower == "localhost" || host_lower.ends_with(".local") {
        return true;
    }

    if let Ok(ip) = host.parse::<std::net::IpAddr>() {
        return match ip {
            std::net::IpAddr::V4(v4) => {
                let raw = u32::from(v4);
                let is_carrier_grade_nat = (raw & 0xffc0_0000) == 0x6440_0000;
                v4.is_loopback()
                    || v4.is_private()
                    || v4.is_link_local()
                    || v4.is_unspecified()
                    || is_carrier_grade_nat
            }
            std::net::IpAddr::V6(v6) => {
                v6.is_loopback()
                    || v6.is_unique_local()
                    || v6.is_unicast_link_local()
                    || v6.is_unspecified()
            }
        };
    }

    false
}

fn normalize_provider_input(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn matrix_profiles_have_unique_ids_and_safe_metadata() {
        let mut ids = HashSet::new();
        let mut primary_envs = HashSet::new();
        for profile in openai_compatible_profiles() {
            assert!(
                ids.insert(profile.id),
                "duplicate provider profile id: {}",
                profile.id
            );
            assert!(is_safe_env_key_name(profile.api_key_env));
            assert!(
                primary_envs.insert(profile.api_key_env),
                "duplicate catalog primary credential env: {}",
                profile.api_key_env
            );
            for alias in profile.api_key_aliases {
                assert!(is_safe_env_key_name(alias));
                assert_ne!(*alias, profile.api_key_env);
            }
            assert!(is_safe_env_file_name(profile.env_file));
            assert_eq!(
                normalize_api_base(profile.api_base).as_deref(),
                Some(profile.api_base)
            );
        }
    }

    #[test]
    fn normalize_api_base_accepts_private_http_hosts() {
        assert_eq!(
            normalize_api_base("http://192.168.1.25:8000/v1/").as_deref(),
            Some("http://192.168.1.25:8000/v1")
        );
        assert_eq!(
            normalize_api_base("http://10.0.0.8:11434/v1").as_deref(),
            Some("http://10.0.0.8:11434/v1")
        );
        assert_eq!(
            normalize_api_base("http://100.103.78.84:11434/v1").as_deref(),
            Some("http://100.103.78.84:11434/v1")
        );
        assert_eq!(
            normalize_api_base("http://hsv.local:11434/v1").as_deref(),
            Some("http://hsv.local:11434/v1")
        );
        assert_eq!(
            normalize_api_base("http://[fd00::1]:8080/v1").as_deref(),
            Some("http://[fd00::1]:8080/v1")
        );
    }

    #[test]
    fn normalize_api_base_rejects_public_http_hosts() {
        assert_eq!(normalize_api_base("http://example.com/v1"), None);
        assert_eq!(normalize_api_base("http://8.8.8.8/v1"), None);
    }

    #[test]
    fn alibaba_coding_plan_uses_current_international_endpoint() {
        assert_eq!(
            ALIBABA_CODING_PLAN_PROFILE.api_base,
            "https://coding-intl.dashscope.aliyuncs.com/v1"
        );
    }

    #[test]
    fn zai_login_identifies_coding_plan_subscription_key() {
        let provider = resolve_login_provider("zai").expect("Z.AI provider");
        assert_eq!(provider.auth_kind, LoginProviderAuthKind::ApiKey);
        assert_eq!(provider.menu_detail, "Coding Plan subscription API key");

        let LoginProviderTarget::OpenAiCompatible(profile) = provider.target else {
            panic!("Z.AI must use its OpenAI-compatible Coding Plan endpoint");
        };
        assert_eq!(profile.api_base, "https://api.z.ai/api/coding/paas/v4");
        assert_eq!(profile.setup_url, "https://docs.z.ai/devpack/quick-start");
        assert_eq!(profile.default_model, Some("glm-5.2"));
        assert_eq!(profile.api_key_env, "ZHIPU_API_KEY");
        assert_eq!(profile.api_key_aliases, &["ZAI_API_KEY"]);
    }

    #[test]
    fn resolve_login_provider_loose_matches_id_alias_and_display_name() {
        // id
        assert_eq!(
            resolve_login_provider_loose("anthropic-api").map(|d| d.id),
            Some("anthropic-api")
        );
        // alias
        assert_eq!(
            resolve_login_provider_loose("claude-api").map(|d| d.id),
            Some("anthropic-api")
        );
        // display name (the form LoginCompleted carries for API-key paste logins)
        assert_eq!(
            resolve_login_provider_loose("Anthropic API").map(|d| d.id),
            Some("anthropic-api")
        );
        // display name is matched case-insensitively
        assert_eq!(
            resolve_login_provider_loose("anthropic api").map(|d| d.id),
            Some("anthropic-api")
        );
        // unknown input stays unresolved
        assert_eq!(resolve_login_provider_loose("not-a-provider"), None);
    }

    #[test]
    fn resolve_login_provider_loose_resolves_every_descriptor_by_id_and_display_name() {
        // Guards the LoginCompleted attribution path: the TUI publishes either a
        // descriptor id (OAuth logins) or a display label (API-key paste logins),
        // and both must resolve so the post-login auth-change refresh is
        // attributed to the right provider instead of falling back to the
        // session's active provider.
        for descriptor in login_providers() {
            assert_eq!(
                resolve_login_provider_loose(descriptor.id).map(|d| d.id),
                Some(descriptor.id),
                "descriptor id {:?} should resolve",
                descriptor.id
            );
            assert_eq!(
                resolve_login_provider_loose(descriptor.display_name).map(|d| d.id),
                Some(descriptor.id),
                "display name {:?} (id {:?}) should resolve",
                descriptor.display_name,
                descriptor.id
            );
        }
    }

    #[test]
    fn minimax_profile_uses_official_openai_compatible_configuration() {
        assert_eq!(MINIMAX_PROFILE.api_base, "https://api.minimax.io/v1");
        assert_eq!(MINIMAX_PROFILE.api_key_env, "MINIMAX_API_KEY");
        assert_eq!(MINIMAX_PROFILE.default_model, Some("MiniMax-M3"));
        assert_eq!(MINIMAX_LOGIN_PROVIDER.menu_detail, MINIMAX_CREDENTIAL_LABEL);
        assert_eq!(
            MINIMAX_LOGIN_PROVIDER.auth_status_method,
            MINIMAX_CREDENTIAL_LABEL
        );
        assert_eq!(
            openai_compatible_profiles()
                .iter()
                .filter(|profile| profile.api_key_env == "OPENAI_API_KEY")
                .map(|profile| profile.id)
                .collect::<Vec<_>>(),
            vec!["openai-api"]
        );
        assert_eq!(ZAI_PROFILE.api_key_aliases, &["ZAI_API_KEY"]);
    }

    #[test]
    fn nvidia_nim_profile_uses_hosted_openai_compatible_configuration() {
        assert_eq!(
            NVIDIA_NIM_PROFILE.api_base,
            "https://integrate.api.nvidia.com/v1"
        );
        assert_eq!(NVIDIA_NIM_PROFILE.api_key_env, "NVIDIA_API_KEY");
        assert_eq!(NVIDIA_NIM_PROFILE.env_file, "nvidia-nim.env");
        assert_eq!(
            NVIDIA_NIM_PROFILE.default_model,
            Some("nvidia/llama-3.1-nemotron-ultra-253b-v1")
        );
        assert!(matches!(
            NVIDIA_NIM_LOGIN_PROVIDER.target,
            LoginProviderTarget::OpenAiCompatible(profile) if profile.id == "nvidia-nim"
        ));
    }

    #[test]
    fn cerebras_profile_uses_official_openai_compatible_configuration() {
        assert_eq!(CEREBRAS_PROFILE.id, "cerebras");
        assert_eq!(CEREBRAS_PROFILE.display_name, "Cerebras");
        assert_eq!(CEREBRAS_PROFILE.api_base, "https://api.cerebras.ai/v1");
        assert_eq!(CEREBRAS_PROFILE.api_key_env, "CEREBRAS_API_KEY");
        assert_eq!(CEREBRAS_PROFILE.env_file, "cerebras.env");
        assert_eq!(
            CEREBRAS_PROFILE.setup_url,
            "https://inference-docs.cerebras.ai/introduction"
        );
        assert_eq!(CEREBRAS_PROFILE.default_model, Some("gpt-oss-120b"));
        const { assert!(CEREBRAS_PROFILE.requires_api_key) };
        assert_eq!(
            CEREBRAS_LOGIN_PROVIDER.auth_kind,
            LoginProviderAuthKind::ApiKey
        );
        assert_eq!(
            CEREBRAS_LOGIN_PROVIDER.auth_state_key,
            LoginProviderAuthStateKey::OpenRouterLike
        );
        assert!(matches!(
            CEREBRAS_LOGIN_PROVIDER.target,
            LoginProviderTarget::OpenAiCompatible(profile) if profile.id == "cerebras"
        ));
    }

    #[test]
    fn ollama_profile_is_local_openai_compatible_without_required_api_key() {
        assert_eq!(OLLAMA_PROFILE.id, "ollama");
        assert_eq!(OLLAMA_PROFILE.api_base, "http://localhost:11434/v1");
        assert_eq!(OLLAMA_PROFILE.api_key_env, "OLLAMA_API_KEY");
        assert_eq!(OLLAMA_PROFILE.env_file, "ollama.env");
        assert_eq!(
            OLLAMA_PROFILE.setup_url,
            "https://docs.ollama.com/api/openai-compatibility"
        );
        assert_eq!(OLLAMA_PROFILE.default_model, None);
        const {
            assert!(!OLLAMA_PROFILE.requires_api_key);
        }

        assert_eq!(
            OLLAMA_LOGIN_PROVIDER.auth_kind,
            LoginProviderAuthKind::Local
        );
        assert_eq!(OLLAMA_LOGIN_PROVIDER.auth_status_method, "local endpoint");
        assert!(matches!(
            OLLAMA_LOGIN_PROVIDER.target,
            LoginProviderTarget::OpenAiCompatible(profile) if profile.id == "ollama"
        ));
    }

    #[test]
    fn matrix_login_provider_aliases_resolve_to_canonical_ids() {
        assert_eq!(
            resolve_login_provider("subscription").map(|provider| provider.id),
            Some("jcode")
        );
        assert_eq!(
            resolve_login_provider("anthropic").map(|provider| provider.id),
            Some("claude")
        );
        assert_eq!(
            resolve_login_provider("opencodego").map(|provider| provider.id),
            Some("opencode-go")
        );
        assert_eq!(
            resolve_login_provider("z.ai").map(|provider| provider.id),
            Some("zai")
        );
        assert_eq!(
            resolve_login_provider("zhipu").map(|provider| provider.id),
            Some("zai")
        );
        assert_eq!(
            resolve_login_provider("kimi").map(|provider| provider.id),
            Some("kimi")
        );
        assert_eq!(
            resolve_login_provider("kimi-for-coding").map(|provider| provider.id),
            Some("kimi")
        );
        assert_eq!(
            resolve_login_provider("compat").map(|provider| provider.id),
            Some("openai-compatible")
        );
        assert_eq!(
            resolve_login_provider("aoai").map(|provider| provider.id),
            Some("azure")
        );
        assert_eq!(
            resolve_login_provider("cerberascode").map(|provider| provider.id),
            Some("cerebras")
        );
        assert_eq!(
            resolve_login_provider("bailian").map(|provider| provider.id),
            Some("alibaba-coding-plan")
        );
        assert_eq!(
            resolve_login_provider("302.ai").map(|provider| provider.id),
            Some("302ai")
        );
        assert_eq!(
            resolve_login_provider("hf").map(|provider| provider.id),
            Some("huggingface")
        );
        assert_eq!(
            resolve_login_provider("moonshot").map(|provider| provider.id),
            Some("moonshotai")
        );
        assert_eq!(
            resolve_login_provider("mistralai").map(|provider| provider.id),
            Some("mistral")
        );
        assert_eq!(
            resolve_login_provider("pplx").map(|provider| provider.id),
            Some("perplexity")
        );
        assert_eq!(
            resolve_login_provider("together").map(|provider| provider.id),
            Some("togetherai")
        );
        assert_eq!(
            resolve_login_provider("deep-infra").map(|provider| provider.id),
            Some("deepinfra")
        );
        assert_eq!(
            resolve_login_provider("fireworks.ai").map(|provider| provider.id),
            Some("fireworks")
        );
        assert_eq!(
            resolve_login_provider("minimax-ai").map(|provider| provider.id),
            Some("minimax")
        );
        assert_eq!(
            resolve_login_provider("grok").map(|provider| provider.id),
            Some("grok-build")
        );
        assert_eq!(
            resolve_login_provider("grok-subscription").map(|provider| provider.id),
            Some("grok-build")
        );
        assert_eq!(
            resolve_login_provider("lm-studio").map(|provider| provider.id),
            Some("lmstudio")
        );
        assert_eq!(
            resolve_login_provider("gmail").map(|provider| provider.id),
            Some("google")
        );
    }

    #[test]
    fn matrix_login_provider_ids_and_aliases_are_unique() {
        let mut seen = HashSet::new();
        for provider in login_providers() {
            assert!(
                seen.insert(provider.id),
                "duplicate login provider identifier: {}",
                provider.id
            );
            for alias in provider.aliases {
                assert!(
                    seen.insert(*alias),
                    "duplicate login provider alias: {}",
                    alias
                );
            }
        }
    }

    #[test]
    fn grok_build_is_cli_subscription_auth_not_xai_api_key_auth() {
        let provider = resolve_login_provider("grok-build").expect("Grok Build provider");
        assert_eq!(provider.auth_kind, LoginProviderAuthKind::Cli);
        assert_eq!(
            provider.auth_state_key,
            LoginProviderAuthStateKey::GrokBuild
        );
        assert_eq!(provider.target, LoginProviderTarget::GrokBuild);
        assert!(
            cli_login_providers()
                .iter()
                .any(|candidate| candidate.id == provider.id)
        );
        assert!(
            auth_status_login_providers()
                .iter()
                .any(|candidate| candidate.id == provider.id)
        );
        assert!(
            !tui_login_providers()
                .iter()
                .any(|candidate| candidate.id == provider.id),
            "Grok Build login must stay terminal-owned"
        );

        let xai = resolve_login_provider("xai").expect("xAI API-key provider");
        assert_eq!(xai.id, "xai");
        assert_eq!(xai.auth_kind, LoginProviderAuthKind::ApiKey);
    }

    #[test]
    fn grok_direct_is_distinct_managed_oauth_profile() {
        let direct = resolve_login_provider("grok-direct").expect("Grok Direct provider");
        assert_eq!(direct.auth_kind, LoginProviderAuthKind::DeviceCode);
        assert_eq!(direct.auth_state_key, LoginProviderAuthStateKey::GrokDirect);
        assert_eq!(
            direct.target,
            LoginProviderTarget::OpenAiCompatible(GROK_DIRECT_PROFILE)
        );
        assert!(direct.menu_detail.contains("Experimental"));
        assert_eq!(
            GROK_DIRECT_PROFILE.auth_strategy,
            OpenAiCompatibleAuthStrategy::ManagedOAuth {
                provider: ManagedOAuthProvider::GrokDirect,
                api_key_fallback: false,
            }
        );
        assert_eq!(GROK_DIRECT_PROFILE.default_model, Some("grok-4.6"));
        assert_eq!(
            resolve_login_provider("grok").map(|provider| provider.id),
            Some("grok-build")
        );
        assert_ne!(direct.id, XAI_LOGIN_PROVIDER.id);
        assert_ne!(direct.id, GROK_BUILD_LOGIN_PROVIDER.id);
    }

    #[test]
    fn kimi_code_acp_is_terminal_owned_cli_auth_not_kimi_api_auth() {
        let provider = resolve_login_provider("kimi-code-acp").expect("Kimi Code ACP provider");
        assert_eq!(provider.auth_kind, LoginProviderAuthKind::Cli);
        assert_eq!(
            provider.auth_state_key,
            LoginProviderAuthStateKey::KimiCodeAcp
        );
        assert_eq!(provider.target, LoginProviderTarget::KimiCodeAcp);
        assert!(
            cli_login_providers()
                .iter()
                .any(|candidate| candidate.id == provider.id)
        );
        assert!(
            auth_status_login_providers()
                .iter()
                .any(|candidate| candidate.id == provider.id)
        );
        assert!(
            !tui_login_providers()
                .iter()
                .any(|candidate| candidate.id == provider.id),
            "Kimi Code ACP login must stay terminal-owned"
        );

        let kimi_api = resolve_login_provider("kimi").expect("Kimi API provider");
        assert_eq!(kimi_api.id, "kimi");
        assert_ne!(kimi_api.target, LoginProviderTarget::KimiCodeAcp);
    }

    #[test]
    fn reasonix_is_terminal_owned_cli_setup_auth() {
        let provider = resolve_login_provider("reasonix").expect("Reasonix provider");
        assert_eq!(provider.auth_kind, LoginProviderAuthKind::Cli);
        assert_eq!(provider.auth_state_key, LoginProviderAuthStateKey::Reasonix);
        assert_eq!(provider.target, LoginProviderTarget::Reasonix);
        assert!(
            cli_login_providers()
                .iter()
                .any(|candidate| candidate.id == provider.id)
        );
        assert!(
            auth_status_login_providers()
                .iter()
                .any(|candidate| candidate.id == provider.id)
        );
        assert!(
            !tui_login_providers()
                .iter()
                .any(|candidate| candidate.id == provider.id),
            "Reasonix setup must stay terminal-owned"
        );
    }

    #[test]
    fn matrix_tui_login_selection_supports_numbers_and_names() {
        let providers = tui_login_providers();
        assert_eq!(
            resolve_login_selection("1", &providers).map(|provider| provider.id),
            Some("auto-import")
        );
        assert_eq!(
            resolve_login_selection("2", &providers).map(|provider| provider.id),
            Some("claude")
        );
        // `anthropic-api` sits at 3 (between claude and openai), shifting the
        // rest of the list down one slot relative to the pre-May-2026 order.
        assert_eq!(
            resolve_login_selection("3", &providers).map(|provider| provider.id),
            Some("anthropic-api")
        );
        assert_eq!(
            resolve_login_selection("7", &providers).map(|provider| provider.id),
            Some("bedrock")
        );
        assert_eq!(
            resolve_login_selection("compat", &providers).map(|provider| provider.id),
            Some("openai-compatible")
        );
        assert!(resolve_login_selection("google", &providers).is_none());
    }

    #[test]
    fn matrix_cli_login_selection_preserves_existing_order() {
        let providers = cli_login_providers();
        assert_eq!(
            resolve_login_selection("1", &providers).map(|provider| provider.id),
            Some("auto-import")
        );
        // `anthropic-api` at 3 shifted everything after it down one slot.
        assert_eq!(
            resolve_login_selection("3", &providers).map(|provider| provider.id),
            Some("anthropic-api")
        );
        assert_eq!(
            resolve_login_selection("5", &providers).map(|provider| provider.id),
            Some("jcode")
        );
        assert_eq!(
            resolve_login_selection("6", &providers).map(|provider| provider.id),
            Some("copilot")
        );
        assert_eq!(
            resolve_login_selection("7", &providers).map(|provider| provider.id),
            Some("openrouter")
        );
        assert_eq!(
            resolve_login_selection("8", &providers).map(|provider| provider.id),
            Some("bedrock")
        );
        assert_eq!(
            resolve_login_selection("9", &providers).map(|provider| provider.id),
            Some("azure")
        );
        assert_eq!(
            resolve_login_selection("bedrock", &providers).map(|provider| provider.id),
            Some("bedrock")
        );
    }

    // --- G02-FIX-1: catalog-route override ------------------------------------

    /// Gate 1's control. FAILS if CATALOG_PATH_OVERRIDES loses the perplexity
    /// entry, which is the state of the tree before this fix.
    #[test]
    fn perplexity_catalog_url_carries_the_v1_prefix() {
        let perplexity = openai_compatible_profiles()
            .iter()
            .find(|profile| profile.id == "perplexity")
            .expect("perplexity profile must exist");
        assert_eq!(
            openai_compatible_models_url(perplexity.api_base),
            "https://api.perplexity.ai/v1/models",
        );
    }

    /// Gate 3. The load-bearing assertion: this is the ONLY test that fails if
    /// the fix were implemented by appending /v1 to api_base, because that edit
    /// also produces /v1/models and so leaves the catalog test above passing.
    /// Perplexity serves chat at the bare base; a /v1 chat URL 404s.
    #[test]
    fn perplexity_chat_base_must_not_carry_a_v1_prefix() {
        let perplexity = openai_compatible_profiles()
            .iter()
            .find(|profile| profile.id == "perplexity")
            .expect("perplexity profile must exist");
        assert_eq!(perplexity.api_base, "https://api.perplexity.ai");
    }

    /// Gate 4. The override must be inert for every other profile, and in
    /// particular for the five other profiles whose api_base does not end in
    /// /v1 -- the population the defect could plausibly have extended to. Each
    /// was re-probed WITH a bearer and answers 200/400/401, never 404.
    #[test]
    fn only_perplexity_overrides_its_catalog_path() {
        assert_eq!(CATALOG_PATH_OVERRIDES.len(), 1);
        for profile in openai_compatible_profiles() {
            if profile.id == "perplexity" {
                continue;
            }
            assert_eq!(
                openai_compatible_models_url(profile.api_base),
                format!("{}/models", profile.api_base.trim_end_matches('/')),
                "profile {} must keep the default catalog route",
                profile.id
            );
        }
        for id in ["zai", "gemini-api", "deepseek", "fpt", "deepinfra"] {
            let profile = openai_compatible_profiles()
                .iter()
                .find(|profile| profile.id == id)
                .unwrap_or_else(|| panic!("profile {id} must exist"));
            assert!(
                !profile.api_base.trim_end_matches('/').ends_with("/v1"),
                "{id} is expected to be one of the non-/v1 profiles"
            );
            assert_eq!(
                openai_compatible_models_url(profile.api_base),
                format!("{}/models", profile.api_base.trim_end_matches('/'))
            );
        }
    }

    /// The lookup key is api_base, which is NOT unique: openai-api and
    /// openai-compatible share https://api.openai.com/v1. An override therefore
    /// applies to every profile sharing that base. Fail loudly if a future
    /// override is added for one of a shared pair, since that would silently
    /// reroute its twin.
    #[test]
    fn overridden_bases_are_not_shared_by_profiles_that_disagree() {
        for (override_base, _) in CATALOG_PATH_OVERRIDES {
            let sharing: Vec<_> = openai_compatible_profiles()
                .iter()
                .filter(|profile| {
                    profile.api_base.trim_end_matches('/') == override_base.trim_end_matches('/')
                })
                .map(|profile| profile.id)
                .collect();
            assert_eq!(
                sharing.len(),
                1,
                "override for {override_base} would also apply to {sharing:?}"
            );
        }
    }

    #[test]
    fn unknown_bases_default_and_trailing_slashes_normalize() {
        assert_eq!(
            openai_compatible_models_url("https://example.test/v1"),
            "https://example.test/v1/models"
        );
        assert_eq!(
            openai_compatible_models_url("https://api.perplexity.ai/"),
            "https://api.perplexity.ai/v1/models"
        );
    }

    /// Gate 2. All three {api_base}/models construction sites must resolve
    /// through the shared helper. A unit test in this crate cannot execute
    /// three downstream crates' network calls, so this asserts against their
    /// source text: it fails, naming the drifted file, if any site is reverted
    /// to a bare format!.
    #[test]
    fn all_catalog_url_sites_route_through_the_resolver() {
        let sites = [
            "crates/jcode-provider-openrouter-runtime/src/lib.rs",
            "crates/jcode-provider-doctor/src/live_provider_probes.rs",
            "crates/jcode-base/src/usage/api_keys.rs",
        ];
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(std::path::Path::parent)
            .expect("workspace root");
        for site in sites {
            let path = root.join(site);
            let source = std::fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("cannot read {site}: {error}"));
            assert!(
                source.contains("openai_compatible_models_url"),
                "{site} no longer routes its catalog URL through \
                 openai_compatible_models_url"
            );
            for line in source.lines() {
                assert!(
                    !line.contains(r#"format!("{}/models""#),
                    "{site} rebuilds the catalog URL with a bare format!: {}",
                    line.trim()
                );
            }
        }
    }
}
