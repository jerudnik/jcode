use super::*;

pub fn selfdev_status_output() -> Result<ToolOutput> {
    let manifest = build::BuildManifest::load()?;

    let mut status = String::new();

    status.push_str("## Current Version\n\n");
    status.push_str(&format!(
        "**Running:** jcode {}\n",
        jcode_build_meta::VERSION
    ));

    if let Some(repo_dir) = build::get_repo_dir() {
        let output = std::process::Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(&repo_dir)
            .output()
            .ok();

        if let Some(output) = output {
            let changes: Vec<&str> = std::str::from_utf8(&output.stdout)
                .unwrap_or("")
                .lines()
                .collect();
            if changes.is_empty() {
                status.push_str("**Working tree:** clean\n");
            } else {
                status.push_str(&format!(
                    "**Working tree:** {} uncommitted change{}\n",
                    changes.len(),
                    if changes.len() == 1 { "" } else { "s" }
                ));
            }
        }
    }

    // F20c: one publish target replaced the current/stable/shared-server/canary
    // channel matrix, so there is a single published binary to report.
    status.push_str("\n## Published Build\n\n");
    match build::current_fixed_binary_path() {
        Ok(path) if path.exists() => {
            status.push_str(&format!("**Path:** {}\n", path.display()));
            match build::read_dev_binary_source_metadata(&path) {
                Some(meta) => {
                    status.push_str(&format!("**Version:** {}\n", meta.version_label));
                    status.push_str(&format!(
                        "**Source fingerprint:** `{}`\n",
                        meta.source_fingerprint
                    ));
                }
                None => status.push_str("**Version:** unknown (no source metadata)\n"),
            }
        }
        _ => status.push_str("**Path:** none published\n"),
    }

    status.push_str("\n## Debug Socket\n\n");
    status.push_str(&format!(
        "**Path:** {}\n",
        server::debug_socket_path().display()
    ));

    if let Some(reload_state) = server::ReloadState::load() {
        status.push_str("\n## Reload State\n\n");
        status.push_str(&format!(
            "**Phase:** {:?}\n**Request:** {}\n**Hash:** {}\n**PID:** {}\n**Timestamp:** {}\n",
            reload_state.phase,
            reload_state.request_id,
            reload_state.hash,
            reload_state.pid,
            reload_state.timestamp,
        ));
        if let Some(detail) = reload_state.detail {
            status.push_str(&format!("**Detail:** {}\n", detail));
        }
    }

    let pending_requests = BuildRequest::pending_requests()?;
    if !pending_requests.is_empty() {
        status.push_str("\n## Build Queue\n\n");
        for (index, request) in pending_requests.iter().enumerate() {
            let watchers = BuildRequest::attached_watchers(&request.request_id)?;
            let state = match request.state {
                BuildRequestState::Queued => "queued",
                BuildRequestState::Building => "building",
                BuildRequestState::Attached => "attached",
                BuildRequestState::Completed => "completed",
                BuildRequestState::Superseded => "superseded",
                BuildRequestState::Failed => "failed",
                BuildRequestState::Cancelled => "cancelled",
            };
            status.push_str(&format!(
                "{}. **{}** — {}\n   Reason: {}\n   Requested: {}\n",
                index + 1,
                state,
                request.display_owner(),
                request.reason,
                request.requested_at,
            ));
            if let Some(version) = request.version.as_deref() {
                status.push_str(&format!("   Target version: `{}`\n", version));
            }
            if let Some(source) = request.requested_source.as_ref() {
                status.push_str(&format!(
                    "   Source fingerprint: `{}` (dirty={}, changed_paths={})\n",
                    source.fingerprint, source.dirty, source.changed_paths
                ));
            }
            if let Some(progress) = request.last_progress.as_deref() {
                status.push_str(&format!("   Progress: {}\n", progress));
            }
            if let Some(task_id) = request.background_task_id.as_deref() {
                status.push_str(&format!("   Task: `{}`\n", task_id));
            }
            if let Some(started_at) = request.started_at.as_deref() {
                status.push_str(&format!("   Started: {}\n", started_at));
            }
            if let Some(published) = request.published_version.as_deref() {
                status.push_str(&format!("   Published version: `{}`\n", published));
            }
            status.push_str(&format!("   Validated: {}\n", request.validated));
            if !watchers.is_empty() {
                let watcher_names = watchers
                    .iter()
                    .map(BuildRequest::display_owner)
                    .collect::<Vec<_>>()
                    .join(", ");
                status.push_str(&format!(
                    "   Attached watchers: {} ({})\n",
                    watchers.len(),
                    watcher_names
                ));
            }
        }
    }

    if !manifest.history.is_empty() {
        status.push_str("\n## Recent Builds\n\n");
        for (i, info) in manifest.history.iter().take(5).enumerate() {
            let dirty_marker = if info.dirty { " (dirty)" } else { "" };
            let msg = info
                .commit_message
                .as_deref()
                .unwrap_or("No commit message");
            status.push_str(&format!(
                "{}. `{}`{} - {}\n   Built: {}\n",
                i + 1,
                info.hash,
                dirty_marker,
                msg,
                info.built_at.format("%Y-%m-%d %H:%M:%S UTC")
            ));
        }
    }

    Ok(ToolOutput::new(status))
}

impl SelfDevTool {
    pub(super) async fn do_status(&self) -> Result<ToolOutput> {
        selfdev_status_output()
    }

    pub(super) async fn do_socket_info(&self) -> Result<ToolOutput> {
        let debug_socket = server::debug_socket_path();
        let main_socket = server::socket_path();

        let info = json!({
            "debug_socket": debug_socket.to_string_lossy(),
            "main_socket": main_socket.to_string_lossy(),
            "debug_enabled": crate::config::config().display.debug_socket ||
                             std::env::var("JCODE_DEBUG_CONTROL").is_ok() ||
                             crate::storage::jcode_dir().map(|d| d.join("debug_control").exists()).unwrap_or(false),
            "connect_example": format!(
                "echo '{{\"type\":\"debug_command\",\"id\":1,\"command\":\"help\"}}' | nc -U {}",
                debug_socket.display()
            ),
        });

        Ok(ToolOutput::new(format!(
            "## Debug Socket Info\n\n\
             **Debug socket:** {}\n\
             **Main socket:** {}\n\n\
             Use the `debug_socket` tool to send commands, or connect directly:\n\
             ```bash\n\
             echo '{{\"type\":\"debug_command\",\"id\":1,\"command\":\"help\"}}' | nc -U {}\n\
             ```\n\n\
             For programmatic access, use the `debug_socket` tool with the command parameter.",
            debug_socket.display(),
            main_socket.display(),
            debug_socket.display()
        ))
        .with_metadata(info))
    }

    pub(super) async fn do_socket_help(&self) -> Result<ToolOutput> {
        Ok(ToolOutput::new(
            r#"## Debug Socket Commands

Commands are namespaced with `server:`, `client:`, or `tester:` prefixes.
Unnamespaced commands default to `server:`.

### Server Commands (agent/tools)
| Command | Description |
|---------|-------------|
| `state` | Agent state (session, model) |
| `history` | Conversation history as JSON |
| `tools` | List available tools |
| `last_response` | Last assistant response |
| `message:<text>` | Send message, get LLM response |
| `tool:<name> <json>` | Execute tool directly |
| `sessions` | List all sessions |
| `create_session` | Create headless session |
| `help` | Full help text |

### Client Commands (TUI/visual debug)
| Command | Description |
|---------|-------------|
| `client:frame` | Get latest visual debug frame (JSON) |
| `client:frame-normalized` | Normalized frame for diffs |
| `client:screen` | Dump frames to file |
| `client:enable` | Enable visual debug capture |
| `client:disable` | Disable visual debug capture |
| `client:status` | Client debug status |
| `client:scroll-test[:<json>]` | Run offscreen scroll+diagram test |
| `client:scroll-suite[:<json>]` | Run scroll+diagram test suite |

### Tester Commands (spawn test instances)
| Command | Description |
|---------|-------------|
| `tester:spawn` | Spawn new tester instance |
| `tester:spawn {"cwd":"/path"}` | Spawn with options |
| `tester:list` | List active testers |
| `tester:<id>:frame` | Get frame from tester |
| `tester:<id>:state` | Get tester state |
| `tester:<id>:message:<text>` | Send message to tester |
| `tester:<id>:scroll-test[:<json>]` | Run offscreen scroll+diagram test |
| `tester:<id>:scroll-suite[:<json>]` | Run scroll+diagram test suite |
| `tester:<id>:stop` | Stop tester |

Use the `debug_socket` tool to execute these commands directly."#
                .to_string(),
        ))
    }
}
