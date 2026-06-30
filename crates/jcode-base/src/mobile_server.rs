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
    if let Some(path) = std::env::var_os("JCODE_HOME") {
        return PathBuf::from(path);
    }
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".jcode")
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
