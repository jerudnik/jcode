use serde_json::Value;
use std::collections::HashMap;
use std::ffi::OsString;
use std::fmt;
use std::path::{Component, Path, PathBuf};
use std::sync::{LazyLock, RwLock};

/// Filesystem authority is the canonical assigned directory, not the Git
/// repository or worktree that contains it. An assignment may intentionally
/// name a subdirectory, and sharing a repository must not grant access to a
/// sibling worktree.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct PlanScopeKey {
    swarm_id: String,
    plan_generation: u64,
}

#[derive(Default)]
struct ExecutionScopeRegistry {
    plans: HashMap<PlanScopeKey, PathBuf>,
    sessions: HashMap<String, PlanScopeKey>,
}

static EXECUTION_SCOPES: LazyLock<RwLock<ExecutionScopeRegistry>> =
    LazyLock::new(|| RwLock::new(ExecutionScopeRegistry::default()));

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ExecutionScopeError {
    InvalidBoundary {
        boundary: PathBuf,
        reason: String,
    },
    BoundaryChanged {
        swarm_id: String,
        plan_generation: u64,
        recorded: PathBuf,
        requested: PathBuf,
    },
    MissingPlanScope {
        swarm_id: String,
        plan_generation: u64,
    },
    SessionAlreadyBound {
        session_id: String,
    },
    MissingBoundScope {
        session_id: String,
    },
    InvalidToolInput {
        tool: String,
        boundary: PathBuf,
        reason: String,
    },
    OutsideBoundary {
        tool: String,
        boundary: PathBuf,
        target: PathBuf,
    },
    VcsMetadataDenied {
        tool: String,
        boundary: PathBuf,
        target: PathBuf,
    },
    SandboxUnavailable {
        tool: String,
        boundary: PathBuf,
    },
    UnknownFilesystemEffects {
        tool: String,
        boundary: PathBuf,
    },
}

impl fmt::Display for ExecutionScopeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidBoundary { boundary, reason } => write!(
                f,
                "invalid execution directory boundary '{}': {reason}",
                boundary.display()
            ),
            Self::BoundaryChanged {
                swarm_id,
                plan_generation,
                recorded,
                requested,
            } => write!(
                f,
                "execution directory boundary for swarm '{swarm_id}' generation {plan_generation} is already '{}' and cannot change to '{}'",
                recorded.display(),
                requested.display()
            ),
            Self::MissingPlanScope {
                swarm_id,
                plan_generation,
            } => write!(
                f,
                "no execution directory boundary is recorded for swarm '{swarm_id}' generation {plan_generation}"
            ),
            Self::SessionAlreadyBound { session_id } => write!(
                f,
                "session '{session_id}' already has an execution directory boundary; clear it before reuse"
            ),
            Self::MissingBoundScope { session_id } => write!(
                f,
                "session '{session_id}' references an execution directory boundary that is not recorded"
            ),
            Self::InvalidToolInput {
                tool,
                boundary,
                reason,
            } => write!(
                f,
                "tool '{tool}' was refused at execution directory boundary '{}': {reason}",
                boundary.display()
            ),
            Self::OutsideBoundary {
                tool,
                boundary,
                target,
            } => write!(
                f,
                "tool '{tool}' was refused: target '{}' is outside execution directory boundary '{}'",
                target.display(),
                boundary.display()
            ),
            Self::VcsMetadataDenied {
                tool,
                boundary,
                target,
            } => write!(
                f,
                "tool '{tool}' was refused: Git control path '{}' is not accessible at execution directory boundary '{}'",
                target.display(),
                boundary.display()
            ),
            Self::SandboxUnavailable { tool, boundary } => write!(
                f,
                "tool '{tool}' was refused at execution directory boundary '{}': no filesystem sandbox is available",
                boundary.display()
            ),
            Self::UnknownFilesystemEffects { tool, boundary } => write!(
                f,
                "tool '{tool}' was refused at execution directory boundary '{}': filesystem effects are not classified",
                boundary.display()
            ),
        }
    }
}

impl std::error::Error for ExecutionScopeError {}

#[allow(
    dead_code,
    reason = "called by the graph owner after this provider API lands"
)]
pub(crate) fn record_plan_execution_scope(
    swarm_id: &str,
    plan_generation: u64,
    root: &Path,
) -> Result<PathBuf, ExecutionScopeError> {
    let canonical_root =
        std::fs::canonicalize(root).map_err(|error| ExecutionScopeError::InvalidBoundary {
            boundary: root.to_path_buf(),
            reason: error.to_string(),
        })?;
    if !canonical_root.is_dir() {
        return Err(ExecutionScopeError::InvalidBoundary {
            boundary: canonical_root,
            reason: "boundary is not a directory".to_string(),
        });
    }

    let key = PlanScopeKey {
        swarm_id: swarm_id.to_string(),
        plan_generation,
    };
    let mut scopes = EXECUTION_SCOPES
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(recorded) = scopes.plans.get(&key) {
        if recorded != &canonical_root {
            return Err(ExecutionScopeError::BoundaryChanged {
                swarm_id: swarm_id.to_string(),
                plan_generation,
                recorded: recorded.clone(),
                requested: canonical_root,
            });
        }
        return Ok(recorded.clone());
    }
    scopes.plans.insert(key, canonical_root.clone());
    Ok(canonical_root)
}

#[allow(
    dead_code,
    reason = "called by the assignment owner after this provider API lands"
)]
pub(crate) fn bind_session_execution_scope(
    session_id: &str,
    swarm_id: &str,
    plan_generation: u64,
) -> Result<(), ExecutionScopeError> {
    let key = PlanScopeKey {
        swarm_id: swarm_id.to_string(),
        plan_generation,
    };
    let mut scopes = EXECUTION_SCOPES
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if !scopes.plans.contains_key(&key) {
        return Err(ExecutionScopeError::MissingPlanScope {
            swarm_id: swarm_id.to_string(),
            plan_generation,
        });
    }
    if let Some(current) = scopes.sessions.get(session_id) {
        if current == &key {
            return Ok(());
        }
        return Err(ExecutionScopeError::SessionAlreadyBound {
            session_id: session_id.to_string(),
        });
    }
    scopes.sessions.insert(session_id.to_string(), key);
    Ok(())
}

#[allow(
    dead_code,
    reason = "called by the assignment owner after this provider API lands"
)]
pub(crate) fn clear_session_execution_scope(session_id: &str) {
    EXECUTION_SCOPES
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .sessions
        .remove(session_id);
}

pub(crate) fn authorize_tool_call(
    session_id: &str,
    tool_name: &str,
    input: &Value,
) -> Result<Option<PathBuf>, ExecutionScopeError> {
    let boundary =
        {
            let scopes = EXECUTION_SCOPES
                .read()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let Some(key) = scopes.sessions.get(session_id) else {
                return Ok(None);
            };
            scopes.plans.get(key).cloned().ok_or_else(|| {
                ExecutionScopeError::MissingBoundScope {
                    session_id: session_id.to_string(),
                }
            })?
        };

    match tool_name {
        "read" => {
            let target = required_string(input, "file_path", tool_name, &boundary)?;
            authorize_target(tool_name, &boundary, target)?;
        }
        "ls" => {
            authorize_optional_target(input, "path", tool_name, &boundary)?;
        }
        "agentgrep" => {
            authorize_optional_target(input, "path", tool_name, &boundary)?;
            authorize_optional_target(input, "file", tool_name, &boundary)?;
            authorize_optional_target(input, "file_path", tool_name, &boundary)?;
            // Outline mode reads the file named by 'query', or by the first
            // term, when 'file' is omitted. Those fields are path inputs
            // there, not search patterns. 'pattern' is a serde alias for
            // 'query', so the raw JSON may carry the path under either name.
            if input.get("mode").and_then(Value::as_str) == Some("outline") {
                authorize_optional_target(input, "query", tool_name, &boundary)?;
                authorize_optional_target(input, "pattern", tool_name, &boundary)?;
                if let Some(Value::String(term)) = input
                    .get("terms")
                    .and_then(Value::as_array)
                    .and_then(|terms| terms.first())
                {
                    authorize_target(tool_name, &boundary, term)?;
                }
            }
        }
        "write" | "edit" | "multiedit" => {
            let target = required_string(input, "file_path", tool_name, &boundary)?;
            authorize_target(tool_name, &boundary, target)?;
        }
        "patch" => {
            let patch = required_string(input, "patch_text", tool_name, &boundary)?;
            for target in unified_patch_targets(patch) {
                authorize_target(tool_name, &boundary, &target)?;
            }
        }
        "apply_patch" => {
            let patch = required_string(input, "patch_text", tool_name, &boundary)?;
            for target in codex_patch_targets(patch) {
                authorize_target(tool_name, &boundary, &target)?;
            }
        }
        "bash" | "nix" => {
            return Err(ExecutionScopeError::SandboxUnavailable {
                tool: tool_name.to_string(),
                boundary,
            });
        }
        "swarm" => {
            // Spawning is the one way a bound session can reach the filesystem
            // without naming a file: a child rooted outside the boundary would
            // inherit no scope of its own and could read and write anywhere.
            authorize_optional_target(input, "working_dir", tool_name, &boundary)?;
        }
        "batch"
        | "bg"
        | "conversation_search"
        | "initiative"
        | "invalid"
        | "memory"
        | "session_search"
        | "skill_manage"
        | "todo"
        | "webfetch"
        | "websearch" => {}
        _ => {
            return Err(ExecutionScopeError::UnknownFilesystemEffects {
                tool: tool_name.to_string(),
                boundary,
            });
        }
    }
    Ok(Some(boundary))
}

fn authorize_optional_target(
    input: &Value,
    field: &str,
    tool: &str,
    boundary: &Path,
) -> Result<(), ExecutionScopeError> {
    match input.get(field) {
        None | Some(Value::Null) => Ok(()),
        Some(Value::String(target)) => authorize_target(tool, boundary, target),
        Some(_) => Err(ExecutionScopeError::InvalidToolInput {
            tool: tool.to_string(),
            boundary: boundary.to_path_buf(),
            reason: format!("field '{field}' must be a string when provided"),
        }),
    }
}

fn required_string<'a>(
    input: &'a Value,
    field: &str,
    tool: &str,
    boundary: &Path,
) -> Result<&'a str, ExecutionScopeError> {
    input
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| ExecutionScopeError::InvalidToolInput {
            tool: tool.to_string(),
            boundary: boundary.to_path_buf(),
            reason: format!("missing string field '{field}'"),
        })
}

fn authorize_target(tool: &str, boundary: &Path, target: &str) -> Result<(), ExecutionScopeError> {
    let target_path = Path::new(target);
    if target_path
        .components()
        .any(|component| component == Component::ParentDir)
    {
        return Err(ExecutionScopeError::InvalidToolInput {
            tool: tool.to_string(),
            boundary: boundary.to_path_buf(),
            reason: format!("target '{target}' contains a parent-directory component ('..')"),
        });
    }
    let candidate = if target_path.is_absolute() {
        target_path.to_path_buf()
    } else {
        boundary.join(target_path)
    };
    if contains_git_control_component(&candidate) {
        return Err(ExecutionScopeError::VcsMetadataDenied {
            tool: tool.to_string(),
            boundary: boundary.to_path_buf(),
            target: candidate,
        });
    }
    let normalized = canonicalize_new_path(&candidate).map_err(|error| {
        ExecutionScopeError::InvalidToolInput {
            tool: tool.to_string(),
            boundary: boundary.to_path_buf(),
            reason: format!("cannot resolve target '{}': {error}", candidate.display()),
        }
    })?;
    if contains_git_control_component(&normalized) {
        return Err(ExecutionScopeError::VcsMetadataDenied {
            tool: tool.to_string(),
            boundary: boundary.to_path_buf(),
            target: normalized,
        });
    }
    if normalized.starts_with(boundary) {
        Ok(())
    } else {
        Err(ExecutionScopeError::OutsideBoundary {
            tool: tool.to_string(),
            boundary: boundary.to_path_buf(),
            target: normalized,
        })
    }
}

fn contains_git_control_component(path: &Path) -> bool {
    path.components().any(|component| {
        component
            .as_os_str()
            .to_string_lossy()
            .eq_ignore_ascii_case(".git")
    })
}

fn canonicalize_new_path(path: &Path) -> std::io::Result<PathBuf> {
    let mut existing = path;
    let mut missing = Vec::<OsString>::new();
    loop {
        match std::fs::symlink_metadata(existing) {
            Ok(_) => break,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        let name = existing.file_name().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("no existing parent for {}", path.display()),
            )
        })?;
        missing.push(name.to_os_string());
        existing = existing.parent().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("no existing parent for {}", path.display()),
            )
        })?;
    }

    let mut canonical = std::fs::canonicalize(existing)?;
    for component in missing.into_iter().rev() {
        canonical.push(component);
    }
    Ok(canonical)
}

fn unified_patch_targets(patch: &str) -> Vec<String> {
    patch
        .lines()
        .filter_map(|line| {
            let path = line
                .strip_prefix("--- ")
                .or_else(|| line.strip_prefix("+++ "))?;
            let path = path.split('\t').next().unwrap_or(path);
            if path == "/dev/null" {
                return None;
            }
            Some(
                path.strip_prefix("a/")
                    .or_else(|| path.strip_prefix("b/"))
                    .unwrap_or(path)
                    .to_string(),
            )
        })
        .collect()
}

fn codex_patch_targets(patch: &str) -> Vec<String> {
    const PREFIXES: [&str; 4] = [
        "*** Add File: ",
        "*** Delete File: ",
        "*** Update File: ",
        "*** Move to: ",
    ];
    // Read each header exactly as `parse_apply_patch` does: trim the line end
    // before matching the prefix, then trim the path. Authorizing the raw
    // suffix instead lets a second space hide the target from this check while
    // the executor still trims it back to an absolute or `../` path.
    patch
        .lines()
        .filter_map(|line| {
            let line = line.trim_end();
            PREFIXES
                .iter()
                .find_map(|prefix| line.strip_prefix(prefix))
                .map(|path| path.trim().to_string())
        })
        .filter(|path| !path.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patch_headers_are_read_the_way_the_executor_reads_them() {
        // `parse_apply_patch` trims the extracted path, so padding after the
        // prefix must not hide an escaping target from authorization.
        let patch = concat!(
            "*** Begin Patch\n",
            "*** Add File:  /etc/passwd\n",
            "+owned\n",
            "*** Delete File: \t../sibling/notes.md\n",
            "*** Update File: inside.txt   \n",
            "*** End Patch\n",
        );

        assert_eq!(
            codex_patch_targets(patch),
            vec![
                "/etc/passwd".to_string(),
                "../sibling/notes.md".to_string(),
                "inside.txt".to_string(),
            ]
        );
    }

    #[cfg(unix)]
    #[test]
    fn scoped_write_rejects_symlink_and_parent_directory_escapes() {
        let temp = tempfile::TempDir::new().expect("temp dir");
        let inside = temp.path().join("inside");
        let outside = temp.path().join("outside");
        std::fs::create_dir_all(&inside).expect("inside dir");
        std::fs::create_dir_all(&outside).expect("outside dir");
        std::os::unix::fs::symlink(&outside, inside.join("link")).expect("symlink");
        std::os::unix::fs::symlink(outside.join("future.txt"), inside.join("dangling"))
            .expect("dangling symlink");

        let swarm_id = format!("scope-test-{}", temp.path().display());
        let session_id = format!("scope-session-{}", temp.path().display());
        let boundary = record_plan_execution_scope(&swarm_id, 1, &inside).expect("record scope");
        bind_session_execution_scope(&session_id, &swarm_id, 1).expect("bind scope");

        let symlink_error = authorize_tool_call(
            &session_id,
            "write",
            &serde_json::json!({"file_path": "link/new.txt"}),
        )
        .expect_err("symlink escape");
        let parent_error = authorize_tool_call(
            &session_id,
            "write",
            &serde_json::json!({"file_path": "../outside.txt"}),
        )
        .expect_err("parent escape");
        let dangling_error = authorize_tool_call(
            &session_id,
            "write",
            &serde_json::json!({"file_path": "dangling"}),
        )
        .expect_err("dangling symlink escape");
        let git_error = authorize_tool_call(
            &session_id,
            "write",
            &serde_json::json!({"file_path": ".git/index"}),
        )
        .expect_err("Git metadata write");
        let read_error = authorize_tool_call(
            &session_id,
            "read",
            &serde_json::json!({"file_path": outside.join("secret.txt")}),
        )
        .expect_err("outside read");
        clear_session_execution_scope(&session_id);

        assert!(matches!(
            &symlink_error,
            ExecutionScopeError::OutsideBoundary { .. }
        ));
        assert!(matches!(
            &parent_error,
            ExecutionScopeError::InvalidToolInput { .. }
        ));
        assert!(matches!(
            &dangling_error,
            ExecutionScopeError::InvalidToolInput { .. }
        ));
        assert!(matches!(
            &git_error,
            ExecutionScopeError::VcsMetadataDenied { .. }
        ));
        assert!(matches!(
            &read_error,
            ExecutionScopeError::OutsideBoundary { .. }
        ));
        assert!(
            symlink_error
                .to_string()
                .contains(&boundary.display().to_string())
        );
        assert!(
            parent_error
                .to_string()
                .contains(&boundary.display().to_string())
        );
        assert!(
            dangling_error
                .to_string()
                .contains(&boundary.display().to_string())
        );
        assert!(
            git_error
                .to_string()
                .contains(&boundary.display().to_string())
        );
        assert!(
            read_error
                .to_string()
                .contains(&boundary.display().to_string())
        );
    }

    #[test]
    fn scoped_spawn_cannot_root_a_child_session_outside_the_boundary() {
        // A child session inherits no scope of its own, so a spawn rooted
        // outside the boundary would hand the caller unrestricted file tools.
        let temp = tempfile::TempDir::new().expect("temp dir");
        let inside = temp.path().join("inside");
        let outside = temp.path().join("outside");
        std::fs::create_dir_all(&inside).expect("inside dir");
        std::fs::create_dir_all(&outside).expect("outside dir");

        let swarm_id = format!("spawn-scope-{}", temp.path().display());
        let session_id = format!("spawn-session-{}", temp.path().display());
        let boundary = record_plan_execution_scope(&swarm_id, 1, &inside).expect("record scope");
        bind_session_execution_scope(&session_id, &swarm_id, 1).expect("bind scope");

        let escape = authorize_tool_call(
            &session_id,
            "swarm",
            &serde_json::json!({"action": "spawn", "working_dir": outside.to_string_lossy()}),
        )
        .expect_err("spawn outside the boundary");
        let allowed = authorize_tool_call(
            &session_id,
            "swarm",
            &serde_json::json!({"action": "spawn", "working_dir": inside.to_string_lossy()}),
        );
        let unrooted = authorize_tool_call(
            &session_id,
            "swarm",
            &serde_json::json!({"action": "list"}),
        );
        clear_session_execution_scope(&session_id);

        assert!(matches!(
            &escape,
            ExecutionScopeError::OutsideBoundary { .. }
        ));
        assert!(
            escape
                .to_string()
                .contains(&boundary.display().to_string())
        );
        assert_eq!(allowed.expect("spawn inside the boundary"), Some(boundary.clone()));
        assert_eq!(unrooted.expect("swarm call without a directory"), Some(boundary));
    }

    #[test]
    fn scoped_agentgrep_outline_cannot_name_a_file_through_query_or_terms() {
        let temp = tempfile::TempDir::new().expect("temp dir");
        let inside = temp.path().join("inside");
        let outside = temp.path().join("outside");
        std::fs::create_dir_all(&inside).expect("inside dir");
        std::fs::create_dir_all(&outside).expect("outside dir");
        std::fs::write(outside.join("secret.rs"), "fn hidden() {}\n").expect("outside file");

        let swarm_id = format!("scope-outline-{}", temp.path().display());
        let session_id = format!("scope-outline-session-{}", temp.path().display());
        let boundary = record_plan_execution_scope(&swarm_id, 1, &inside).expect("record scope");
        bind_session_execution_scope(&session_id, &swarm_id, 1).expect("bind scope");

        // Outline mode falls back to 'query', then 'terms[0]', as the file to
        // read when 'file' is omitted. Both fallbacks are path inputs there.
        let query_escape = authorize_tool_call(
            &session_id,
            "agentgrep",
            &serde_json::json!({
                "mode": "outline",
                "query": outside.join("secret.rs"),
            }),
        )
        .expect_err("outline query escape");
        // 'pattern' deserializes into the same field as 'query'; the check
        // runs on raw JSON, so the alias must be covered explicitly.
        let pattern_escape = authorize_tool_call(
            &session_id,
            "agentgrep",
            &serde_json::json!({
                "mode": "outline",
                "pattern": outside.join("secret.rs"),
            }),
        )
        .expect_err("outline pattern alias escape");
        let term_escape = authorize_tool_call(
            &session_id,
            "agentgrep",
            &serde_json::json!({
                "mode": "outline",
                "terms": [outside.join("secret.rs")],
            }),
        )
        .expect_err("outline term escape");
        // A search-mode regex that is not a path must stay allowed.
        let search_query = authorize_tool_call(
            &session_id,
            "agentgrep",
            &serde_json::json!({"query": "fn hidden"}),
        );
        clear_session_execution_scope(&session_id);

        assert!(matches!(
            &query_escape,
            ExecutionScopeError::OutsideBoundary { .. }
        ));
        assert!(matches!(
            &pattern_escape,
            ExecutionScopeError::OutsideBoundary { .. }
        ));
        assert!(matches!(
            &term_escape,
            ExecutionScopeError::OutsideBoundary { .. }
        ));
        assert_eq!(
            search_query.expect("search query stays allowed"),
            Some(boundary)
        );
    }
}
