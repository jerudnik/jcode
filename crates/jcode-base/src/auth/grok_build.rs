//! Grok Build CLI discovery and subscription-readiness state.
//!
//! Jcode never downloads or reads Grok tokens. The official CLI owns the
//! credential format and validates it over ACP. This module only discovers the
//! executable and detects whether a cached login exists so provider activation
//! can stay honest before the first ACP probe.

use std::path::{Path, PathBuf};

pub const CLI_PATH_ENV: &str = "JCODE_GROK_CLI_PATH";
pub const INSTALL_COMMAND: &str =
    "NIXPKGS_ALLOW_UNFREE=1 nix profile install --impure nixpkgs#grok-build";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GrokBuildReadiness {
    RuntimeNotInstalled,
    AuthenticationRequired,
    LoginDetected,
    AuthenticatedNoModels,
    SubscriptionReady,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GrokBuildAuthState {
    pub backend_installed: bool,
    pub login_detected: bool,
    pub login_validated: bool,
    pub subscription_ready: bool,
}

impl GrokBuildAuthState {
    pub fn from_signals(
        backend_installed: bool,
        login_detected: bool,
        login_validated: bool,
        models_available: bool,
    ) -> Self {
        let login_detected = backend_installed && login_detected;
        let login_validated = login_detected && login_validated;
        Self {
            backend_installed,
            login_detected,
            login_validated,
            subscription_ready: login_validated && models_available,
        }
    }

    pub fn readiness(self) -> GrokBuildReadiness {
        match self {
            Self {
                backend_installed: false,
                ..
            } => GrokBuildReadiness::RuntimeNotInstalled,
            Self {
                login_detected: false,
                ..
            } => GrokBuildReadiness::AuthenticationRequired,
            Self {
                login_validated: false,
                ..
            } => GrokBuildReadiness::LoginDetected,
            Self {
                subscription_ready: false,
                ..
            } => GrokBuildReadiness::AuthenticatedNoModels,
            _ => GrokBuildReadiness::SubscriptionReady,
        }
    }
}

fn nonempty_path(path: Option<PathBuf>) -> Option<PathBuf> {
    path.filter(|path| !path.as_os_str().is_empty())
}

fn resolve_cli_path(configured: Option<PathBuf>, environment: Option<PathBuf>) -> PathBuf {
    nonempty_path(environment)
        .or_else(|| nonempty_path(configured))
        .unwrap_or_else(|| PathBuf::from("grok"))
}

pub fn cli_path() -> PathBuf {
    resolve_cli_path(
        crate::config::config().acp.grok_cli_path.clone(),
        std::env::var_os(CLI_PATH_ENV).map(PathBuf::from),
    )
}

pub fn cli_available() -> bool {
    super::command_exists(cli_path().to_string_lossy().as_ref())
}

pub fn runtime_not_installed_hint() -> String {
    format!(
        "Grok Build runtime is not installed. Run `{INSTALL_COMMAND}`, allow grok-build in your Nix configuration, or configure `acp.grok_cli_path` / {CLI_PATH_ENV}."
    )
}

fn auth_file_path() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .map(|home| home.join(".grok").join("auth.json"))
}

/// Detect only the presence of a CLI-owned cached login. Credential bytes are
/// never returned, copied, or passed into jcode's provider runtime.
pub fn has_cached_login() -> bool {
    auth_file_path().is_some_and(|path| cached_login_file_detected(&path))
}

fn is_available_from_signals(cli_available: bool, cached_login: bool) -> bool {
    cli_available && cached_login
}

pub fn is_available() -> bool {
    is_available_from_signals(cli_available(), has_cached_login())
}

fn cached_login_file_detected(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|metadata| metadata.is_file() && metadata.len() > 0)
}

pub fn detected_state() -> GrokBuildAuthState {
    GrokBuildAuthState::from_signals(cli_available(), has_cached_login(), false, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn environment_precedes_config_then_path_lookup() {
        assert_eq!(
            resolve_cli_path(
                Some(PathBuf::from("/configured/grok")),
                Some(PathBuf::from("/env/grok")),
            ),
            PathBuf::from("/env/grok")
        );
        assert_eq!(
            resolve_cli_path(Some(PathBuf::from("/configured/grok")), None),
            PathBuf::from("/configured/grok")
        );
        assert_eq!(
            resolve_cli_path(None, Some(PathBuf::from("/env/grok"))),
            PathBuf::from("/env/grok")
        );
        assert_eq!(resolve_cli_path(None, None), PathBuf::from("grok"));
    }

    #[test]
    fn readiness_transitions_are_monotonic_and_distinct() {
        let cases = [
            (
                GrokBuildAuthState::from_signals(false, false, false, false),
                GrokBuildReadiness::RuntimeNotInstalled,
            ),
            (
                GrokBuildAuthState::from_signals(true, false, false, false),
                GrokBuildReadiness::AuthenticationRequired,
            ),
            (
                GrokBuildAuthState::from_signals(true, true, false, false),
                GrokBuildReadiness::LoginDetected,
            ),
            (
                GrokBuildAuthState::from_signals(true, true, true, false),
                GrokBuildReadiness::AuthenticatedNoModels,
            ),
            (
                GrokBuildAuthState::from_signals(true, true, true, true),
                GrokBuildReadiness::SubscriptionReady,
            ),
        ];
        for (state, expected) in cases {
            assert_eq!(state.readiness(), expected);
        }
    }

    #[test]
    fn impossible_later_signals_do_not_skip_required_states() {
        let state = GrokBuildAuthState::from_signals(false, true, true, true);
        assert_eq!(state.readiness(), GrokBuildReadiness::RuntimeNotInstalled);
        assert!(!state.login_detected);
        assert!(!state.login_validated);
        assert!(!state.subscription_ready);
    }

    #[test]
    fn install_hint_is_nix_only_and_mentions_unfree() {
        let hint = runtime_not_installed_hint();
        assert!(hint.contains(INSTALL_COMMAND));
        assert!(hint.contains("NIXPKGS_ALLOW_UNFREE"));
        assert!(!hint.contains("curl"));
        assert!(!hint.contains("download"));
    }

    #[test]
    fn cached_login_detection_never_parses_cli_credentials() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("auth.json");
        assert!(!cached_login_file_detected(&path));
        std::fs::write(&path, b"").unwrap();
        assert!(!cached_login_file_detected(&path));
        std::fs::write(&path, b"opaque-cli-owned-credential").unwrap();
        assert!(cached_login_file_detected(&path));
    }

    #[test]
    fn provider_availability_requires_runtime_and_cached_login() {
        assert!(!is_available_from_signals(false, false));
        assert!(!is_available_from_signals(false, true));
        assert!(!is_available_from_signals(true, false));
        assert!(is_available_from_signals(true, true));
    }
}
