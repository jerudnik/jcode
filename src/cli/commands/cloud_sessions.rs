use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, Stdio};

pub enum CloudSubcommand {
    Sessions(CloudSessionsSubcommand),
}

pub enum CloudSessionsSubcommand {
    Configure {
        api_base: Option<String>,
        api_token: Option<String>,
        api_token_env: Option<String>,
        api_token_id: Option<String>,
        user_id: Option<String>,
        helper: Option<String>,
        clear: bool,
    },
    Status {
        json: bool,
    },
    Upload {
        session_file: String,
        raw: bool,
        user_id: String,
        profile: Option<String>,
        region: Option<String>,
        helper: Option<String>,
    },
    UploadLatest {
        sessions_dir: String,
        raw: bool,
        user_id: String,
        profile: Option<String>,
        region: Option<String>,
        helper: Option<String>,
    },
    Sync {
        sessions_dir: Option<String>,
        since_days: Option<u64>,
        all: bool,
        max: usize,
        min_interval_mins: Option<u64>,
        raw: bool,
        dry_run: bool,
        force: bool,
        json: bool,
        user_id: String,
        profile: Option<String>,
        region: Option<String>,
        helper: Option<String>,
    },
    List {
        limit: usize,
        json: bool,
        user_id: String,
        profile: Option<String>,
        region: Option<String>,
        helper: Option<String>,
    },
    Verify {
        session_id: String,
        user_id: String,
        profile: Option<String>,
        region: Option<String>,
        helper: Option<String>,
    },
    Dashboard {
        limit: usize,
        output: Option<String>,
        open: bool,
        with_view: bool,
        user_id: String,
        profile: Option<String>,
        region: Option<String>,
        helper: Option<String>,
    },
    View {
        session_id: String,
        format: String,
        output: Option<String>,
        open: bool,
        user_id: String,
        profile: Option<String>,
        region: Option<String>,
        helper: Option<String>,
    },
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(in crate::cli) struct CloudSessionsConfig {
    pub(super) api_base: Option<String>,
    pub(super) api_token: Option<String>,
    pub(super) api_token_id: Option<String>,
    pub(super) helper: Option<String>,
    pub(super) user_id: Option<String>,
}

#[derive(Debug, Serialize)]
struct CloudSessionsConfigStatus {
    path: String,
    exists: bool,
    api_base: Option<String>,
    api_token_configured: bool,
    api_token_id: Option<String>,
    helper: Option<String>,
    user_id: Option<String>,
}

pub fn run_cloud_command(cmd: CloudSubcommand) -> Result<()> {
    match cmd {
        CloudSubcommand::Sessions(action) => run_cloud_sessions_command(action),
    }
}

fn run_cloud_sessions_command(action: CloudSessionsSubcommand) -> Result<()> {
    match action {
        CloudSessionsSubcommand::Configure {
            api_base,
            api_token,
            api_token_env,
            api_token_id,
            user_id,
            helper,
            clear,
        } => run_cloud_sessions_configure(
            api_base,
            api_token,
            api_token_env,
            api_token_id,
            user_id,
            helper,
            clear,
        ),
        CloudSessionsSubcommand::Status { json } => run_cloud_sessions_status(json),
        CloudSessionsSubcommand::Dashboard {
            limit,
            output,
            open,
            with_view,
            user_id,
            profile,
            region,
            helper,
        } => run_cloud_sessions_dashboard(CloudSessionsDashboardRequest {
            limit,
            output,
            open,
            with_view,
            user_id,
            profile,
            region,
            helper,
        }),
        CloudSessionsSubcommand::Sync {
            sessions_dir,
            since_days,
            all,
            max,
            min_interval_mins,
            raw,
            dry_run,
            force,
            json,
            user_id,
            profile,
            region,
            helper,
        } => run_cloud_sessions_sync(CloudSessionsSyncRequest {
            sessions_dir,
            since_days,
            all,
            max,
            min_interval_mins,
            raw,
            dry_run,
            force,
            json,
            user_id,
            profile,
            region,
            helper,
        }),
        other => run_cloud_sessions_helper_command(other),
    }
}

fn run_cloud_sessions_helper_command(action: CloudSessionsSubcommand) -> Result<()> {
    let config = load_cloud_sessions_config()?.unwrap_or_default();
    let helper_override = cloud_sessions_helper_override(&action).or_else(|| config.helper.clone());
    let helper = resolve_jade_sessions_helper(helper_override.as_deref())?;
    let helper_env = cloud_sessions_helper_env(&config);
    let args = build_jade_sessions_args_with_config(action, &config);
    let mut command = ProcessCommand::new(&helper);
    command
        .args(&args)
        .envs(helper_env)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    let status = command
        .status()
        .map_err(|err| anyhow::anyhow!("failed to run {}: {err}", helper.display()))?;

    if !status.success() {
        anyhow::bail!("{} exited with status {status}", helper.display());
    }
    Ok(())
}

pub(in crate::cli) fn cloud_sessions_config_path() -> Result<PathBuf> {
    Ok(crate::storage::jcode_dir()?.join("cloud_sessions.json"))
}

pub(in crate::cli) fn load_cloud_sessions_config() -> Result<Option<CloudSessionsConfig>> {
    let path = cloud_sessions_config_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(&path)
        .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", path.display()))?;
    let config = serde_json::from_str(&content)
        .map_err(|err| anyhow::anyhow!("failed to parse {}: {err}", path.display()))?;
    Ok(Some(config))
}

fn save_cloud_sessions_config(config: &CloudSessionsConfig) -> Result<PathBuf> {
    let path = cloud_sessions_config_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_vec_pretty(config)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .mode(0o600)
            .open(&path)?;
        file.write_all(&content)?;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    }
    #[cfg(not(unix))]
    {
        std::fs::write(&path, &content)?;
    }
    Ok(path)
}

pub(in crate::cli) fn run_cloud_sessions_configure(
    api_base: Option<String>,
    api_token: Option<String>,
    api_token_env: Option<String>,
    api_token_id: Option<String>,
    user_id: Option<String>,
    helper: Option<String>,
    clear: bool,
) -> Result<()> {
    let path = cloud_sessions_config_path()?;
    if clear {
        match std::fs::remove_file(&path) {
            Ok(()) => println!("Removed Jade cloud sessions config at {}", path.display()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                println!("No Jade cloud sessions config found at {}", path.display());
            }
            Err(err) => return Err(err.into()),
        }
        return Ok(());
    }

    if api_base.is_none()
        && api_token.is_none()
        && api_token_env.is_none()
        && api_token_id.is_none()
        && user_id.is_none()
        && helper.is_none()
    {
        anyhow::bail!(
            "nothing to configure; pass --api-base, --api-token/--api-token-env, --api-token-id, --user-id, --helper, or --clear"
        );
    }

    let mut config = load_cloud_sessions_config()?.unwrap_or_default();
    if let Some(value) = non_empty(api_base) {
        config.api_base = Some(value);
    }
    if let Some(value) = non_empty(api_token) {
        config.api_token = Some(value);
    }
    if let Some(var) = non_empty(api_token_env) {
        let value = std::env::var(&var)
            .map_err(|err| anyhow::anyhow!("failed to read {var} for --api-token-env: {err}"))?;
        let value = non_empty(Some(value))
            .ok_or_else(|| anyhow::anyhow!("{var} for --api-token-env was empty"))?;
        config.api_token = Some(value);
    }
    if let Some(value) = non_empty(api_token_id) {
        config.api_token_id = Some(value);
    }
    if let Some(value) = non_empty(user_id) {
        config.user_id = Some(value);
    }
    if let Some(value) = non_empty(helper) {
        config.helper = Some(value);
    }

    let path = save_cloud_sessions_config(&config)?;
    println!("Saved Jade cloud sessions config to {}", path.display());
    println!("api_base: {}", configured_label(config.api_base.as_deref()));
    println!(
        "api_token: {}",
        if config.api_token.is_some() {
            "configured"
        } else {
            "not configured"
        }
    );
    println!(
        "api_token_id: {}",
        configured_label(config.api_token_id.as_deref())
    );
    println!("user_id: {}", configured_label(config.user_id.as_deref()));
    println!("helper: {}", configured_label(config.helper.as_deref()));
    Ok(())
}

fn run_cloud_sessions_status(json: bool) -> Result<()> {
    let path = cloud_sessions_config_path()?;
    let config = load_cloud_sessions_config()?.unwrap_or_default();
    let status = CloudSessionsConfigStatus {
        path: path.display().to_string(),
        exists: path.exists(),
        api_base: config.api_base,
        api_token_configured: config.api_token.is_some(),
        api_token_id: config.api_token_id,
        helper: config.helper,
        user_id: config.user_id,
    };
    if json {
        println!("{}", serde_json::to_string_pretty(&status)?);
    } else {
        println!("Jade cloud sessions config: {}", status.path);
        println!("exists: {}", status.exists);
        println!("api_base: {}", configured_label(status.api_base.as_deref()));
        println!(
            "api_token: {}",
            if status.api_token_configured {
                "configured"
            } else {
                "not configured"
            }
        );
        println!(
            "api_token_id: {}",
            configured_label(status.api_token_id.as_deref())
        );
        println!("user_id: {}", configured_label(status.user_id.as_deref()));
        println!("helper: {}", configured_label(status.helper.as_deref()));
    }
    Ok(())
}

fn configured_label(value: Option<&str>) -> &str {
    value
        .filter(|value| !value.is_empty())
        .unwrap_or("not configured")
}

fn config_or_default_user_id(user_id: String, config: &CloudSessionsConfig) -> String {
    if user_id == "dev" {
        config.user_id.clone().unwrap_or(user_id)
    } else {
        user_id
    }
}

fn non_empty(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub(in crate::cli) struct CloudSessionsSyncRequest {
    pub(super) sessions_dir: Option<String>,
    pub(super) since_days: Option<u64>,
    pub(super) all: bool,
    pub(super) max: usize,
    pub(super) min_interval_mins: Option<u64>,
    pub(super) raw: bool,
    pub(super) dry_run: bool,
    pub(super) force: bool,
    pub(super) json: bool,
    pub(super) user_id: String,
    pub(super) profile: Option<String>,
    pub(super) region: Option<String>,
    pub(super) helper: Option<String>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub(in crate::cli) struct CloudSessionsSyncState {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) last_sync_at: Option<String>,
    #[serde(default)]
    pub(super) sessions: std::collections::BTreeMap<String, CloudSessionsSyncRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct CloudSessionsSyncRecord {
    sha256: String,
    size: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    modified_unix: Option<i64>,
    uploaded_at: String,
}

#[derive(Debug, Serialize)]
struct CloudSessionsSyncEntry {
    session_id: String,
    path: String,
    status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct CloudSessionsSyncReport {
    sessions_dir: String,
    dry_run: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    throttled: bool,
    scanned: usize,
    uploaded: usize,
    skipped_unchanged: usize,
    failed: usize,
    reached_max: bool,
    entries: Vec<CloudSessionsSyncEntry>,
}

pub(in crate::cli) struct SyncCandidate {
    pub(in crate::cli) session_id: String,
    pub(in crate::cli) path: PathBuf,
    pub(in crate::cli) size: u64,
    pub(in crate::cli) modified_unix: Option<i64>,
}

pub(in crate::cli) fn cloud_sessions_sync_state_path() -> Result<PathBuf> {
    Ok(crate::storage::jcode_dir()?.join("cloud_sessions_sync.json"))
}

pub(in crate::cli) fn load_cloud_sessions_sync_state() -> Result<CloudSessionsSyncState> {
    let path = cloud_sessions_sync_state_path()?;
    if !path.exists() {
        return Ok(CloudSessionsSyncState::default());
    }
    let content = std::fs::read_to_string(&path)
        .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", path.display()))?;
    serde_json::from_str(&content)
        .map_err(|err| anyhow::anyhow!("failed to parse {}: {err}", path.display()))
}

pub(in crate::cli) fn save_cloud_sessions_sync_state(
    state: &CloudSessionsSyncState,
) -> Result<PathBuf> {
    let path = cloud_sessions_sync_state_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_vec_pretty(state)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .mode(0o600)
            .open(&path)?;
        file.write_all(&content)?;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    }
    #[cfg(not(unix))]
    {
        std::fs::write(&path, &content)?;
    }
    Ok(path)
}

fn resolve_sync_sessions_dir(override_path: Option<&str>) -> Result<PathBuf> {
    if let Some(path) = override_path.map(str::trim).filter(|path| !path.is_empty()) {
        return Ok(expand_home_path(path));
    }
    Ok(crate::storage::jcode_dir()?.join("sessions"))
}

fn expand_home_path(path: &str) -> PathBuf {
    if let Some(stripped) = path.strip_prefix("~/")
        && let Some(home) = dirs::home_dir()
    {
        return home.join(stripped);
    }
    if path == "~"
        && let Some(home) = dirs::home_dir()
    {
        return home;
    }
    PathBuf::from(path)
}

pub(in crate::cli) fn is_syncable_session_stem(stem: &str) -> bool {
    (stem.starts_with("session_") || stem.starts_with("imported_")) && !stem.ends_with(".journal")
}

fn sha256_file(path: &Path) -> Result<String> {
    use sha2::{Digest, Sha256};
    let mut file = std::fs::File::open(path)
        .map_err(|err| anyhow::anyhow!("failed to open {}: {err}", path.display()))?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher)
        .map_err(|err| anyhow::anyhow!("failed to hash {}: {err}", path.display()))?;
    Ok(hex::encode(hasher.finalize()))
}

pub(in crate::cli) fn collect_sync_candidates(dir: &Path) -> Result<Vec<SyncCandidate>> {
    let mut candidates = Vec::new();
    if !dir.exists() {
        anyhow::bail!("sessions directory not found: {}", dir.display());
    }
    for entry in std::fs::read_dir(dir)
        .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", dir.display()))?
        .flatten()
    {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        if !is_syncable_session_stem(stem) {
            continue;
        }
        let metadata = match entry.metadata() {
            Ok(metadata) if metadata.is_file() => metadata,
            _ => continue,
        };
        let modified_unix = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|dur| dur.as_secs() as i64);
        candidates.push(SyncCandidate {
            session_id: stem.to_string(),
            path,
            size: metadata.len(),
            modified_unix,
        });
    }
    Ok(candidates)
}

fn run_jade_upload(
    helper: &Path,
    helper_env: &[(&'static str, String)],
    file: &Path,
    user_id: &str,
    profile: Option<&str>,
    region: Option<&str>,
    raw: bool,
) -> Result<()> {
    let mut args = vec!["upload".to_string()];
    append_common_jade_args(
        &mut args,
        user_id.to_string(),
        profile.map(ToOwned::to_owned),
        region.map(ToOwned::to_owned),
    );
    if raw {
        args.push("--raw".to_string());
    }
    args.push(file.display().to_string());

    let output = ProcessCommand::new(helper)
        .args(&args)
        .envs(helper_env.iter().cloned())
        .stdin(Stdio::null())
        .output()
        .map_err(|err| anyhow::anyhow!("failed to run {}: {err}", helper.display()))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let detail = stderr.trim();
        let detail = if detail.is_empty() {
            format!("exited with status {}", output.status)
        } else {
            detail.lines().last().unwrap_or(detail).to_string()
        };
        anyhow::bail!(detail);
    }
    Ok(())
}

pub(in crate::cli) fn run_cloud_sessions_sync(request: CloudSessionsSyncRequest) -> Result<()> {
    let config = load_cloud_sessions_config()?.unwrap_or_default();
    let helper_override = request.helper.clone().or_else(|| config.helper.clone());
    let user_id = config_or_default_user_id(request.user_id.clone(), &config);
    let sessions_dir = resolve_sync_sessions_dir(request.sessions_dir.as_deref())?;
    let mut state = load_cloud_sessions_sync_state()?;

    // Self-throttle so the command is safe to call from cron/systemd timers without
    // re-uploading or even rescanning more often than requested.
    if !request.force
        && !request.dry_run
        && let Some(min_interval) = request.min_interval_mins
        && min_interval > 0
        && let Some(last) = state
            .last_sync_at
            .as_deref()
            .and_then(|ts| chrono::DateTime::parse_from_rfc3339(ts).ok())
    {
        let elapsed_mins = (chrono::Utc::now() - last.with_timezone(&chrono::Utc)).num_minutes();
        if elapsed_mins < min_interval as i64 {
            let report = CloudSessionsSyncReport {
                sessions_dir: sessions_dir.display().to_string(),
                dry_run: request.dry_run,
                throttled: true,
                scanned: 0,
                uploaded: 0,
                skipped_unchanged: 0,
                failed: 0,
                reached_max: false,
                entries: Vec::new(),
            };
            if request.json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!(
                    "Jade cloud sessions sync skipped: last sync {elapsed_mins}m ago (< --min-interval-mins {min_interval})"
                );
            }
            return Ok(());
        }
    }

    let helper = resolve_jade_sessions_helper(helper_override.as_deref())?;
    let helper_env = cloud_sessions_helper_env(&config);
    let mut candidates = collect_sync_candidates(&sessions_dir)?;

    if !request.all {
        let since_days = request.since_days.unwrap_or(7);
        let cutoff = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|dur| dur.as_secs() as i64)
            .unwrap_or(0)
            - (since_days as i64) * 86_400;
        candidates.retain(|candidate| candidate.modified_unix.map(|m| m >= cutoff).unwrap_or(true));
    }

    // Newest first so --max keeps the most recent sessions.
    candidates.sort_by(|a, b| b.modified_unix.cmp(&a.modified_unix));

    let mut report = CloudSessionsSyncReport {
        sessions_dir: sessions_dir.display().to_string(),
        dry_run: request.dry_run,
        throttled: false,
        scanned: 0,
        uploaded: 0,
        skipped_unchanged: 0,
        failed: 0,
        reached_max: false,
        entries: Vec::new(),
    };
    let mut state_dirty = false;

    for candidate in candidates {
        if report.uploaded + report.failed >= request.max {
            report.reached_max = true;
            break;
        }
        report.scanned += 1;
        let sha = match sha256_file(&candidate.path) {
            Ok(sha) => sha,
            Err(err) => {
                report.failed += 1;
                report.entries.push(CloudSessionsSyncEntry {
                    session_id: candidate.session_id,
                    path: candidate.path.display().to_string(),
                    status: "failed",
                    error: Some(err.to_string()),
                });
                continue;
            }
        };

        if !request.force
            && let Some(record) = state.sessions.get(&candidate.session_id)
            && record.sha256 == sha
        {
            report.skipped_unchanged += 1;
            continue;
        }

        if request.dry_run {
            report.uploaded += 1;
            report.entries.push(CloudSessionsSyncEntry {
                session_id: candidate.session_id,
                path: candidate.path.display().to_string(),
                status: "would-upload",
                error: None,
            });
            continue;
        }

        match run_jade_upload(
            &helper,
            &helper_env,
            &candidate.path,
            &user_id,
            request.profile.as_deref(),
            request.region.as_deref(),
            request.raw,
        ) {
            Ok(()) => {
                report.uploaded += 1;
                state.sessions.insert(
                    candidate.session_id.clone(),
                    CloudSessionsSyncRecord {
                        sha256: sha,
                        size: candidate.size,
                        modified_unix: candidate.modified_unix,
                        uploaded_at: chrono::Utc::now().to_rfc3339(),
                    },
                );
                state_dirty = true;
                report.entries.push(CloudSessionsSyncEntry {
                    session_id: candidate.session_id,
                    path: candidate.path.display().to_string(),
                    status: "uploaded",
                    error: None,
                });
            }
            Err(err) => {
                report.failed += 1;
                report.entries.push(CloudSessionsSyncEntry {
                    session_id: candidate.session_id,
                    path: candidate.path.display().to_string(),
                    status: "failed",
                    error: Some(err.to_string()),
                });
            }
        }
    }

    // Record completion time for non-dry runs (even if nothing changed) so
    // --min-interval-mins throttling works for schedulers, and persist any
    // newly uploaded session records.
    if !request.dry_run {
        state.last_sync_at = Some(chrono::Utc::now().to_rfc3339());
        save_cloud_sessions_sync_state(&state)?;
    } else if state_dirty {
        save_cloud_sessions_sync_state(&state)?;
    }

    if request.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        let verb = if request.dry_run {
            "Would upload"
        } else {
            "Uploaded"
        };
        println!("Jade cloud sessions sync ({})", report.sessions_dir);
        println!(
            "scanned: {}  {}: {}  unchanged: {}  failed: {}",
            report.scanned, verb, report.uploaded, report.skipped_unchanged, report.failed
        );
        if report.reached_max {
            println!("note: reached --max {}; rerun to continue", request.max);
        }
        for entry in &report.entries {
            match entry.error.as_deref() {
                Some(error) => println!("  [{}] {} ({})", entry.status, entry.session_id, error),
                None => println!("  [{}] {}", entry.status, entry.session_id),
            }
        }
    }

    if report.failed > 0 {
        anyhow::bail!("{} session(s) failed to upload", report.failed);
    }
    Ok(())
}

#[cfg(test)]
pub(in crate::cli) use dashboard::{
    CloudSessionListItem, dashboard_views_dir, parse_cloud_session_list_json, relative_link,
    render_cloud_sessions_dashboard_html, sanitize_filename,
};

pub(super) mod dashboard;

use dashboard::{CloudSessionsDashboardRequest, run_cloud_sessions_dashboard};
pub(in crate::cli) fn cloud_sessions_helper_env(
    config: &CloudSessionsConfig,
) -> Vec<(&'static str, String)> {
    let mut env = Vec::new();
    if let Some(api_base) = non_empty(config.api_base.clone()) {
        env.push(("JADE_API_BASE", api_base));
    }
    if let Some(api_token) = non_empty(config.api_token.clone()) {
        env.push(("JADE_API_TOKEN", api_token));
    }
    if let Some(api_token_id) = non_empty(config.api_token_id.clone()) {
        env.push(("JADE_API_TOKEN_ID", api_token_id));
    }
    env
}

fn cloud_sessions_helper_override(action: &CloudSessionsSubcommand) -> Option<String> {
    match action {
        CloudSessionsSubcommand::Configure { .. }
        | CloudSessionsSubcommand::Status { .. }
        | CloudSessionsSubcommand::Sync { .. } => None,
        CloudSessionsSubcommand::Upload { helper, .. }
        | CloudSessionsSubcommand::UploadLatest { helper, .. }
        | CloudSessionsSubcommand::List { helper, .. }
        | CloudSessionsSubcommand::Verify { helper, .. }
        | CloudSessionsSubcommand::Dashboard { helper, .. }
        | CloudSessionsSubcommand::View { helper, .. } => helper.clone(),
    }
}

fn append_common_jade_args(
    args: &mut Vec<String>,
    user_id: String,
    profile: Option<String>,
    region: Option<String>,
) {
    args.extend(["--user-id".to_string(), user_id]);
    if let Some(profile) = profile {
        args.extend(["--profile".to_string(), profile]);
    }
    if let Some(region) = region {
        args.extend(["--region".to_string(), region]);
    }
}

#[cfg(test)]
pub(in crate::cli) fn build_jade_sessions_args(action: CloudSessionsSubcommand) -> Vec<String> {
    build_jade_sessions_args_with_config(action, &CloudSessionsConfig::default())
}

pub(in crate::cli) fn build_jade_sessions_args_with_config(
    action: CloudSessionsSubcommand,
    config: &CloudSessionsConfig,
) -> Vec<String> {
    match action {
        CloudSessionsSubcommand::Configure { .. }
        | CloudSessionsSubcommand::Status { .. }
        | CloudSessionsSubcommand::Sync { .. }
        | CloudSessionsSubcommand::Dashboard { .. } => {
            unreachable!(
                "configure/status/sync/dashboard do not invoke the Jade helper via this builder"
            )
        }
        CloudSessionsSubcommand::Upload {
            session_file,
            raw,
            user_id,
            profile,
            region,
            ..
        } => {
            let mut args = vec!["upload".to_string()];
            append_common_jade_args(
                &mut args,
                config_or_default_user_id(user_id, config),
                profile,
                region,
            );
            if raw {
                args.push("--raw".to_string());
            }
            args.push(session_file);
            args
        }
        CloudSessionsSubcommand::UploadLatest {
            sessions_dir,
            raw,
            user_id,
            profile,
            region,
            ..
        } => {
            let mut args = vec!["upload-latest".to_string()];
            append_common_jade_args(
                &mut args,
                config_or_default_user_id(user_id, config),
                profile,
                region,
            );
            args.extend(["--sessions-dir".to_string(), sessions_dir]);
            if raw {
                args.push("--raw".to_string());
            }
            args
        }
        CloudSessionsSubcommand::List {
            limit,
            json,
            user_id,
            profile,
            region,
            ..
        } => {
            let mut args = vec!["list".to_string()];
            append_common_jade_args(
                &mut args,
                config_or_default_user_id(user_id, config),
                profile,
                region,
            );
            args.extend(["--limit".to_string(), limit.to_string()]);
            if json {
                args.push("--json".to_string());
            }
            args
        }
        CloudSessionsSubcommand::Verify {
            session_id,
            user_id,
            profile,
            region,
            ..
        } => {
            let mut args = vec!["verify".to_string()];
            append_common_jade_args(
                &mut args,
                config_or_default_user_id(user_id, config),
                profile,
                region,
            );
            args.push(session_id);
            args
        }
        CloudSessionsSubcommand::View {
            session_id,
            format,
            output,
            open,
            user_id,
            profile,
            region,
            ..
        } => {
            let mut args = vec!["view".to_string()];
            append_common_jade_args(
                &mut args,
                config_or_default_user_id(user_id, config),
                profile,
                region,
            );
            args.extend(["--format".to_string(), format]);
            if let Some(output) = output {
                args.extend(["--output".to_string(), output]);
            }
            if open {
                args.push("--open".to_string());
            }
            args.push(session_id);
            args
        }
    }
}

pub(in crate::cli) fn resolve_jade_sessions_helper(override_path: Option<&str>) -> Result<PathBuf> {
    if let Some(path) = override_path.map(str::trim).filter(|path| !path.is_empty()) {
        return Ok(PathBuf::from(path));
    }

    if let Some(path) = std::env::var_os("JCODE_JADE_SESSIONS_HELPER")
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
    {
        return Ok(path);
    }

    let mut candidates = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("../jade/scripts/jade_sessions.py"));
        candidates.push(cwd.join("jade/scripts/jade_sessions.py"));
    }
    if let Some(home) = dirs::home_dir() {
        candidates.push(home.join("jade/scripts/jade_sessions.py"));
    }

    for candidate in candidates {
        if is_executable_file(&candidate) {
            return Ok(candidate);
        }
    }

    anyhow::bail!(
        "Could not find Jade session helper. Set --helper PATH or JCODE_JADE_SESSIONS_HELPER. Expected a private helper like ~/jade/scripts/jade_sessions.py"
    );
}

#[cfg(unix)]
fn is_executable_file(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.is_file()
        && path
            .metadata()
            .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable_file(path: &Path) -> bool {
    path.is_file()
}
