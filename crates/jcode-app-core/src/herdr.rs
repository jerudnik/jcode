use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

static HERDR_SEQ: AtomicU64 = AtomicU64::new(0);
const HERDR_SEND_TIMEOUT: Duration = Duration::from_millis(50);

#[derive(Clone, Debug, PartialEq, Eq)]
struct HerdrConfig {
    socket_path: PathBuf,
    pane_id: String,
    workspace_id: Option<String>,
    tab_id: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HerdrReporter {
    config: Option<HerdrConfig>,
}

impl HerdrReporter {
    pub fn from_env() -> Self {
        Self::from_env_lookup(|key| std::env::var(key).ok())
    }

    fn from_env_lookup(mut get: impl FnMut(&str) -> Option<String>) -> Self {
        let jcode_herdr = get("JCODE_HERDR");
        if env_opted_out(jcode_herdr.as_deref()) {
            return Self::default();
        }
        let explicitly_enabled = env_opted_in(jcode_herdr.as_deref());
        if !explicitly_enabled && get("HERDR_ENV").as_deref() != Some("1") {
            return Self::default();
        }
        let Some(socket_path) = non_empty(get("HERDR_SOCKET_PATH"))
            .or_else(|| non_empty(get("JCODE_HERDR_SOCKET_PATH")))
        else {
            return Self::default();
        };
        let Some(pane_id) =
            non_empty(get("HERDR_PANE_ID")).or_else(|| non_empty(get("JCODE_HERDR_PANE_ID")))
        else {
            return Self::default();
        };

        Self {
            config: Some(HerdrConfig {
                socket_path: PathBuf::from(socket_path),
                pane_id,
                workspace_id: non_empty(get("HERDR_WORKSPACE_ID"))
                    .or_else(|| non_empty(get("JCODE_HERDR_WORKSPACE_ID"))),
                tab_id: non_empty(get("HERDR_TAB_ID"))
                    .or_else(|| non_empty(get("JCODE_HERDR_TAB_ID"))),
            }),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.config.is_some()
    }

    pub fn report_agent(&self, state: &'static str, custom_status: impl Into<String>) {
        self.report("pane.report_agent", state, custom_status.into(), None, None);
    }

    pub fn report_agent_session(
        &self,
        session_id: impl Into<String>,
        session_path: Option<PathBuf>,
        state: &'static str,
        custom_status: impl Into<String>,
    ) {
        let session_id = session_id.into();
        let custom_status = custom_status.into();
        self.report(
            "pane.report_agent_session",
            "idle",
            "session".to_string(),
            Some(session_id.clone()),
            session_path.clone(),
        );
        self.report(
            "pane.report_agent",
            state,
            custom_status,
            Some(session_id),
            session_path,
        );
    }

    pub fn release_agent(&self, session_id: impl Into<String>) {
        self.report(
            "pane.release_agent",
            "idle",
            "released".to_string(),
            Some(session_id.into()),
            None,
        );
    }

    fn report(
        &self,
        method: &'static str,
        state: &'static str,
        custom_status: String,
        session_id: Option<String>,
        session_path: Option<PathBuf>,
    ) {
        let Some(config) = self.config.clone() else {
            return;
        };
        let seq = next_seq();
        let payload = build_request(
            &config,
            seq,
            method,
            state,
            &custom_status,
            session_id.as_deref(),
            session_path.as_deref(),
        );

        let Ok(handle) = tokio::runtime::Handle::try_current() else {
            return;
        };
        handle.spawn(async move {
            if let Err(error) = send_payload(&config.socket_path, payload).await {
                crate::logging::debug(&format!("Herdr report failed: {error}"));
            }
        });
    }

    #[cfg(test)]
    fn for_test(socket_path: impl Into<PathBuf>, pane_id: impl Into<String>) -> Self {
        Self {
            config: Some(HerdrConfig {
                socket_path: socket_path.into(),
                pane_id: pane_id.into(),
                workspace_id: Some("workspace-test".to_string()),
                tab_id: Some("tab-test".to_string()),
            }),
        }
    }

    #[cfg(test)]
    async fn send_test_report(
        &self,
        state: &'static str,
        custom_status: &str,
    ) -> std::io::Result<()> {
        let Some(config) = self.config.as_ref() else {
            return Ok(());
        };
        let payload = build_request(
            config,
            42,
            "pane.report_agent",
            state,
            custom_status,
            None,
            None,
        );
        send_payload(&config.socket_path, payload).await
    }
}

fn next_seq() -> u64 {
    let wall_seq = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0);

    loop {
        let current = HERDR_SEQ.load(Ordering::Relaxed);
        let next = wall_seq.max(current.saturating_add(1));
        if HERDR_SEQ
            .compare_exchange(current, next, Ordering::Relaxed, Ordering::Relaxed)
            .is_ok()
        {
            return next;
        }
    }
}

fn env_opted_out(value: Option<&str>) -> bool {
    let Some(value) = value else {
        return false;
    };
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "0" | "false" | "no" | "off" | "disable" | "disabled"
    )
}

fn env_opted_in(value: Option<&str>) -> bool {
    let Some(value) = value else {
        return false;
    };
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on" | "enable" | "enabled"
    )
}

fn non_empty(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn build_request(
    config: &HerdrConfig,
    seq: u64,
    method: &'static str,
    state: &'static str,
    custom_status: &str,
    agent_session_id: Option<&str>,
    agent_session_path: Option<&Path>,
) -> Value {
    let mut params = json!({
        "pane_id": config.pane_id,
        "source": "jcode",
        "agent": "jcode",
        "state": state,
        "custom_status": custom_status,
        "seq": seq,
    });

    if let Some(workspace_id) = config.workspace_id.as_deref() {
        params["workspace_id"] = json!(workspace_id);
    }
    if let Some(tab_id) = config.tab_id.as_deref() {
        params["tab_id"] = json!(tab_id);
    }
    if let Some(agent_session_id) = agent_session_id.filter(|value| !value.is_empty()) {
        params["agent_session_id"] = json!(agent_session_id);
    }
    if let Some(agent_session_path) = agent_session_path {
        params["agent_session_path"] = json!(agent_session_path.display().to_string());
    }

    json!({
        "id": format!("jcode-herdr:{}:{}", std::process::id(), seq),
        "method": method,
        "params": params,
    })
}

async fn send_payload(socket_path: &Path, payload: Value) -> std::io::Result<()> {
    send_payload_impl(socket_path, payload).await
}

#[cfg(unix)]
async fn send_payload_impl(socket_path: &Path, payload: Value) -> std::io::Result<()> {
    use tokio::io::AsyncWriteExt;
    use tokio::net::UnixStream;
    use tokio::time::timeout;

    timeout(HERDR_SEND_TIMEOUT, async move {
        let mut stream = UnixStream::connect(socket_path).await?;
        let mut line = serde_json::to_vec(&payload).map_err(std::io::Error::other)?;
        line.push(b'\n');
        stream.write_all(&line).await?;
        stream.flush().await
    })
    .await
    .map_err(|_| std::io::Error::new(std::io::ErrorKind::TimedOut, "Herdr report timed out"))?
}

#[cfg(not(unix))]
async fn send_payload_impl(_socket_path: &Path, _payload: Value) -> std::io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn detect(vars: &[(&str, &str)]) -> HerdrReporter {
        let vars: HashMap<&str, &str> = vars.iter().copied().collect();
        HerdrReporter::from_env_lookup(|key| vars.get(key).map(|value| (*value).to_string()))
    }

    #[test]
    fn env_detection_requires_herdr_env_socket_and_pane() {
        assert!(!detect(&[]).is_enabled());
        assert!(!detect(&[("HERDR_ENV", "1")]).is_enabled());
        assert!(
            detect(&[
                ("HERDR_ENV", "1"),
                ("HERDR_SOCKET_PATH", "/tmp/herdr.sock"),
                ("HERDR_PANE_ID", "pane-1"),
            ])
            .is_enabled()
        );
    }

    #[test]
    fn env_detection_honors_opt_out() {
        let reporter = detect(&[
            ("JCODE_HERDR", "0"),
            ("HERDR_ENV", "1"),
            ("HERDR_SOCKET_PATH", "/tmp/herdr.sock"),
            ("HERDR_PANE_ID", "pane-1"),
        ]);
        assert!(!reporter.is_enabled());
    }

    #[test]
    fn env_detection_supports_explicit_jcode_aliases() {
        let reporter = detect(&[
            ("JCODE_HERDR", "enabled"),
            ("JCODE_HERDR_SOCKET_PATH", "/tmp/herdr.sock"),
            ("JCODE_HERDR_PANE_ID", "pane-1"),
            ("JCODE_HERDR_WORKSPACE_ID", "workspace-1"),
            ("JCODE_HERDR_TAB_ID", "tab-1"),
        ]);
        assert!(reporter.is_enabled());
        let config = reporter.config.expect("herdr config");
        assert_eq!(config.socket_path, PathBuf::from("/tmp/herdr.sock"));
        assert_eq!(config.pane_id, "pane-1");
        assert_eq!(config.workspace_id.as_deref(), Some("workspace-1"));
        assert_eq!(config.tab_id.as_deref(), Some("tab-1"));
    }

    #[test]
    fn serialized_payload_shape_matches_herdr_report() {
        let config = HerdrConfig {
            socket_path: PathBuf::from("/tmp/herdr.sock"),
            pane_id: "pane-1".to_string(),
            workspace_id: Some("workspace-1".to_string()),
            tab_id: Some("tab-1".to_string()),
        };
        let payload = build_request(
            &config,
            7,
            "pane.report_agent_session",
            "working",
            "thinking",
            Some("session-1"),
            Some(Path::new("/tmp/session.json")),
        );
        assert_eq!(payload["method"], "pane.report_agent_session");
        assert_eq!(
            payload["id"],
            format!("jcode-herdr:{}:7", std::process::id())
        );
        assert_eq!(payload["params"]["pane_id"], "pane-1");
        assert_eq!(payload["params"]["source"], "jcode");
        assert_eq!(payload["params"]["agent"], "jcode");
        assert_eq!(payload["params"]["state"], "working");
        assert_eq!(payload["params"]["custom_status"], "thinking");
        assert_eq!(payload["params"]["seq"], 7);
        assert_eq!(payload["params"]["agent_session_id"], "session-1");
        assert_eq!(payload["params"]["agent_session_path"], "/tmp/session.json");
        assert_eq!(payload["params"]["workspace_id"], "workspace-1");
        assert_eq!(payload["params"]["tab_id"], "tab-1");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn unix_socket_report_writes_one_json_line() {
        use tokio::io::{AsyncBufReadExt, BufReader};
        use tokio::net::UnixListener;

        let temp = tempfile::tempdir().expect("tempdir");
        let socket_path = temp.path().join("herdr.sock");
        let listener = UnixListener::bind(&socket_path).expect("bind herdr test socket");
        let capture = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept");
            let mut reader = BufReader::new(stream);
            let mut line = String::new();
            reader.read_line(&mut line).await.expect("read line");
            serde_json::from_str::<Value>(&line).expect("json payload")
        });

        let reporter = HerdrReporter::for_test(&socket_path, "pane-test");
        reporter
            .send_test_report("working", "searching")
            .await
            .expect("send herdr report");
        let payload = capture.await.expect("capture task");
        assert_eq!(payload["method"], "pane.report_agent");
        assert_eq!(payload["params"]["pane_id"], "pane-test");
        assert_eq!(payload["params"]["state"], "working");
        assert_eq!(payload["params"]["custom_status"], "searching");
    }
}
