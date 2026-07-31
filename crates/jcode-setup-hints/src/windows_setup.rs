use super::{SetupHintsState, StartupHints, read_choice};
use anyhow::Result;
use jcode_storage as storage;
use std::io::{self, Write};

fn detect_terminal() -> &'static str {
    if std::env::var("WT_SESSION").is_ok() {
        "windows-terminal"
    } else if std::env::var("WEZTERM_EXECUTABLE").is_ok() || std::env::var("WEZTERM_PANE").is_ok() {
        "wezterm"
    } else if std::env::var("ALACRITTY_WINDOW_ID").is_ok() {
        "alacritty"
    } else {
        "unknown"
    }
}

fn is_alacritty_installed() -> bool {
    std::process::Command::new("where")
        .arg("alacritty")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn is_winget_available() -> bool {
    std::process::Command::new("where")
        .arg("winget")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub(super) fn find_alacritty_path() -> Option<String> {
    let candidates = [
        r"C:\Program Files\Alacritty\alacritty.exe",
        r"C:\Program Files (x86)\Alacritty\alacritty.exe",
    ];
    for c in &candidates {
        if std::path::Path::new(c).exists() {
            return Some(c.to_string());
        }
    }
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        let p = format!(r"{}\Microsoft\WinGet\Links\alacritty.exe", local);
        if std::path::Path::new(&p).exists() {
            return Some(p);
        }
    }
    let output = std::process::Command::new("where")
        .arg("alacritty")
        .output()
        .ok()?;
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if let Some(line) = stdout.lines().next() {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

fn install_alacritty() -> Result<()> {
    eprintln!("  Installing Alacritty via winget...");
    eprintln!("  (Windows may ask for permission to install)\n");

    let status = std::process::Command::new("winget")
        .args([
            "install",
            "-e",
            "--id",
            "Alacritty.Alacritty",
            "--accept-source-agreements",
        ])
        .status()?;

    if status.success() {
        Ok(())
    } else {
        anyhow::bail!("winget install failed (exit code: {:?})", status.code())
    }
}

fn nudge_alacritty(state: &mut SetupHintsState) -> bool {
    let terminal = detect_terminal();

    let current_terminal = match terminal {
        "windows-terminal" => "Windows Terminal",
        "wezterm" => "WezTerm",
        _ => "your current terminal",
    };

    eprintln!("\x1b[36m┌─────────────────────────────────────────────────────────────┐\x1b[0m");
    eprintln!(
        "\x1b[36m│\x1b[0m \x1b[1m💡 Alacritty: the fastest terminal for jcode\x1b[0m               \x1b[36m│\x1b[0m"
    );
    eprintln!(
        "\x1b[36m│\x1b[0m                                                             \x1b[36m│\x1b[0m"
    );
    eprintln!(
        "\x1b[36m│\x1b[0m    {:<55} \x1b[36m│\x1b[0m",
        format!("You're using {}.", current_terminal)
    );
    eprintln!(
        "\x1b[36m│\x1b[0m    Alacritty is GPU-accelerated with the lowest latency.    \x1b[36m│\x1b[0m"
    );
    eprintln!(
        "\x1b[36m│\x1b[0m                                                             \x1b[36m│\x1b[0m"
    );
    eprintln!(
        "\x1b[36m│\x1b[0m    \x1b[32m[y]\x1b[0m Install   \x1b[90m[n]\x1b[0m Not now   \x1b[90m[d]\x1b[0m Don't ask again       \x1b[36m│\x1b[0m"
    );
    eprintln!("\x1b[36m└─────────────────────────────────────────────────────────────┘\x1b[0m");
    eprint!("\x1b[36m  >\x1b[0m ");
    let _ = io::stderr().flush();

    let choice = read_choice();

    match choice.as_str() {
        "y" | "yes" => {
            eprint!("\n");
            if !is_winget_available() {
                eprintln!("  \x1b[33m⚠\x1b[0m  winget not found. Install Alacritty manually:");
                eprintln!("     https://alacritty.org/");
                eprintln!();
                eprintln!("     Or install winget first: https://aka.ms/getwinget");
                eprintln!();
                return false;
            }

            match install_alacritty() {
                Ok(()) => {
                    state.alacritty_configured = true;
                    let _ = state.save();
                    eprintln!("  \x1b[32m✓\x1b[0m Alacritty installed!");
                    eprintln!();
                    true
                }
                Err(e) => {
                    eprintln!("  \x1b[31m✗\x1b[0m Failed to install Alacritty: {}", e);
                    eprintln!("    Install manually: https://alacritty.org/");
                    eprintln!();
                    false
                }
            }
        }
        "d" | "dont" => {
            state.alacritty_dismissed = true;
            let _ = state.save();
            false
        }
        _ => false,
    }
}

pub(super) fn maybe_show_windows_setup_hints(
    state: &mut SetupHintsState,
    startup_hints: Option<StartupHints>,
) -> Option<StartupHints> {
    if state.launch_count % 3 != 0 {
        return startup_hints;
    }

    let terminal = detect_terminal();
    let already_using_alacritty = terminal == "alacritty";

    if already_using_alacritty {
        state.alacritty_configured = true;
        state.alacritty_dismissed = true;
        let _ = state.save();
    }

    let wants_alacritty_nudge =
        !state.alacritty_configured && !state.alacritty_dismissed && !already_using_alacritty;

    // Stop pestering the user once we have shown the nudge prompt enough times,
    // even if they never explicitly chose "Don't ask again".
    if wants_alacritty_nudge && !state.nudge_budget_remaining() {
        return startup_hints;
    }

    if wants_alacritty_nudge {
        state.record_nudge_shown();
        nudge_alacritty(state);
    }

    startup_hints
}

pub(super) fn create_windows_desktop_shortcut(state: &mut SetupHintsState) -> Result<()> {
    let exe = std::env::current_exe()?;
    let exe_path = exe.to_string_lossy();

    let (target, args) = if is_alacritty_installed() {
        let alacritty = find_alacritty_path().unwrap_or_else(|| "alacritty".to_string());
        (alacritty, format!("-e \"{}\"", exe_path))
    } else {
        (exe_path.to_string(), String::new())
    };

    let desktop_dir = std::env::var("USERPROFILE").unwrap_or_else(|_| "C:\\Users\\Default".into());
    let shortcut_path = format!("{}\\Desktop\\jcode.lnk", desktop_dir);

    let ps_script = format!(
        r#"
$shell = New-Object -ComObject WScript.Shell
$shortcut = $shell.CreateShortcut("{shortcut_path}")
$shortcut.TargetPath = "{target}"
$shortcut.Arguments = '{args}'
$shortcut.Description = "jcode - AI coding agent"
$shortcut.Save()
Write-Output "OK"
"#,
        shortcut_path = shortcut_path,
        target = target,
        args = args,
    );

    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &ps_script])
        .output()?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.contains("OK") {
            state.desktop_shortcut_created = true;
            let _ = state.save();
            jcode_logging::info(&format!("Created desktop shortcut: {}", shortcut_path));
        }
    }

    Ok(())
}
