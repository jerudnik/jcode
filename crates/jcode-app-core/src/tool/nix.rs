use super::{Tool, ToolContext, ToolOutput};
use crate::util::truncate_str;
use anyhow::Result;
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value, json};
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command as TokioCommand;

const MAX_OUTPUT_LEN: usize = 30000;
const DEFAULT_TIMEOUT_MS: u64 = 300_000; // 5 min: first fetch of a package can be slow
const MAX_TIMEOUT_MS: u64 = 1_800_000; // 30 min hard ceiling

const NIX_TOOL_DESCRIPTION: &str = "Run an ephemeral CLI tool through Nix without installing it \
globally. Provide one or more nixpkgs package names (e.g. [\"ripgrep\", \"jq\"]) and the argv to \
run. Mode \"shell\" puts the packages' bin/ on PATH and runs your `command` argv \
(`nix shell nixpkgs#pkg... --command <command>`); mode \"run\" executes a single flake app/package \
(`nix run <ref> -- <args>`). Use this for one-off utilities, data wrangling, format conversion, \
network fetches (curl/httpie), or any tool not already on PATH. The tool is fetched into the Nix \
store on first use and cached after. Non-interactive only: do not launch TUIs, pagers, or programs \
that require a terminal.";

#[derive(Deserialize)]
struct NixInput {
    /// "shell" (default): packages on PATH, run `command` argv.
    /// "run": execute a single flake app/package reference with `args`.
    #[serde(default)]
    mode: Option<String>,
    /// Package names or flake references. For "shell" each becomes
    /// `nixpkgs#<name>` unless it already contains a `#` (a full flake ref).
    /// For "run" exactly one entry is used as the flake reference.
    #[serde(default)]
    packages: Vec<String>,
    /// argv for "shell" mode: the program and its arguments to run with the
    /// packages on PATH.
    #[serde(default)]
    command: Vec<String>,
    /// argv passed after `--` for "run" mode.
    #[serde(default)]
    args: Vec<String>,
    /// Timeout in milliseconds. Defaults to 300000 (5 min), capped at 1800000.
    #[serde(default)]
    timeout: Option<u64>,
}

pub struct NixTool;

impl NixTool {
    pub fn new() -> Self {
        Self
    }

    /// Normalize a package token into a flake installable. Bare names map to
    /// `nixpkgs#<name>`; anything already containing `#` is treated as a full
    /// flake reference and passed through unchanged.
    fn to_installable(pkg: &str) -> String {
        let pkg = pkg.trim();
        if pkg.contains('#') || pkg.contains(':') {
            pkg.to_string()
        } else {
            format!("nixpkgs#{pkg}")
        }
    }

    fn build_argv(input: &NixInput) -> Result<Vec<String>> {
        let mode = input.mode.as_deref().unwrap_or("shell").trim();
        match mode {
            "shell" | "" => {
                if input.packages.is_empty() {
                    anyhow::bail!("nix shell requires at least one package in `packages`");
                }
                if input.command.is_empty() {
                    anyhow::bail!(
                        "nix shell requires a `command` argv (the program and args to run)"
                    );
                }
                let mut argv = vec!["shell".to_string()];
                for pkg in &input.packages {
                    argv.push(Self::to_installable(pkg));
                }
                argv.push("--command".to_string());
                argv.extend(input.command.iter().cloned());
                Ok(argv)
            }
            "run" => {
                if input.packages.len() != 1 {
                    anyhow::bail!(
                        "nix run requires exactly one flake reference in `packages` (got {})",
                        input.packages.len()
                    );
                }
                let mut argv = vec!["run".to_string(), Self::to_installable(&input.packages[0])];
                if !input.args.is_empty() {
                    argv.push("--".to_string());
                    argv.extend(input.args.iter().cloned());
                }
                Ok(argv)
            }
            other => anyhow::bail!("unknown nix mode '{other}' (expected 'shell' or 'run')"),
        }
    }
}

impl Default for NixTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for NixTool {
    fn name(&self) -> &str {
        "nix"
    }

    fn description(&self) -> &str {
        NIX_TOOL_DESCRIPTION
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["packages"],
            "properties": {
                "intent": super::intent_schema_property(),
                "mode": {
                    "type": "string",
                    "enum": ["shell", "run"],
                    "description": "shell: packages on PATH, run `command` argv. run: execute one flake app/package with `args`. Defaults to shell."
                },
                "packages": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "nixpkgs package names (e.g. \"ripgrep\") or full flake refs (e.g. \"nixpkgs#jq\", \"github:owner/repo\"). For run mode, exactly one ref."
                },
                "command": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "shell mode argv: the program and its arguments, e.g. [\"rg\", \"-n\", \"TODO\", \"src\"]."
                },
                "args": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "run mode argv passed after `--`."
                },
                "timeout": {
                    "type": "integer",
                    "description": "Timeout in milliseconds (default 300000, max 1800000). First fetch of a package can be slow."
                }
            }
        })
    }

    async fn execute(&self, input: Value, ctx: ToolContext) -> Result<ToolOutput> {
        let params: NixInput = serde_json::from_value(input)?;
        let timeout_ms = params
            .timeout
            .unwrap_or(DEFAULT_TIMEOUT_MS)
            .min(MAX_TIMEOUT_MS);
        let argv = Self::build_argv(&params)?;

        let mut cmd = TokioCommand::new("nix");
        // Ensure flakes are available regardless of the user's nix.conf, matching
        // the repo convention for invoking nix non-interactively.
        cmd.arg("--extra-experimental-features")
            .arg("nix-command flakes")
            .args(&argv);
        cmd.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(ref dir) = ctx.working_dir {
            cmd.current_dir(dir);
        }

        let display = format!("nix {}", argv.join(" "));
        let child = cmd.spawn().map_err(|e| {
            anyhow::anyhow!(
                "failed to spawn `nix` (is Nix installed and on PATH?): {e}. Command: {display}"
            )
        })?;

        let output = match tokio::time::timeout(
            Duration::from_millis(timeout_ms),
            child.wait_with_output(),
        )
        .await
        {
            Ok(result) => result?,
            Err(_) => {
                return Ok(ToolOutput::new(format!(
                    "`{display}` timed out after {timeout_ms}ms. First-time package fetches can \
                         be slow; retry with a larger `timeout`, or pre-warm the package."
                )));
            }
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let mut body = String::new();
        if !stdout.trim().is_empty() {
            body.push_str(&stdout);
        }
        if !stderr.trim().is_empty() {
            if !body.is_empty() {
                body.push('\n');
            }
            body.push_str("[stderr]\n");
            body.push_str(&stderr);
        }
        if body.trim().is_empty() {
            body.push_str("(no output)");
        }
        let body = truncate_str(&body, MAX_OUTPUT_LEN).to_string();

        let status = output.status;
        let header = if status.success() {
            format!("$ {display}\n")
        } else {
            let code = status
                .code()
                .map(|c| c.to_string())
                .unwrap_or_else(|| "signal".to_string());
            format!("$ {display}\n(exit {code})\n")
        };

        Ok(ToolOutput::new(format!("{header}{body}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(value: Value) -> NixInput {
        serde_json::from_value(value).unwrap()
    }

    #[test]
    fn shell_mode_builds_command_with_nixpkgs_prefix() {
        let argv = NixTool::build_argv(&input(json!({
            "mode": "shell",
            "packages": ["ripgrep", "jq"],
            "command": ["rg", "-n", "TODO"]
        })))
        .unwrap();
        assert_eq!(
            argv,
            vec![
                "shell",
                "nixpkgs#ripgrep",
                "nixpkgs#jq",
                "--command",
                "rg",
                "-n",
                "TODO"
            ]
        );
    }

    #[test]
    fn shell_mode_defaults_when_mode_omitted() {
        let argv = NixTool::build_argv(&input(json!({
            "packages": ["curl"],
            "command": ["curl", "-s", "https://example.com"]
        })))
        .unwrap();
        assert_eq!(argv[0], "shell");
        assert_eq!(argv[1], "nixpkgs#curl");
    }

    #[test]
    fn full_flake_refs_pass_through() {
        let argv = NixTool::build_argv(&input(json!({
            "packages": ["nixpkgs#jq", "github:owner/repo"],
            "command": ["jq", "."]
        })))
        .unwrap();
        assert_eq!(argv[1], "nixpkgs#jq");
        assert_eq!(argv[2], "github:owner/repo");
    }

    #[test]
    fn run_mode_builds_run_with_double_dash() {
        let argv = NixTool::build_argv(&input(json!({
            "mode": "run",
            "packages": ["ddgr"],
            "args": ["--json", "rust"]
        })))
        .unwrap();
        assert_eq!(argv, vec!["run", "nixpkgs#ddgr", "--", "--json", "rust"]);
    }

    #[test]
    fn run_mode_without_args_omits_double_dash() {
        let argv = NixTool::build_argv(&input(json!({
            "mode": "run",
            "packages": ["hello"]
        })))
        .unwrap();
        assert_eq!(argv, vec!["run", "nixpkgs#hello"]);
    }

    #[test]
    fn shell_mode_requires_packages_and_command() {
        assert!(NixTool::build_argv(&input(json!({"command": ["ls"]}))).is_err());
        assert!(NixTool::build_argv(&input(json!({"packages": ["ripgrep"]}))).is_err());
    }

    #[test]
    fn run_mode_requires_single_ref() {
        assert!(
            NixTool::build_argv(&input(json!({"mode": "run", "packages": ["a", "b"]}))).is_err()
        );
    }

    #[test]
    fn unknown_mode_errors() {
        assert!(NixTool::build_argv(&input(json!({"mode": "bogus", "packages": ["x"]}))).is_err());
    }
}
