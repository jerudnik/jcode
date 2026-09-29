//! One Claude Code version for all subscription API identity fields.

use std::process::Stdio;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::sync::OnceCell;

/// Last reviewed compatible identity when Claude Code cannot be queried.
pub const FALLBACK_VERSION: &str = "2.1.257";
const DETECTION_TIMEOUT: Duration = Duration::from_secs(2);
const MAX_OUTPUT_BYTES: u64 = 1024;

pub struct ClaudeCliIdentity {
    pub version: String,
    pub user_agent: String,
    pub billing_header: String,
}

impl ClaudeCliIdentity {
    fn new(version: &str) -> Self {
        Self {
            version: version.to_owned(),
            user_agent: format!("claude-cli/{version} (external, sdk-cli)"),
            billing_header: format!("cc_version={version}; cc_entrypoint=sdk-cli; cch=33f85;"),
        }
    }
}

/// Query the executable on PATH once, caching both success and fallback.
/// Restart Jcode after installing or upgrading Claude Code to refresh it.
pub async fn claude_cli_identity() -> &'static ClaudeCliIdentity {
    static IDENTITY: OnceCell<ClaudeCliIdentity> = OnceCell::const_new();
    IDENTITY
        .get_or_init(|| detect_identity(Command::new("claude"), DETECTION_TIMEOUT))
        .await
}

async fn detect_identity(command: Command, timeout: Duration) -> ClaudeCliIdentity {
    if let Some(version) = detect_version(command, timeout).await {
        jcode_logging::info(&format!("Claude Code OAuth identity: detected {version}"));
        ClaudeCliIdentity::new(&version)
    } else {
        jcode_logging::warn(&format!(
            "Cannot query Claude Code on PATH; using OAuth identity {FALLBACK_VERSION}. Install or update Claude Code and restart Jcode if Anthropic rejects this version."
        ));
        ClaudeCliIdentity::new(FALLBACK_VERSION)
    }
}

async fn detect_version(mut command: Command, timeout: Duration) -> Option<String> {
    let mut child = command
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .ok()?;
    let result = tokio::time::timeout(timeout, async {
        let mut output = Vec::new();
        child
            .stdout
            .take()?
            .take(MAX_OUTPUT_BYTES + 1)
            .read_to_end(&mut output)
            .await
            .ok()?;
        if output.len() > MAX_OUTPUT_BYTES as usize {
            return None;
        }
        if !child.wait().await.ok()?.success() {
            return None;
        }
        parse_version(&output).map(str::to_owned)
    })
    .await
    .ok()
    .flatten();
    // Reap failed/timed-out commands as well. Dropping a cancelled detection
    // also kills the owned child via kill_on_drop.
    if result.is_none() {
        let _ = child.kill().await;
    }
    result
}

fn parse_version(output: &[u8]) -> Option<&str> {
    let text = std::str::from_utf8(output).ok()?.trim();
    let version = text.strip_suffix(" (Claude Code)")?;
    let parts: Vec<_> = version.split('.').collect();
    (parts.len() == 3
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit())))
    .then_some(version)
}

#[cfg(test)]
#[path = "claude_cli_identity_tests.rs"]
mod tests;
