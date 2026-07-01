//! `jcode doctor` -- binary identity diagnostics.
//!
//! Answers the daily self-dev confusion documented in
//! `docs/architecture/SELFDEV_NIX_DAEMON_DIVERGENCE.md`: *which* binary am I
//! running, from which checkout, and does it match the background daemon I'm
//! about to talk to? Everything here is read-only and derived from data that
//! already exists: `jcode_build_meta` consts (this client), the build-support
//! path helpers (origin/dirty), and the server registry (the running daemon).
//!
//! Output (human form):
//!
//! ```text
//! client:  /nix/store/...-jcode/bin/jcode  origin=nix       v0.14.6 (abc1234)
//! server:  ~/.jcode/run/server.sock  pid=12345  origin=selfdev  v0.14.6 (def5678) started 2026-06-27T08:00:00Z
//! verdict: RECONNECT (build mismatch: client abc1234 != server def5678)
//! fallback: nix run github:jerudnik/jcode -- doctor
//! ```

use anyhow::Result;
use serde::Serialize;
use std::path::Path;

/// Where a jcode binary came from, inferred from its on-disk path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
enum Origin {
    /// Immutable Nix store path (`/nix/store/...`).
    Nix,
    /// A self-dev / local build channel under `~/.jcode/builds/`.
    Selfdev,
    /// A release channel binary under `~/.jcode/builds/{stable,versions}/`.
    Release,
    /// A bare `cargo` build under a `target/` directory.
    Source,
    /// Anything else (e.g. a hand-copied binary on `PATH`).
    Unknown,
}

impl Origin {
    fn as_str(self) -> &'static str {
        match self {
            Origin::Nix => "nix",
            Origin::Selfdev => "selfdev",
            Origin::Release => "release",
            Origin::Source => "source",
            Origin::Unknown => "unknown",
        }
    }

    /// Classify by path. Order matters: Nix store wins, then the explicit build
    /// channels, then a cargo `target/` tree, else unknown.
    fn classify(path: &Path) -> Origin {
        let p = path.to_string_lossy();
        if p.contains("/nix/store/") {
            Origin::Nix
        } else if p.contains("/builds/stable/") || p.contains("/builds/versions/") {
            Origin::Release
        } else if p.contains("/builds/") {
            // current / canary / shared-server / any other local-build channel.
            Origin::Selfdev
        } else if p.contains("/target/debug/")
            || p.contains("/target/release/")
            || p.contains("/target/selfdev/")
        {
            Origin::Source
        } else {
            Origin::Unknown
        }
    }
}

#[derive(Debug, Serialize)]
struct ClientIdentity {
    path: String,
    origin: String,
    version: String,
    git_hash: String,
    dirty: Option<bool>,
    /// Source checkout this binary was built from. Only meaningful for a
    /// source/selfdev build sitting in a live repo; `None` for Nix/release
    /// binaries whose stamped path is an immutable build sandbox.
    build_source_dir: Option<String>,
}

#[derive(Debug, Serialize)]
struct ServerIdentity {
    running: bool,
    socket: String,
    pid: Option<u32>,
    name: Option<String>,
    version: Option<String>,
    git_hash: Option<String>,
    started_at: Option<String>,
}

/// The compatibility verdict between the client binary and the running daemon.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Verdict {
    /// No server is running; nothing to compare.
    NoServer,
    /// Client and server are the same committed build (git hash matches, clean).
    Same,
    /// Same commit, but the client tree is dirty: protocol-compatible, yet your
    /// uncommitted edits are not in the running daemon until you rebuild+reload.
    Compatible,
    /// Different commits: a reload/reconnect is warranted.
    Reconnect,
}

#[derive(Debug, Serialize)]
struct DoctorReport {
    client: ClientIdentity,
    server: ServerIdentity,
    verdict: String,
    verdict_detail: String,
    fallback: String,
}

fn client_identity() -> ClientIdentity {
    let path = crate::build::current_binary_path()
        .ok()
        .or_else(|| std::env::current_exe().ok())
        .unwrap_or_default();
    let origin = Origin::classify(&path);

    // The working tree is only meaningful for a source/selfdev build sitting in
    // a repo. Don't shell out to git for an immutable Nix or release binary.
    let dirty = match origin {
        Origin::Source | Origin::Selfdev | Origin::Unknown => crate::build::get_repo_dir()
            .and_then(|repo| crate::build::is_working_tree_dirty(&repo).ok()),
        Origin::Nix | Origin::Release => None,
    };

    ClientIdentity {
        path: path.display().to_string(),
        origin: origin.as_str().to_string(),
        version: jcode_build_meta::SEMVER.to_string(),
        git_hash: jcode_build_meta::GIT_HASH.to_string(),
        dirty,
        build_source_dir: build_source_dir(origin),
    }
}

/// The source checkout the binary was built from, surfaced only where it names a
/// live editable tree. For immutable Nix/release binaries the stamped path is a
/// build sandbox, so reporting it would mislead rather than answer "which
/// checkout produced this"; return `None` there. (G4 in the divergence doc.)
fn build_source_dir(origin: Origin) -> Option<String> {
    match origin {
        Origin::Source | Origin::Selfdev | Origin::Unknown => {
            let dir = jcode_build_meta::BUILD_SOURCE_DIR.trim();
            (!dir.is_empty() && dir != "unknown").then(|| dir.to_string())
        }
        Origin::Nix | Origin::Release => None,
    }
}

fn server_identity() -> ServerIdentity {
    let socket = crate::server::socket_path();
    let socket_str = socket.display().to_string();

    match crate::registry::find_server_by_socket_sync(&socket) {
        Some(info) => ServerIdentity {
            running: true,
            socket: socket_str,
            pid: Some(info.pid),
            name: Some(info.display_name()),
            version: Some(info.version.clone()),
            git_hash: Some(info.git_hash.clone()),
            started_at: Some(info.started_at.clone()),
        },
        None => ServerIdentity {
            running: false,
            socket: socket_str,
            pid: None,
            name: None,
            version: None,
            git_hash: None,
            started_at: None,
        },
    }
}

fn verdict(client: &ClientIdentity, server: &ServerIdentity) -> (Verdict, String) {
    if !server.running {
        return (
            Verdict::NoServer,
            "no background server running; this client would start one".to_string(),
        );
    }
    let s_hash = server.git_hash.as_deref().unwrap_or("");
    // Compare on the short hash to be robust to full-vs-short forms.
    let same_hash = !s_hash.is_empty()
        && (s_hash.starts_with(&client.git_hash) || client.git_hash.starts_with(s_hash));
    if same_hash {
        if client.dirty == Some(true) {
            (
                Verdict::Compatible,
                format!(
                    "same commit {} as the daemon, but your working tree is dirty; \
                     uncommitted changes are not in the running server until you rebuild + `jcode server reload`",
                    client.git_hash
                ),
            )
        } else {
            (
                Verdict::Same,
                "client and server are the same committed build".to_string(),
            )
        }
    } else {
        let s_ver = server
            .version
            .as_deref()
            .map(|v| v.split(" (").next().unwrap_or(v).trim())
            .unwrap_or("?");
        (
            Verdict::Reconnect,
            format!(
                "build mismatch: client {} ({}) != server {} ({}); run `jcode server reload`",
                client.version, client.git_hash, s_ver, s_hash
            ),
        )
    }
}

/// Best fallback command for getting a known-good binary.
fn fallback_command() -> String {
    "nix run github:jerudnik/jcode -- doctor   (or `jcode server reload`)".to_string()
}

pub fn run_doctor_command(emit_json: bool) -> Result<()> {
    let client = client_identity();
    let server = server_identity();
    let (v, detail) = verdict(&client, &server);

    let report = DoctorReport {
        client,
        server,
        verdict: format!("{v:?}").to_lowercase(),
        verdict_detail: detail,
        fallback: fallback_command(),
    };

    if emit_json {
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }

    let dirty_suffix = match report.client.dirty {
        Some(true) => " +dirty",
        _ => "",
    };
    println!(
        "client:  {}  origin={}  {} ({}){}",
        report.client.path,
        report.client.origin,
        report.client.version,
        report.client.git_hash,
        dirty_suffix,
    );
    if let Some(src) = report.client.build_source_dir.as_deref() {
        println!("  built-from: {src}");
    }
    if report.server.running {
        // The registry version string already embeds the hash (e.g.
        // "v0.31.37-dev (b83b6668)"); strip the parenthetical so the line does
        // not print the hash twice.
        let server_ver = report
            .server
            .version
            .as_deref()
            .map(|v| v.split(" (").next().unwrap_or(v).trim().to_string())
            .unwrap_or_else(|| "?".to_string());
        println!(
            "server:  {}  pid={}  {}  {} ({}) started {}",
            report.server.socket,
            report.server.pid.map(|p| p.to_string()).unwrap_or_default(),
            report.server.name.as_deref().unwrap_or("?"),
            server_ver,
            report.server.git_hash.as_deref().unwrap_or("?"),
            report.server.started_at.as_deref().unwrap_or("?"),
        );
    } else {
        println!("server:  {}  (not running)", report.server.socket);
    }
    println!(
        "verdict: {} -- {}",
        report.verdict.to_uppercase(),
        report.verdict_detail
    );
    println!("fallback: {}", report.fallback);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn classifies_nix_store_path() {
        let p = PathBuf::from("/nix/store/abc123-jcode-0.14.6/bin/jcode");
        assert_eq!(Origin::classify(&p), Origin::Nix);
    }

    #[test]
    fn classifies_selfdev_current_channel() {
        let p = PathBuf::from("/home/u/.jcode/builds/current/jcode");
        assert_eq!(Origin::classify(&p), Origin::Selfdev);
    }

    #[test]
    fn classifies_release_channels() {
        assert_eq!(
            Origin::classify(&PathBuf::from("/home/u/.jcode/builds/stable/jcode")),
            Origin::Release
        );
        assert_eq!(
            Origin::classify(&PathBuf::from(
                "/home/u/.jcode/builds/versions/0.14.6/jcode"
            )),
            Origin::Release
        );
    }

    #[test]
    fn classifies_cargo_target() {
        assert_eq!(
            Origin::classify(&PathBuf::from("/home/u/src/jcode/target/selfdev/jcode")),
            Origin::Source
        );
    }

    #[test]
    fn classifies_unknown_path() {
        assert_eq!(
            Origin::classify(&PathBuf::from("/usr/local/bin/jcode")),
            Origin::Unknown
        );
    }

    fn client(version: &str, hash: &str) -> ClientIdentity {
        client_with_dirty(version, hash, None)
    }

    fn client_with_dirty(version: &str, hash: &str, dirty: Option<bool>) -> ClientIdentity {
        ClientIdentity {
            path: "/x/jcode".into(),
            origin: "source".into(),
            version: version.into(),
            git_hash: hash.into(),
            dirty,
            build_source_dir: None,
        }
    }

    fn server(running: bool, version: Option<&str>, hash: Option<&str>) -> ServerIdentity {
        ServerIdentity {
            running,
            socket: "/x/sock".into(),
            pid: running.then_some(1),
            name: None,
            version: version.map(str::to_string),
            git_hash: hash.map(str::to_string),
            started_at: None,
        }
    }

    #[test]
    fn verdict_no_server_when_not_running() {
        let (v, _) = verdict(&client("v1", "abc1234"), &server(false, None, None));
        assert_eq!(v, Verdict::NoServer);
    }

    #[test]
    fn verdict_same_on_matching_hash() {
        let (v, _) = verdict(
            &client("v1", "abc1234"),
            &server(true, Some("v1"), Some("abc1234")),
        );
        assert_eq!(v, Verdict::Same);
    }

    #[test]
    fn verdict_same_tolerates_short_vs_full_hash() {
        let (v, _) = verdict(
            &client("v1", "abc1234"),
            &server(true, Some("v1"), Some("abc1234def567890")),
        );
        assert_eq!(v, Verdict::Same);
    }

    #[test]
    fn verdict_compatible_same_commit_but_dirty_client() {
        let (v, _) = verdict(
            &client_with_dirty("0.1.0", "abc1234", Some(true)),
            &server(true, Some("v0.1.0 (abc1234)"), Some("abc1234")),
        );
        assert_eq!(v, Verdict::Compatible);
    }

    #[test]
    fn verdict_reconnect_on_full_mismatch() {
        let (v, _) = verdict(
            &client("0.2.0", "abc1234"),
            &server(true, Some("v0.1.0 (def5678)"), Some("def5678")),
        );
        assert_eq!(v, Verdict::Reconnect);
    }

    #[test]
    fn build_source_dir_hidden_for_immutable_origins() {
        assert_eq!(build_source_dir(Origin::Nix), None);
        assert_eq!(build_source_dir(Origin::Release), None);
    }

    #[test]
    fn build_source_dir_shown_for_live_origins_when_stamped() {
        // The stamped value is whatever this test binary was built with; it is a
        // real path (cargo target tree), never empty/"unknown", so source/selfdev
        // origins surface Some(_).
        assert!(build_source_dir(Origin::Source).is_some());
        assert!(build_source_dir(Origin::Selfdev).is_some());
    }
}
