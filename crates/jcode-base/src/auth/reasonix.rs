//! Non-secret Reasonix CLI and configuration discovery.
//!
//! Reasonix owns provider credentials. Jcode checks only whether the executable
//! exists and whether a documented project or user config is present and nonempty.

use std::path::{Path, PathBuf};

pub const CLI_PATH_ENV: &str = "JCODE_REASONIX_CLI_PATH";
pub const PROJECT_CONFIG_FILE: &str = "reasonix.toml";
pub const USER_CONFIG_FILE: &str = "config.toml";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReasonixConfigPresence {
    pub project: bool,
    pub user: bool,
}

impl ReasonixConfigPresence {
    pub fn any(self) -> bool {
        self.project || self.user
    }
}

fn nonempty_path(path: Option<PathBuf>) -> Option<PathBuf> {
    path.filter(|path| !path.as_os_str().is_empty())
}

fn resolve_cli_path(configured: Option<PathBuf>, environment: Option<PathBuf>) -> PathBuf {
    nonempty_path(environment)
        .or_else(|| nonempty_path(configured))
        .unwrap_or_else(|| PathBuf::from("reasonix"))
}

pub fn cli_path() -> PathBuf {
    resolve_cli_path(
        crate::config::config().acp.reasonix_cli_path.clone(),
        std::env::var_os(CLI_PATH_ENV).map(PathBuf::from),
    )
}

pub fn cli_available() -> bool {
    super::command_exists(cli_path().to_string_lossy().as_ref())
}

pub fn runtime_not_installed_hint() -> String {
    format!(
        "Reasonix runtime is not installed. Install the official `reasonix` CLI or configure `acp.reasonix_cli_path` / {CLI_PATH_ENV}."
    )
}

pub fn setup_required_hint() -> &'static str {
    "Reasonix configuration not found. Run `reasonix setup`, then retry."
}

pub fn config_presence() -> ReasonixConfigPresence {
    config_presence_from(
        std::env::current_dir().ok().as_deref(),
        std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(PathBuf::from)
            .as_deref(),
        std::env::var_os("REASONIX_HOME")
            .map(PathBuf::from)
            .as_deref(),
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .as_deref(),
    )
}

fn config_presence_from(
    cwd: Option<&Path>,
    home: Option<&Path>,
    reasonix_home: Option<&Path>,
    xdg_config_home: Option<&Path>,
) -> ReasonixConfigPresence {
    ReasonixConfigPresence {
        project: cwd
            .map(|cwd| cwd.join(PROJECT_CONFIG_FILE))
            .is_some_and(|path| nonempty_file(&path)),
        user: user_config_candidates(home, reasonix_home, xdg_config_home)
            .iter()
            .any(|path| nonempty_file(path)),
    }
}

fn user_config_candidates(
    home: Option<&Path>,
    reasonix_home: Option<&Path>,
    xdg_config_home: Option<&Path>,
) -> Vec<PathBuf> {
    if let Some(reasonix_home) = reasonix_home.filter(|path| !path.as_os_str().is_empty()) {
        return vec![reasonix_home.join(USER_CONFIG_FILE)];
    }

    let mut paths = Vec::new();
    if let Some(home) = home {
        paths.push(home.join(".reasonix").join(USER_CONFIG_FILE));
        #[cfg(target_os = "macos")]
        paths.push(
            home.join("Library/Application Support/reasonix")
                .join(USER_CONFIG_FILE),
        );
        #[cfg(target_os = "windows")]
        paths.push(home.join("AppData/Roaming/reasonix").join(USER_CONFIG_FILE));
    }
    if let Some(xdg) = xdg_config_home.filter(|path| !path.as_os_str().is_empty()) {
        paths.push(xdg.join("reasonix").join(USER_CONFIG_FILE));
    }
    if let Some(home) = home {
        paths.push(home.join(".config/reasonix").join(USER_CONFIG_FILE));
    }
    paths
}

fn nonempty_file(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|metadata| metadata.is_file() && metadata.len() > 0)
}

pub fn is_available() -> bool {
    cli_available() && config_presence().any()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn environment_precedes_config_then_path_lookup() {
        assert_eq!(
            resolve_cli_path(
                Some(PathBuf::from("/configured/reasonix")),
                Some(PathBuf::from("/env/reasonix")),
            ),
            PathBuf::from("/env/reasonix")
        );
        assert_eq!(
            resolve_cli_path(Some(PathBuf::from("/configured/reasonix")), None),
            PathBuf::from("/configured/reasonix")
        );
        assert_eq!(resolve_cli_path(None, None), PathBuf::from("reasonix"));
    }

    #[test]
    fn status_reads_only_documented_config_presence() {
        let project = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        assert_eq!(
            config_presence_from(Some(project.path()), Some(home.path()), None, None),
            ReasonixConfigPresence::default()
        );

        std::fs::write(project.path().join(PROJECT_CONFIG_FILE), "").unwrap();
        assert!(!config_presence_from(Some(project.path()), Some(home.path()), None, None).project);
        std::fs::write(project.path().join(PROJECT_CONFIG_FILE), "[provider]\n").unwrap();
        assert!(config_presence_from(Some(project.path()), Some(home.path()), None, None).project);

        let user_config = home.path().join(".reasonix").join(USER_CONFIG_FILE);
        std::fs::create_dir_all(user_config.parent().unwrap()).unwrap();
        std::fs::write(&user_config, "provider = 'deepseek'\n").unwrap();
        let presence = config_presence_from(Some(project.path()), Some(home.path()), None, None);
        assert!(presence.project);
        assert!(presence.user);
    }

    #[test]
    fn status_does_not_treat_secret_env_file_as_configuration() {
        let project = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        let secret = home.path().join(".reasonix/.env");
        std::fs::create_dir_all(secret.parent().unwrap()).unwrap();
        std::fs::write(secret, "DEEPSEEK_API_KEY=secret\n").unwrap();
        assert_eq!(
            config_presence_from(Some(project.path()), Some(home.path()), None, None),
            ReasonixConfigPresence::default()
        );
    }

    #[test]
    fn reasonix_home_override_is_authoritative() {
        let home = tempfile::tempdir().unwrap();
        let override_home = tempfile::tempdir().unwrap();
        let default_config = home.path().join(".reasonix/config.toml");
        std::fs::create_dir_all(default_config.parent().unwrap()).unwrap();
        std::fs::write(default_config, "provider = 'default'\n").unwrap();

        assert!(
            !config_presence_from(None, Some(home.path()), Some(override_home.path()), None,).user
        );
        std::fs::write(
            override_home.path().join(USER_CONFIG_FILE),
            "provider = 'override'\n",
        )
        .unwrap();
        assert!(
            config_presence_from(None, Some(home.path()), Some(override_home.path()), None,).user
        );
    }
}
