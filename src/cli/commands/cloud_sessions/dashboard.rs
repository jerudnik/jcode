use anyhow::Result;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, Stdio};

use super::{
    append_common_jade_args, cloud_sessions_helper_env, config_or_default_user_id,
    expand_home_path, load_cloud_sessions_config, resolve_jade_sessions_helper,
};

pub(super) struct CloudSessionsDashboardRequest {
    pub(super) limit: usize,
    pub(super) output: Option<String>,
    pub(super) open: bool,
    pub(super) with_view: bool,
    pub(super) user_id: String,
    pub(super) profile: Option<String>,
    pub(super) region: Option<String>,
    pub(super) helper: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(in crate::cli) struct CloudSessionListItem {
    #[serde(default)]
    pub(in crate::cli) session_id: Option<String>,
    #[serde(default)]
    pub(super) title: Option<String>,
    #[serde(default)]
    pub(super) short_name: Option<String>,
    #[serde(default)]
    pub(super) message_count: Option<serde_json::Value>,
    #[serde(default)]
    pub(super) uploaded_at: Option<String>,
}

fn fetch_cloud_session_list_json(
    helper: &Path,
    helper_env: &[(&'static str, String)],
    user_id: &str,
    profile: Option<&str>,
    region: Option<&str>,
    limit: usize,
) -> Result<Vec<CloudSessionListItem>> {
    let mut args = vec!["list".to_string()];
    append_common_jade_args(
        &mut args,
        user_id.to_string(),
        profile.map(ToOwned::to_owned),
        region.map(ToOwned::to_owned),
    );
    args.extend(["--limit".to_string(), limit.to_string()]);
    args.push("--json".to_string());

    let output = ProcessCommand::new(helper)
        .args(&args)
        .envs(helper_env.iter().cloned())
        .stdin(Stdio::null())
        .output()
        .map_err(|err| anyhow::anyhow!("failed to run {}: {err}", helper.display()))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let detail = stderr.trim();
        anyhow::bail!(
            "{} list failed: {}",
            helper.display(),
            if detail.is_empty() {
                format!("exited with status {}", output.status)
            } else {
                detail.to_string()
            }
        );
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_cloud_session_list_json(stdout.trim())
}

/// Parse the helper's `list --json` output.
///
/// The Jade helper prints a top-level JSON array, but we also accept an object
/// wrapper keyed by `items` or `sessions` so the dashboard keeps working if the
/// helper's output shape changes.
pub(in crate::cli) fn parse_cloud_session_list_json(raw: &str) -> Result<Vec<CloudSessionListItem>> {
    let value: serde_json::Value = serde_json::from_str(raw)
        .map_err(|err| anyhow::anyhow!("failed to parse Jade list JSON: {err}"))?;
    let array = match value {
        serde_json::Value::Array(items) => items,
        serde_json::Value::Object(mut map) => map
            .remove("items")
            .or_else(|| map.remove("sessions"))
            .and_then(|value| match value {
                serde_json::Value::Array(items) => Some(items),
                _ => None,
            })
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "failed to parse Jade list JSON: expected an array or an object with an `items`/`sessions` array"
                )
            })?,
        other => anyhow::bail!(
            "failed to parse Jade list JSON: expected an array, found {}",
            json_value_kind(&other)
        ),
    };
    array
        .into_iter()
        .map(|item| {
            serde_json::from_value(item)
                .map_err(|err| anyhow::anyhow!("failed to parse Jade list item: {err}"))
        })
        .collect()
}

fn json_value_kind(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "a boolean",
        serde_json::Value::Number(_) => "a number",
        serde_json::Value::String(_) => "a string",
        serde_json::Value::Array(_) => "an array",
        serde_json::Value::Object(_) => "an object",
    }
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn message_count_label(value: Option<&serde_json::Value>) -> String {
    match value {
        Some(serde_json::Value::Number(num)) => num.to_string(),
        Some(serde_json::Value::String(text)) => text.clone(),
        _ => "-".to_string(),
    }
}

pub(in crate::cli) fn render_cloud_sessions_dashboard_html(
    user_id: &str,
    items: &[CloudSessionListItem],
    view_links: &std::collections::BTreeMap<String, String>,
) -> String {
    let generated = chrono::Utc::now().to_rfc3339();
    let mut rows = String::new();
    for item in items {
        let session_id = item.session_id.as_deref().unwrap_or("(unknown)");
        let title = item
            .title
            .as_deref()
            .filter(|value| !value.is_empty())
            .or(item.short_name.as_deref())
            .unwrap_or("(untitled)");
        let uploaded = item.uploaded_at.as_deref().unwrap_or("-");
        // When a local per-session viewer was generated, link the session id to it.
        let id_cell = match item.session_id.as_deref().and_then(|id| view_links.get(id)) {
            Some(link) => format!(
                "<a href='{}'>{}</a>",
                html_escape(link),
                html_escape(session_id)
            ),
            None => html_escape(session_id),
        };
        rows.push_str(&format!(
            "<tr><td class='id'>{}</td><td>{}</td><td class='num'>{}</td><td class='ts'>{}</td></tr>\n",
            id_cell,
            html_escape(title),
            html_escape(&message_count_label(item.message_count.as_ref())),
            html_escape(uploaded),
        ));
    }
    if rows.is_empty() {
        rows.push_str("<tr><td colspan='4' class='empty'>No uploaded sessions found.</td></tr>\n");
    }
    format!(
        "<!doctype html><meta charset='utf-8'>\n\
<title>Jade Cloud Sessions Dashboard</title>\n\
<style>body{{font-family:system-ui,sans-serif;max-width:1100px;margin:2rem auto;padding:0 1rem;color:#1b1b1f}}\
h1{{margin-bottom:0.2rem}}.meta{{color:#666;margin-bottom:1.5rem}}\
table{{border-collapse:collapse;width:100%}}th,td{{text-align:left;padding:0.5rem 0.6rem;border-bottom:1px solid #e3e3e8}}\
th{{background:#f6f8fa;position:sticky;top:0}}td.id{{font-family:ui-monospace,monospace;font-size:0.85rem}}\
td.id a{{color:#0a58ca;text-decoration:none}}td.id a:hover{{text-decoration:underline}}\
td.num{{text-align:right}}td.ts{{white-space:nowrap;color:#555}}td.empty{{text-align:center;color:#888;padding:2rem}}\
tr:hover td{{background:#fafbff}}</style>\n\
<h1>Jade Cloud Sessions</h1>\n\
<div class='meta'>user: {user} &middot; {count} session(s) &middot; generated {generated}</div>\n\
<table><thead><tr><th>Session ID</th><th>Title</th><th>Messages</th><th>Uploaded</th></tr></thead>\n\
<tbody>\n{rows}</tbody></table>\n",
        user = html_escape(user_id),
        count = items.len(),
        generated = html_escape(&generated),
        rows = rows,
    )
}

pub(super) fn run_cloud_sessions_dashboard(request: CloudSessionsDashboardRequest) -> Result<()> {
    let config = load_cloud_sessions_config()?.unwrap_or_default();
    let helper_override = request.helper.clone().or_else(|| config.helper.clone());
    let helper = resolve_jade_sessions_helper(helper_override.as_deref())?;
    let helper_env = cloud_sessions_helper_env(&config);
    let user_id = config_or_default_user_id(request.user_id.clone(), &config);

    let items = fetch_cloud_session_list_json(
        &helper,
        &helper_env,
        &user_id,
        request.profile.as_deref(),
        request.region.as_deref(),
        request.limit,
    )?;

    let output_path = match request
        .output
        .as_deref()
        .map(str::trim)
        .filter(|p| !p.is_empty())
    {
        Some(path) => expand_home_path(path),
        None => std::env::temp_dir().join(format!(
            "jade-cloud-dashboard-{}.html",
            chrono::Utc::now().format("%Y%m%d-%H%M%S")
        )),
    };
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Optionally download each session and render a local per-session viewer,
    // then link the dashboard rows to those files (relative to the dashboard).
    let mut view_links: std::collections::BTreeMap<String, String> =
        std::collections::BTreeMap::new();
    if request.with_view {
        let views_dir = dashboard_views_dir(&output_path);
        std::fs::create_dir_all(&views_dir)?;
        let total = items.len();
        let mut generated = 0usize;
        for (idx, item) in items.iter().enumerate() {
            let Some(session_id) = item.session_id.as_deref().filter(|id| !id.is_empty()) else {
                continue;
            };
            let view_file = views_dir.join(format!("{}.html", sanitize_filename(session_id)));
            eprintln!("[{}/{}] downloading {}", idx + 1, total, session_id);
            match generate_cloud_session_view_html(
                &helper,
                &helper_env,
                &user_id,
                request.profile.as_deref(),
                request.region.as_deref(),
                session_id,
                &view_file,
            ) {
                Ok(()) => {
                    if let Some(rel) = relative_link(&output_path, &view_file) {
                        view_links.insert(session_id.to_string(), rel);
                        generated += 1;
                    }
                }
                Err(err) => {
                    eprintln!("  warning: could not render viewer for {session_id}: {err}");
                }
            }
        }
        eprintln!(
            "Generated {generated}/{total} per-session viewer(s) in {}",
            views_dir.display()
        );
    }

    let html = render_cloud_sessions_dashboard_html(&user_id, &items, &view_links);
    std::fs::write(&output_path, html.as_bytes())
        .map_err(|err| anyhow::anyhow!("failed to write {}: {err}", output_path.display()))?;

    println!(
        "Wrote Jade cloud sessions dashboard ({} session(s)) to {}",
        items.len(),
        output_path.display()
    );
    if request.open {
        let _ = open::that(&output_path);
    }
    Ok(())
}

/// Directory that holds per-session viewer HTML files for a dashboard.
pub(in crate::cli) fn dashboard_views_dir(dashboard_path: &Path) -> PathBuf {
    let stem = dashboard_path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "dashboard".to_string());
    let parent = dashboard_path.parent().unwrap_or_else(|| Path::new("."));
    parent.join(format!("{stem}-views"))
}

/// Make a filesystem-safe filename component from a session id.
pub(in crate::cli) fn sanitize_filename(value: &str) -> String {
    value
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Build a link from the dashboard file to a viewer file, preferring a relative
/// path when both share a parent directory so the dashboard is portable.
pub(in crate::cli) fn relative_link(dashboard_path: &Path, view_file: &Path) -> Option<String> {
    let base = dashboard_path.parent()?;
    let rel = view_file.strip_prefix(base).ok()?;
    Some(rel.to_string_lossy().replace('\\', "/"))
}

/// Invoke the helper's `view --format html --output <file>` for one session.
fn generate_cloud_session_view_html(
    helper: &Path,
    helper_env: &[(&'static str, String)],
    user_id: &str,
    profile: Option<&str>,
    region: Option<&str>,
    session_id: &str,
    output_file: &Path,
) -> Result<()> {
    let mut args = vec!["view".to_string()];
    append_common_jade_args(
        &mut args,
        user_id.to_string(),
        profile.map(ToOwned::to_owned),
        region.map(ToOwned::to_owned),
    );
    args.extend(["--format".to_string(), "html".to_string()]);
    args.extend([
        "--output".to_string(),
        output_file.to_string_lossy().to_string(),
    ]);
    args.push(session_id.to_string());

    let output = ProcessCommand::new(helper)
        .args(&args)
        .envs(helper_env.iter().cloned())
        .stdin(Stdio::null())
        .output()
        .map_err(|err| anyhow::anyhow!("failed to run {}: {err}", helper.display()))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let detail = stderr.trim();
        anyhow::bail!(
            "{}",
            if detail.is_empty() {
                format!("view exited with status {}", output.status)
            } else {
                detail.to_string()
            }
        );
    }
    Ok(())
}
