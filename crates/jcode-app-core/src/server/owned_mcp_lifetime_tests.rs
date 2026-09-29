//! Owned (`shared: false`) MCP children must not outlive the session that
//! spawned them. Each test drives a real isolated daemon: a fake stdio MCP
//! server is configured as owned, a headless session is created over the
//! debug socket (which spawns the child), and the session is then ended the
//! way the daemon ends sessions in production. The child PID is the
//! assertion, not any in-memory bookkeeping.

use super::runtime::ServerRuntime;
use super::{Client, Server};
use crate::message::{Message, ToolDefinition};
use crate::protocol::ServerEvent;
use crate::provider::{EventStream, Provider};
use crate::transport::Listener;
use anyhow::Result;
use async_trait::async_trait;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

struct TestProvider;

#[async_trait]
impl Provider for TestProvider {
    async fn complete(
        &self,
        _messages: &[Message],
        _tools: &[ToolDefinition],
        _system: &str,
        _resume_session_id: Option<&str>,
    ) -> Result<EventStream> {
        Ok(Box::pin(futures::stream::empty()))
    }

    fn name(&self) -> &str {
        "test"
    }

    fn fork(&self) -> Arc<dyn Provider> {
        Arc::new(TestProvider)
    }
}

/// A stdio MCP server that records its own PID to `pid_file` and then serves
/// a minimal protocol. It exits on `shutdown` or when stdin closes, which is
/// what a well-behaved server does; the tests below assert the daemon takes
/// one of those two actions.
fn write_fake_owned_server(dir: &Path, pid_file: &Path) -> PathBuf {
    let path = dir.join("fake-owned-mcp.sh");
    let script = format!(
        r##"#!/bin/bash
echo $$ > "{pid}"
while IFS= read -r line; do
  id=$(echo "$line" | grep -o '"id":[0-9]*' | grep -o '[0-9]*' | head -1)
  case "$line" in
    *'"initialize"'*)
      echo '{{"jsonrpc":"2.0","id":'"$id"',"result":{{"protocolVersion":"2024-11-05","capabilities":{{"tools":{{}}}},"serverInfo":{{"name":"fake","version":"0.0.1"}}}}}}'
      ;;
    *'"tools/list"'*)
      echo '{{"jsonrpc":"2.0","id":'"$id"',"result":{{"tools":[{{"name":"ping","description":"fake","inputSchema":{{"type":"object"}}}}]}}}}'
      ;;
    *'"shutdown"'*)
      exit 0
      ;;
  esac
done
"##,
        pid = pid_file.display()
    );
    let mut file = std::fs::File::create(&path).unwrap();
    file.write_all(script.as_bytes()).unwrap();
    drop(file);
    let mut perms = std::fs::metadata(&path).unwrap().permissions();
    use std::os::unix::fs::PermissionsExt;
    perms.set_mode(0o755);
    std::fs::set_permissions(&path, perms).unwrap();
    path
}

fn pid_is_live(pid: u32) -> bool {
    jcode_core::process::is_running(pid)
}

async fn wait_until(deadline: Duration, mut condition: impl FnMut() -> bool) -> bool {
    let started = Instant::now();
    while started.elapsed() < deadline {
        if condition() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    condition()
}

struct Harness {
    _temp: tempfile::TempDir,
    _env: Vec<crate::storage::EnvVarGuard>,
    _runtime: ServerRuntime,
    _accept_main: tokio::task::JoinHandle<()>,
    _accept_debug: tokio::task::JoinHandle<()>,
    debug_socket: PathBuf,
    pid_file: PathBuf,
    project_dir: PathBuf,
}

async fn start_daemon_with_owned_server() -> Harness {
    let temp = tempfile::tempdir().expect("tempdir");
    let home = temp.path().join("home");
    let project_dir = temp.path().join("project");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(&project_dir).unwrap();
    let pid_file = temp.path().join("owned.pid");
    let server_path = write_fake_owned_server(temp.path(), &pid_file);
    std::fs::write(
        home.join("mcp.json"),
        serde_json::json!({
            "mcpServers": {
                "owned": {
                    "command": server_path.display().to_string(),
                    "shared": false
                }
            }
        })
        .to_string(),
    )
    .unwrap();

    let env = vec![
        crate::storage::EnvVarGuard::set("JCODE_HOME", &home),
        crate::storage::EnvVarGuard::set("JCODE_RUNTIME_DIR", temp.path()),
        crate::storage::EnvVarGuard::set("JCODE_DEBUG_CONTROL", "1"),
    ];
    crate::config::invalidate_config_cache();

    let main_socket = temp.path().join("jcode.sock");
    let debug_socket = temp.path().join("jcode-debug.sock");
    let provider: Arc<dyn Provider> = Arc::new(TestProvider);
    let server = Server::new_with_paths(provider, main_socket.clone(), debug_socket.clone());
    let runtime = ServerRuntime::from_server(&server);
    let accept_main =
        runtime.spawn_main_accept_loop(Listener::bind(&main_socket).expect("bind main"));
    let accept_debug = runtime.spawn_debug_accept_loop(
        Listener::bind(&debug_socket).expect("bind debug"),
        Instant::now(),
    );

    Harness {
        _temp: temp,
        _env: env,
        _runtime: runtime,
        _accept_main: accept_main,
        _accept_debug: accept_debug,
        debug_socket,
        pid_file,
        project_dir,
    }
}

impl Harness {
    /// Create a headless session and wait for its owned child to start.
    async fn create_headless_session(&self) -> (Client, String, u32) {
        let mut debug = Client::connect_debug_with_path(self.debug_socket.clone())
            .await
            .expect("debug connect");
        let id = debug
            .debug_command(
                &format!("create_session:{}", self.project_dir.display()),
                None,
            )
            .await
            .expect("send create_session");
        let session_id = loop {
            let event = debug.read_event().await.expect("debug event");
            match event {
                ServerEvent::DebugResponse {
                    id: got,
                    ok,
                    output,
                } if got == id => {
                    assert!(ok, "create_session failed: {output}");
                    let value: serde_json::Value = serde_json::from_str(&output)
                        .unwrap_or_else(|_| serde_json::json!({ "session_id": output.trim() }));
                    break value["session_id"]
                        .as_str()
                        .map(str::to_string)
                        .unwrap_or_else(|| output.trim().to_string());
                }
                ServerEvent::Error {
                    id: got, message, ..
                } if got == id => {
                    panic!("create_session error: {message}");
                }
                _ => {}
            }
        };
        // The server truncates the pid file before writing it, so wait for a
        // parseable pid rather than for the file to exist.
        let pid_file = self.pid_file.clone();
        let read_pid = move || {
            std::fs::read_to_string(&pid_file)
                .ok()
                .and_then(|text| text.trim().parse::<u32>().ok())
        };
        let mut pid = None;
        assert!(
            wait_until(Duration::from_secs(10), || {
                pid = read_pid();
                pid.is_some()
            })
            .await,
            "owned server must be spawned for the new session"
        );
        let pid = pid.expect("pid file");
        assert!(
            pid_is_live(pid),
            "owned child should be alive after connect"
        );
        (debug, session_id, pid)
    }

    /// End the session the way the daemon does for a headless worker that is
    /// stopped or cleaned up: remove it from the sessions map and drop the
    /// agent (`destroy_session` shares that path with `comm stop`).
    async fn destroy_session(&self, debug: &mut Client, session_id: &str) {
        let id = debug
            .debug_command(&format!("destroy_session:{session_id}"), None)
            .await
            .expect("send destroy_session");
        loop {
            match debug.read_event().await.expect("destroy event") {
                ServerEvent::DebugResponse {
                    id: got,
                    ok,
                    output,
                } if got == id => {
                    assert!(ok, "destroy_session failed: {output}");
                    break;
                }
                ServerEvent::Error {
                    id: got, message, ..
                } if got == id => {
                    panic!("destroy_session error: {message}");
                }
                _ => {}
            }
        }
    }
}

#[cfg(unix)]
#[tokio::test]
async fn owned_child_exits_when_headless_session_is_stopped() {
    // Single-threaded on purpose: the daemon's tasks take test-env read
    // leases, which are admitted only on the thread holding the write lease.
    let _guard = crate::storage::lock_test_env();
    let harness = start_daemon_with_owned_server().await;
    let (mut debug, session_id, pid) = harness.create_headless_session().await;

    harness.destroy_session(&mut debug, &session_id).await;

    assert!(
        wait_until(Duration::from_secs(5), || !pid_is_live(pid)).await,
        "owned MCP child {pid} must exit after its session is stopped"
    );
}
