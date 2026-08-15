//! Kimi Code CLI discovery and CLI-owned authentication readiness.
//!
//! Jcode never reads or copies credential bytes. It only detects whether the
//! official CLI has durable local state before exposing its ACP runtime.

use std::path::{Path, PathBuf};

pub const CLI_PATH_ENV: &str = "JCODE_KIMI_CLI_PATH";

fn nonempty_path(path: Option<PathBuf>) -> Option<PathBuf> {
    path.filter(|path| !path.as_os_str().is_empty())
}

fn resolve_cli_path(configured: Option<PathBuf>, environment: Option<PathBuf>) -> PathBuf {
    nonempty_path(environment)
        .or_else(|| nonempty_path(configured))
        .unwrap_or_else(|| PathBuf::from("kimi"))
}

pub fn cli_path() -> PathBuf {
    resolve_cli_path(
        crate::config::config().acp.kimi_cli_path.clone(),
        std::env::var_os(CLI_PATH_ENV).map(PathBuf::from),
    )
}

pub fn cli_available() -> bool {
    super::command_exists(cli_path().to_string_lossy().as_ref())
}

pub fn runtime_not_installed_hint() -> String {
    format!(
        "Kimi Code CLI is not installed. Install the official OAuth-capable CLI from https://code.kimi.com/install.sh, or configure `acp.kimi_cli_path` / {CLI_PATH_ENV}. No API key is required."
    )
}

pub const fn authentication_required_hint() -> &'static str {
    "Kimi Code CLI login is required. Run `jcode login --provider kimi-code-acp` or `kimi login`."
}

fn data_home() -> Option<PathBuf> {
    std::env::var_os("KIMI_CODE_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .map(PathBuf::from)
                .map(|home| home.join(".kimi-code"))
        })
}

/// Detect only the presence of CLI-owned state. Credential bytes remain under
/// `KIMI_CODE_HOME` (normally `~/.kimi-code`) and are never opened by jcode.
pub fn has_cli_owned_state() -> bool {
    data_home().is_some_and(|home| cli_owned_state_detected(&home))
}

pub fn is_available() -> bool {
    cli_available() && has_cli_owned_state()
}

fn cli_owned_state_detected(home: &Path) -> bool {
    let config = home.join("config.toml");
    if std::fs::metadata(config).is_ok_and(|metadata| metadata.is_file() && metadata.len() > 0) {
        return true;
    }
    std::fs::read_dir(home.join("credentials")).is_ok_and(|entries| {
        entries
            .filter_map(Result::ok)
            .any(|entry| entry.metadata().is_ok_and(|metadata| metadata.is_file()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn environment_precedes_config_then_path_lookup() {
        assert_eq!(
            resolve_cli_path(
                Some(PathBuf::from("/configured/kimi")),
                Some(PathBuf::from("/env/kimi")),
            ),
            PathBuf::from("/env/kimi")
        );
        assert_eq!(
            resolve_cli_path(Some(PathBuf::from("/configured/kimi")), None),
            PathBuf::from("/configured/kimi")
        );
        assert_eq!(resolve_cli_path(None, None), PathBuf::from("kimi"));
    }

    #[test]
    fn detects_state_without_reading_credential_contents() {
        let temp = tempfile::tempdir().unwrap();
        assert!(!cli_owned_state_detected(temp.path()));
        std::fs::create_dir(temp.path().join("credentials")).unwrap();
        std::fs::write(temp.path().join("credentials").join("kimi.json"), b"opaque").unwrap();
        assert!(cli_owned_state_detected(temp.path()));
    }
}
