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
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, Stdio};
use std::sync::mpsc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const GITHUB_MAIN_REMOTE: &str = "https://github.com/jerudnik/jcode.git";
const GITHUB_CACHE_TTL: Duration = Duration::from_secs(60 * 60);
const GITHUB_CHECK_TIMEOUT: Duration = Duration::from_secs(2);
const VERSION_CHECK_TIMEOUT: Duration = Duration::from_millis(500);

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

#[derive(Debug, Clone, Serialize)]
struct InstalledIdentity {
    path: String,
    version: Option<String>,
    git_hash: Option<String>,
    store_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GithubLatest {
    sha: String,
    checked_at: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct VersionCheckCache {
    github_latest: GithubLatest,
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
    nix_installed: Option<InstalledIdentity>,
    github_latest: Option<GithubLatest>,
    verdict: String,
    verdict_detail: String,
    drift_summary: Option<String>,
    fallback: String,
}

fn client_identity() -> ClientIdentity {
    let path = running_binary_path()
        .or_else(|| crate::build::current_binary_path().ok())
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

fn running_binary_path() -> Option<PathBuf> {
    std::env::current_exe().ok().map(canonicalize_or)
}

fn canonicalize_or(path: PathBuf) -> PathBuf {
    std::fs::canonicalize(&path).unwrap_or(path)
}

fn same_path(a: &Path, b: &Path) -> bool {
    canonicalize_or(a.to_path_buf()) == canonicalize_or(b.to_path_buf())
}

fn is_nix_profile_candidate(path: &Path) -> bool {
    let p = path.to_string_lossy();
    p.contains("/nix/store/") || p.contains("/etc/profiles/") || p.contains("/run/current-system/")
}

fn store_hash_from_path(path: &Path) -> Option<String> {
    let p = path.to_string_lossy();
    let marker = "/nix/store/";
    let start = p.find(marker)? + marker.len();
    let rest = &p[start..];
    let first = rest.split('/').next()?;
    let hash = first.split('-').next()?.trim();
    (!hash.is_empty()).then(|| hash.to_string())
}

fn parse_jcode_version_line(line: &str) -> (Option<String>, Option<String>) {
    let trimmed = line.trim();
    let version = trimmed
        .split_whitespace()
        .find(|part| part.starts_with('v') && part.len() > 1)
        .map(str::to_string);
    let git_hash = trimmed
        .split_once('(')
        .and_then(|(_, rest)| rest.split_once(')').map(|(hash, _)| hash.trim()))
        .filter(|hash| !hash.is_empty() && *hash != "unknown")
        .map(str::to_string);
    (version, git_hash)
}

fn command_output_with_timeout(
    mut command: ProcessCommand,
    timeout: Duration,
) -> Option<std::process::Output> {
    let (tx, rx) = mpsc::channel();
    std::thread::Builder::new()
        .name("jcode-version-probe".to_string())
        .spawn(move || {
            let _ = tx.send(command.output());
        })
        .ok()?;
    rx.recv_timeout(timeout).ok()?.ok()
}

fn installed_version(path: &Path) -> (Option<String>, Option<String>) {
    let mut command = ProcessCommand::new(path);
    command
        .arg("--version")
        .stdin(Stdio::null())
        .stderr(Stdio::null());
    let Some(output) = command_output_with_timeout(command, VERSION_CHECK_TIMEOUT) else {
        return (None, None);
    };
    if !output.status.success() {
        return (None, None);
    }
    let text = String::from_utf8_lossy(&output.stdout);
    parse_jcode_version_line(&text)
}

fn nix_installed_identity() -> Option<InstalledIdentity> {
    let running = running_binary_path();
    nix_installed_identity_with_running(running.as_deref())
}

fn nix_installed_identity_with_running(running: Option<&Path>) -> Option<InstalledIdentity> {
    let launcher = crate::build::launcher_binary_path()
        .ok()
        .map(canonicalize_or);
    let path_var = std::env::var_os("PATH")?;
    let mut fallback = None;

    for dir in std::env::split_paths(&path_var) {
        let candidate = canonicalize_or(dir.join(crate::build::binary_name()));
        if !candidate.exists() {
            continue;
        }
        if running.is_some_and(|running| same_path(&candidate, running)) {
            continue;
        }
        if launcher
            .as_deref()
            .is_some_and(|launcher| same_path(&candidate, launcher))
        {
            continue;
        }
        if is_nix_profile_candidate(&candidate) {
            fallback = Some(candidate);
            break;
        }
    }

    let path = fallback?;
    let (version, git_hash) = installed_version(&path);
    Some(InstalledIdentity {
        path: path.display().to_string(),
        version,
        git_hash,
        store_hash: store_hash_from_path(&path),
    })
}

fn cache_path() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(".jcode").join("version-check-cache.json"))
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn read_github_cache(ttl: Duration) -> Option<GithubLatest> {
    let path = cache_path()?;
    let text = std::fs::read_to_string(path).ok()?;
    let cache: VersionCheckCache = serde_json::from_str(&text).ok()?;
    let age = now_secs().saturating_sub(cache.github_latest.checked_at);
    (age <= ttl.as_secs()).then_some(cache.github_latest)
}

fn write_github_cache(latest: &GithubLatest) {
    let Some(path) = cache_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let cache = VersionCheckCache {
        github_latest: latest.clone(),
    };
    if let Ok(json) = serde_json::to_string_pretty(&cache) {
        let _ = std::fs::write(path, json);
    }
}

fn github_latest(ttl: Duration) -> Option<GithubLatest> {
    if let Some(cached) = read_github_cache(ttl) {
        return Some(cached);
    }

    let mut command = ProcessCommand::new("git");
    command
        .args(["ls-remote", GITHUB_MAIN_REMOTE, "main"])
        .stdin(Stdio::null())
        .stderr(Stdio::null());
    let output = command_output_with_timeout(command, GITHUB_CHECK_TIMEOUT)?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let sha = text.split_whitespace().next()?.trim();
    if sha.is_empty() {
        return None;
    }
    let latest = GithubLatest {
        sha: short_hash(sha).to_string(),
        checked_at: now_secs(),
    };
    write_github_cache(&latest);
    Some(latest)
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

fn short_hash(hash: &str) -> &str {
    let hash = hash.trim();
    let end = hash.len().min(8);
    &hash[..end]
}

fn hashes_match(a: &str, b: &str) -> bool {
    let a = a.trim();
    let b = b.trim();
    !a.is_empty() && !b.is_empty() && (a.starts_with(b) || b.starts_with(a))
}

fn drift_messages(
    running_hash: Option<&str>,
    nix_hash: Option<&str>,
    github_sha: Option<&str>,
) -> Vec<String> {
    let mut messages = Vec::new();

    if let (Some(running), Some(nix)) = (running_hash, nix_hash)
        && !hashes_match(running, nix)
    {
        messages.push(format!(
            "running binary is not the nix-installed one (running {} vs nix {}) -- repoint ~/.local/bin/jcode or relaunch",
            short_hash(running),
            short_hash(nix)
        ));
    }

    if let (Some(nix), Some(github)) = (nix_hash, github_sha)
        && !hashes_match(nix, github)
    {
        messages.push(format!(
            "nix build is behind github main ({} vs {}) -- run `nix flake update jcode`",
            short_hash(nix),
            short_hash(github)
        ));
    }

    messages
}

pub fn running_vs_installed_drift() -> Option<String> {
    std::env::var_os("JCODE_NIX_MANAGED")?;

    let running_path = running_binary_path();
    if running_path
        .as_deref()
        .is_some_and(|path| Origin::classify(path) == Origin::Nix)
    {
        return None;
    }

    let installed = nix_installed_identity_with_running(running_path.as_deref())?;
    let nix_hash = installed.git_hash.as_deref()?;
    drift_messages(Some(jcode_build_meta::GIT_HASH), Some(nix_hash), None)
        .into_iter()
        .next()
}

/// Best fallback command for getting a known-good binary.
fn fallback_command() -> String {
    "nix run github:jerudnik/jcode -- doctor   (or `jcode server reload`)".to_string()
}

pub fn run_doctor_command(emit_json: bool) -> Result<()> {
    let client = client_identity();
    let server = server_identity();
    let nix_installed = nix_installed_identity();
    let github_latest = github_latest(GITHUB_CACHE_TTL);
    let (v, detail) = verdict(&client, &server);
    let drift_messages = drift_messages(
        Some(client.git_hash.as_str()),
        nix_installed
            .as_ref()
            .and_then(|installed| installed.git_hash.as_deref()),
        github_latest.as_ref().map(|latest| latest.sha.as_str()),
    );
    let drift_summary = (!drift_messages.is_empty()).then(|| drift_messages.join("; "));

    let report = DoctorReport {
        client,
        server,
        nix_installed,
        github_latest,
        verdict: format!("{v:?}").to_lowercase(),
        verdict_detail: detail,
        drift_summary,
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
    if let Some(installed) = report.nix_installed.as_ref() {
        let version = installed.version.as_deref().unwrap_or("?");
        let hash = installed
            .git_hash
            .as_deref()
            .or(installed.store_hash.as_deref())
            .unwrap_or("?");
        println!(
            "nix:     {}  {} ({})",
            installed.path,
            version,
            short_hash(hash)
        );
    } else {
        println!("nix:     (not found on PATH)");
    }
    if let Some(latest) = report.github_latest.as_ref() {
        println!(
            "github:  main {} checked_at={}",
            short_hash(&latest.sha),
            latest.checked_at
        );
    } else {
        println!("github:  main (unavailable)");
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
    if let Some(summary) = report.drift_summary.as_deref() {
        println!("drift:   {summary}");
    }
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
    fn drift_messages_empty_when_hashes_match() {
        assert!(drift_messages(Some("abc1234"), Some("abc1234"), None).is_empty());
        assert!(
            drift_messages(Some("abc1234"), Some("abc1234def5678"), Some("abc1234")).is_empty()
        );
    }

    #[test]
    fn drift_messages_warns_when_running_differs_from_nix() {
        let messages = drift_messages(Some("abc1234"), Some("def5678"), None);
        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("running binary is not the nix-installed one"));
        assert!(messages[0].contains("running abc1234 vs nix def5678"));
        assert!(messages[0].contains("repoint ~/.local/bin/jcode or relaunch"));
    }

    #[test]
    fn drift_messages_warns_when_nix_is_behind_github() {
        let messages = drift_messages(Some("abc1234"), Some("abc1234"), Some("def5678"));
        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("nix build is behind github main"));
        assert!(messages[0].contains("abc1234 vs def5678"));
        assert!(messages[0].contains("nix flake update jcode"));
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
