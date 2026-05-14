use super::{Tool, ToolContext, ToolOutput};
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value, json};
use std::path::Path;
use std::process::{Command, Stdio};

pub struct WmTool;

impl WmTool {
    pub fn new() -> Self {
        Self
    }
}

#[derive(Debug, Deserialize)]
struct WmInput {
    #[serde(default)]
    action: Option<String>,
    #[serde(default)]
    backend: Option<String>,
    #[serde(default)]
    command: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    cwd: Option<String>,
    #[serde(default)]
    direction: Option<String>,
    #[serde(default)]
    floating: Option<bool>,
    #[serde(default)]
    close_on_exit: Option<bool>,
}

#[async_trait]
impl Tool for WmTool {
    fn name(&self) -> &str {
        "wm"
    }

    fn description(&self) -> &str {
        "Integrated window management for opening helper panes/windows. Prefers zellij panes when inside zellij and can fall back to Ghostty windows on macOS."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "intent": super::intent_schema_property(),
                "action": { "type": "string", "enum": ["status", "open"], "description": "Action. Use status to inspect available backends, open to launch a helper pane/window." },
                "backend": { "type": "string", "enum": ["auto", "zellij", "ghostty"], "description": "Window backend. Defaults to auto." },
                "command": { "type": "string", "description": "Shell command to run for action=open." },
                "title": { "type": "string", "description": "Pane/window title. Defaults to jcode." },
                "cwd": { "type": "string", "description": "Working directory. Defaults to the current session working directory." },
                "direction": { "type": "string", "enum": ["right", "down"], "description": "For zellij, pane split direction." },
                "floating": { "type": "boolean", "description": "For zellij, open as a floating pane." },
                "close_on_exit": { "type": "boolean", "description": "For zellij, close pane automatically when the command exits." }
            }
        })
    }

    async fn execute(&self, input: Value, ctx: ToolContext) -> Result<ToolOutput> {
        let params: WmInput = serde_json::from_value(input)?;
        let action = params.action.as_deref().unwrap_or("status");
        match action {
            "status" => Ok(status_output()),
            "open" => open_window(params, &ctx),
            other => anyhow::bail!("Unknown wm action: {other}. Valid actions: status, open"),
        }
    }
}

fn status_output() -> ToolOutput {
    let zellij = zellij_available();
    let ghostty = ghostty_available();
    let preferred = preferred_backend("auto");
    let preferred_label = preferred.clone().unwrap_or_else(|| "none".to_string());
    let text = format!(
        "Window management backends:\n- zellij: {}{}\n- ghostty: {}\npreferred: {}",
        if zellij { "available" } else { "unavailable" },
        if std::env::var("ZELLIJ").is_ok() {
            " (active session)"
        } else if zellij {
            " (not active, pane opens require running inside zellij)"
        } else {
            ""
        },
        if ghostty { "available" } else { "unavailable" },
        preferred_label
    );
    ToolOutput::new(text)
        .with_title("wm status")
        .with_metadata(json!({
            "zellij": zellij,
            "zellij_session": std::env::var("ZELLIJ").is_ok(),
            "ghostty": ghostty,
            "preferred": preferred,
        }))
}

fn open_window(params: WmInput, ctx: &ToolContext) -> Result<ToolOutput> {
    let command = params
        .command
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .context("wm action=open requires a non-empty command")?;
    let backend = preferred_backend(params.backend.as_deref().unwrap_or("auto"))
        .context("No window backend is available. Start jcode inside zellij or install Ghostty.")?;
    let cwd = params
        .cwd
        .as_deref()
        .map(|cwd| ctx.resolve_path(Path::new(cwd)))
        .or_else(|| ctx.working_dir.clone())
        .unwrap_or_else(|| {
            std::env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf())
        });
    let title = params.title.as_deref().unwrap_or("jcode");

    match backend.as_str() {
        "zellij" => open_zellij(command, title, &params)?,
        "ghostty" => open_ghostty(command, title, &cwd)?,
        _ => unreachable!(),
    }

    Ok(ToolOutput::new(format!(
        "Opened {backend} {} for `{command}`.",
        if backend == "zellij" {
            "pane"
        } else {
            "window"
        }
    ))
    .with_title("wm open")
    .with_metadata(json!({ "backend": backend, "command": command, "title": title, "cwd": cwd })))
}

fn preferred_backend(requested: &str) -> Option<String> {
    match requested {
        "zellij" if zellij_available() => Some("zellij".to_string()),
        "ghostty" if ghostty_available() => Some("ghostty".to_string()),
        "auto" | "" => {
            if zellij_available() && std::env::var("ZELLIJ").is_ok() {
                Some("zellij".to_string())
            } else if ghostty_available() {
                Some("ghostty".to_string())
            } else if zellij_available() {
                Some("zellij".to_string())
            } else {
                None
            }
        }
        _ => None,
    }
}

fn command_available(program: &str) -> bool {
    Command::new("/bin/sh")
        .args([
            "-lc",
            &format!("command -v {} >/dev/null 2>&1", shell_quote(program)),
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn zellij_available() -> bool {
    command_available(&std::env::var("JCODE_ZELLIJ_BIN").unwrap_or_else(|_| "zellij".to_string()))
}

fn ghostty_available() -> bool {
    command_available("ghostty")
        || cfg!(target_os = "macos") && Path::new("/Applications/Ghostty.app").exists()
}

fn open_zellij(command: &str, title: &str, params: &WmInput) -> Result<()> {
    let zellij = std::env::var("JCODE_ZELLIJ_BIN").unwrap_or_else(|_| "zellij".to_string());
    let mut cmd = Command::new(zellij);
    cmd.args(["action", "new-pane", "--name", title]);
    if let Some(direction) = params
        .direction
        .as_deref()
        .map(str::trim)
        .filter(|direction| matches!(*direction, "right" | "down"))
    {
        cmd.args(["--direction", direction]);
    }
    if params.floating.unwrap_or(false) {
        cmd.arg("--floating");
    }
    if params.close_on_exit.unwrap_or(false) {
        cmd.arg("--close-on-exit");
    }
    cmd.args(["--", "bash", "-lc", command]);
    run_status(cmd, "zellij")
}

fn open_ghostty(command: &str, title: &str, cwd: &Path) -> Result<()> {
    #[cfg(target_os = "macos")]
    let mut cmd = {
        let mut cmd = Command::new("open");
        let wrapped = format!(
            "printf '\\033]2;%s\\007' {}; cd {}; exec /bin/bash -lc {}",
            shell_quote(title),
            shell_quote(&cwd.to_string_lossy()),
            shell_quote(command)
        );
        cmd.current_dir(cwd)
            .args(["-na", "Ghostty", "--args", "-e", "/bin/bash", "-lc"])
            .arg(wrapped);
        cmd
    };

    #[cfg(not(target_os = "macos"))]
    let mut cmd = {
        let mut cmd = Command::new("ghostty");
        cmd.current_dir(cwd)
            .args(["-e", "/bin/bash", "-lc"])
            .arg(format!(
                "printf '\\033]2;%s\\007' {}; exec /bin/bash -lc {}",
                shell_quote(title),
                shell_quote(command)
            ));
        cmd
    };

    run_spawn(&mut cmd, "ghostty")
}

fn run_status(mut cmd: Command, backend: &str) -> Result<()> {
    let output = cmd
        .output()
        .with_context(|| format!("Failed to run {backend}"))?;
    if output.status.success() {
        Ok(())
    } else {
        anyhow::bail!(
            "{backend} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )
    }
}

fn run_spawn(cmd: &mut Command, backend: &str) -> Result<()> {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    cmd.spawn()
        .with_context(|| format!("Failed to spawn {backend}"))?;
    Ok(())
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_exposes_open_action() {
        let schema = WmTool::new().parameters_schema();
        let actions = schema["properties"]["action"]["enum"].as_array().unwrap();
        assert!(actions.iter().any(|value| value == "open"));
        assert!(schema["properties"].get("backend").is_some());
    }

    #[test]
    fn shell_quote_handles_single_quotes() {
        assert_eq!(shell_quote("a'b"), "'a'\"'\"'b'");
    }
}
