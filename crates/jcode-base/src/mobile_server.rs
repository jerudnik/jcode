use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const STATUS_FILE: &str = "mobile-server.json";
const LOG_FILE: &str = "mobile-server.log";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MobileServerStatus {
    pub pid: u32,
    pub port: u16,
    pub bind_addr: String,
    pub url: String,
    pub web_root: PathBuf,
    pub log_path: PathBuf,
    pub started_at_unix: u64,
}

impl MobileServerStatus {
    pub fn is_running(&self) -> bool {
        process_is_running(self.pid)
    }
}

pub fn jcode_home() -> PathBuf {
    // Delegates rather than reimplementing. This function used to spell out its
    // own `JCODE_HOME`-then-`dirs::home_dir()` rule, which is the canonical
    // resolver's rule minus the test-harness redirect, so it drifted the moment
    // the resolver grew one. Keeping the fallback here preserves the old
    // infallible signature; `jcode_dir()` only fails when no home can be
    // resolved at all.
    crate::storage::jcode_dir().unwrap_or_else(|_| PathBuf::from(".").join(".jcode"))
}

pub fn status_path() -> PathBuf {
    jcode_home().join(STATUS_FILE)
}

pub fn log_path() -> PathBuf {
    jcode_home().join(LOG_FILE)
}

pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn read_status() -> Option<MobileServerStatus> {
    let path = status_path();
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

pub fn read_running_status() -> Option<MobileServerStatus> {
    read_status().filter(MobileServerStatus::is_running)
}

pub fn write_status(status: &MobileServerStatus) -> anyhow::Result<()> {
    let path = status_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(status)?)?;
    std::fs::rename(tmp, path)?;
    Ok(())
}

pub fn clear_status_if_pid(pid: u32) -> anyhow::Result<()> {
    if read_status().is_some_and(|status| status.pid == pid) {
        let path = status_path();
        if path.exists() {
            std::fs::remove_file(path)?;
        }
    }
    Ok(())
}

pub fn process_is_running(pid: u32) -> bool {
    #[cfg(unix)]
    {
        if pid == 0 {
            return false;
        }
        unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        false
    }
}

#[cfg(test)]
mod ambient_root_tests {
    use super::*;

    /// `jcode_home()` used to spell out its own `JCODE_HOME`-then-`dirs::home_dir()`
    /// rule. That is the canonical resolver's rule minus the test-harness
    /// redirect, so every path built from it (status file, log file) escaped
    /// isolation. Reverting the body to a hand-rolled resolver fails this.
    ///
    /// Holds a read lease for the same reason as
    /// `log_dir_never_resolves_into_the_real_home`: `jcode_home()` and
    /// `jcode_dir()` each resolve `JCODE_HOME` on their own, so a sibling
    /// writer landing between them fails the equality on two valid homes.
    #[test]
    fn mobile_server_paths_never_resolve_into_the_real_home() {
        let _env = crate::storage::lock_test_env_read();

        let real = dirs::home_dir()
            .expect("developer home exists")
            .join(".jcode");

        assert_ne!(
            jcode_home(),
            real,
            "mobile server home must honor the storage redirect"
        );
        assert_eq!(
            jcode_home(),
            crate::storage::jcode_dir().expect("storage home"),
            "mobile server home must be exactly jcode_dir()"
        );
        for path in [status_path(), log_path()] {
            assert!(
                !path.starts_with(&real),
                "derived path escaped the redirect: {}",
                path.display()
            );
        }
    }
}
