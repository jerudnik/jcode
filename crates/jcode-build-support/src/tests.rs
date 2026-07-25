use super::*;

// Delegate to the ONE crate-global publish lock (lib.rs) so home-mutating tests
// here serialize against the install-stage-hook tests in atomic_publish_tests
// and the resolver tests, all of which drive the shared publish path.
pub(crate) use super::publish_test_lock as test_env_lock;

pub(crate) fn with_temp_jcode_home<T>(f: impl FnOnce() -> T) -> T {
    let _guard = test_env_lock();
    let temp_home = tempfile::tempdir().expect("tempdir");
    let prev_home = std::env::var_os("JCODE_HOME");
    jcode_core::env::set_var("JCODE_HOME", temp_home.path());
    let result = f();
    if let Some(prev_home) = prev_home {
        jcode_core::env::set_var("JCODE_HOME", prev_home);
    } else {
        jcode_core::env::remove_var("JCODE_HOME");
    }
    result
}

/// Scoped env-var override for tests that must drive env-sensitive resolution
/// (nix-managed mode, repo discovery) without leaking into sibling tests. All
/// callers already hold the crate-global publish lock via
/// [`with_temp_jcode_home`].
pub(crate) struct EnvVarGuard {
    key: &'static str,
    prev: Option<std::ffi::OsString>,
}

impl EnvVarGuard {
    pub(crate) fn set(key: &'static str, value: impl AsRef<std::ffi::OsStr>) -> Self {
        let prev = std::env::var_os(key);
        jcode_core::env::set_var(key, value);
        Self { key, prev }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        match self.prev.take() {
            Some(prev) => jcode_core::env::set_var(self.key, prev),
            None => jcode_core::env::remove_var(self.key),
        }
    }
}

fn create_git_repo_fixture() -> tempfile::TempDir {
    let temp = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(temp.path().join(".git")).expect("create .git dir");
    std::fs::write(
        temp.path().join("Cargo.toml"),
        "[package]\nname = \"jcode\"\nversion = \"0.0.0\"\n",
    )
    .expect("write Cargo.toml");
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(temp.path())
        .output()
        .expect("git init");
    std::process::Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(temp.path())
        .output()
        .expect("git config email");
    std::process::Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(temp.path())
        .output()
        .expect("git config name");
    // Disable commit/tag signing so the fixture is hermetic against an ambient
    // global git config with commit.gpgsign=true (which would make `git commit`
    // fail when no GPG agent is available, e.g. on a CI runner).
    std::process::Command::new("git")
        .args(["config", "commit.gpgsign", "false"])
        .current_dir(temp.path())
        .output()
        .expect("git config commit.gpgsign");
    std::process::Command::new("git")
        .args(["config", "tag.gpgsign", "false"])
        .current_dir(temp.path())
        .output()
        .expect("git config tag.gpgsign");
    std::process::Command::new("git")
        .args(["add", "Cargo.toml"])
        .current_dir(temp.path())
        .output()
        .expect("git add");
    std::process::Command::new("git")
        .args(["commit", "-m", "init"])
        .current_dir(temp.path())
        .output()
        .expect("git commit");
    temp
}

fn source_state_fixture(short_hash: &str, fingerprint: &str) -> SourceState {
    SourceState {
        repo_scope: "repo-scope".to_string(),
        worktree_scope: "worktree-scope".to_string(),
        short_hash: short_hash.to_string(),
        full_hash: format!("{short_hash}-full"),
        dirty: true,
        fingerprint: fingerprint.to_string(),
        version_label: format!("{short_hash}-dirty-{}", &fingerprint[..12]),
        changed_paths: 1,
    }
}

#[test]
fn dev_binary_matches_source_only_on_exact_metadata() {
    let dir = tempfile::tempdir().expect("tempdir");
    let binary = dir.path().join("jcode");
    let source = source_state_fixture("abc123", "fingerprint-aaaaaaaa");

    // No sidecar metadata yet -> treated as stale.
    assert!(!dev_binary_matches_source(&binary, &source));

    write_dev_binary_source_metadata(&binary, &source).expect("write metadata");
    assert!(dev_binary_matches_source(&binary, &source));

    // A different source (newer commit) -> stale, triggers rebuild.
    let other = source_state_fixture("def456", "fingerprint-bbbbbbbb");
    assert!(!dev_binary_matches_source(&binary, &other));
}

#[test]
fn same_commit_dirty_source_states_project_distinct_runtime_identities_without_live_builds() {
    let first = source_state_fixture("abc1234", "111111111111aaaa");
    let second = source_state_fixture("abc1234", "222222222222bbbb");

    let first_projection = first.runtime_identity_projection("selfdev", "/tmp/jcode-a");
    let second_projection = second.runtime_identity_projection("selfdev", "/tmp/jcode-b");

    assert_eq!(first.short_hash, second.short_hash, "same commit fixture");
    assert_ne!(first.fingerprint, second.fingerprint);
    assert_ne!(first.version_label, second.version_label);
    assert_ne!(first_projection, second_projection);
    assert_eq!(
        first_projection.source_fingerprint.as_deref(),
        Some("111111111111aaaa")
    );
    assert_eq!(first_projection.source_dirty, Some(true));
    assert_eq!(first_projection.source_hash.as_deref(), Some("abc1234"));
    assert_eq!(first_projection.activation_channel, "selfdev");
}

#[test]
fn same_commit_dirty_sidecars_project_distinct_runtime_identities() {
    let dir = tempfile::tempdir().expect("tempdir");
    let first_binary = dir.path().join("jcode-first");
    let second_binary = dir.path().join("jcode-second");
    std::fs::write(&first_binary, "first").expect("write first binary");
    std::fs::write(&second_binary, "second").expect("write second binary");

    let first = source_state_fixture("abc1234", "111111111111aaaa");
    let second = source_state_fixture("abc1234", "222222222222bbbb");
    write_dev_binary_source_metadata(&first_binary, &first).expect("write first sidecar");
    write_dev_binary_source_metadata(&second_binary, &second).expect("write second sidecar");

    let first_projection = runtime_identity_projection_for_binary(&first_binary, "tui-client");
    let second_projection = runtime_identity_projection_for_binary(&second_binary, "tui-client");

    assert_eq!(first.short_hash, second.short_hash, "same commit fixture");
    assert_eq!(first_projection.version_label, "abc1234-dirty-111111111111");
    assert_eq!(
        second_projection.version_label,
        "abc1234-dirty-222222222222"
    );
    assert_eq!(
        first_projection.source_fingerprint.as_deref(),
        Some("111111111111aaaa")
    );
    assert_eq!(
        second_projection.source_fingerprint.as_deref(),
        Some("222222222222bbbb")
    );
    assert_eq!(first_projection.source_dirty, Some(true));
    assert_eq!(second_projection.source_dirty, Some(true));
    assert_ne!(first_projection, second_projection);
}

#[test]
fn test_binary_version_hash_mismatch_rejects_publish_candidate() {
    let source = source_state_fixture("newhash", "123456789abcffff");
    let report = BinaryVersionReport {
        version: Some("v0.0.0-dev (oldhash, dirty)".to_string()),
        git_hash: Some("oldhash".to_string()),
    };

    let error = validate_binary_version_matches_source_report(&report, Path::new("jcode"), &source)
        .expect_err("mismatched git hash should be rejected");

    assert!(
        error
            .to_string()
            .contains("binary was built from git hash oldhash")
    );
}

#[test]
fn test_dev_binary_source_metadata_mismatch_rejects_publish_candidate() {
    let temp = tempfile::tempdir().expect("tempdir");
    let binary = temp.path().join(binary_name());
    std::fs::write(&binary, b"fake").expect("write fake binary");
    let source = source_state_fixture("abc1234", "1111111111112222");
    let stale_source = source_state_fixture("abc1234", "999999999999aaaa");
    write_dev_binary_source_metadata(&binary, &stale_source).expect("write metadata");

    let error = validate_dev_binary_source_metadata(&binary, &source)
        .expect_err("mismatched source metadata should be rejected");

    assert!(error.to_string().contains("source metadata"));
    assert!(error.to_string().contains("999999999999aaaa"));
}

#[cfg(unix)]
#[test]
fn test_smoke_test_server_protocol_uses_fresh_connection_after_ping() {
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixListener;

    let temp = tempfile::tempdir().expect("tempdir");
    let socket_path = temp.path().join("smoke.sock");
    let listener = UnixListener::bind(&socket_path).expect("bind unix listener");

    let server = std::thread::spawn(move || {
        let (first, _) = listener.accept().expect("accept ping client");
        let mut first = BufReader::new(first);
        let mut line = String::new();
        first.read_line(&mut line).expect("read ping request");
        assert!(line.contains("\"type\":\"ping\""));
        first
            .get_mut()
            .write_all(b"{\"type\":\"pong\",\"id\":1}\n")
            .expect("write pong");

        let (second, _) = listener.accept().expect("accept subscribe client");
        let mut second = BufReader::new(second);
        line.clear();
        second.read_line(&mut line).expect("read subscribe request");
        assert!(line.contains("\"type\":\"subscribe\""));
        second
            .get_mut()
            .write_all(b"{\"type\":\"ack\",\"id\":2}\n")
            .expect("write subscribe ack");
    });

    smoke_test_server_protocol(&socket_path, "/tmp").expect("smoke test protocol succeeds");
    server.join().expect("server thread join");
}

#[test]
fn test_find_repo_in_ancestors_walks_upward() {
    let temp = tempfile::tempdir().expect("tempdir");
    let repo = temp.path().join("jcode-repo");
    let nested = repo.join("a").join("b").join("c");

    std::fs::create_dir_all(repo.join(".git")).expect("create .git");
    std::fs::write(
        repo.join("Cargo.toml"),
        "[package]\nname = \"jcode\"\nversion = \"0.0.0\"\n",
    )
    .expect("write Cargo.toml");
    std::fs::create_dir_all(&nested).expect("create nested dirs");

    let found = find_repo_in_ancestors(&nested).expect("repo should be found");
    assert_eq!(found, repo);
}

#[test]
fn launcher_dir_uses_sandbox_bin_when_jcode_home_is_set() {
    with_temp_jcode_home(|| {
        let launcher_dir = launcher_dir().expect("launcher dir");
        let expected = storage::jcode_dir().expect("jcode dir").join("bin");
        assert_eq!(launcher_dir, expected);
    });
}

#[test]
fn dirty_source_state_uses_fingerprint_in_version_label() {
    let repo = create_git_repo_fixture();
    std::fs::write(repo.path().join("notes.txt"), "dirty change\n").expect("write dirty file");

    let state = current_source_state(repo.path()).expect("source state");
    assert!(state.dirty);
    assert!(
        state
            .version_label
            .starts_with(&format!("{}-dirty-", state.short_hash))
    );
    assert!(state.version_label.len() > state.short_hash.len() + 7);
}

/// Publish a fake binary at the single fixed target and return its path.
fn publish_fixed(contents: &str) -> PathBuf {
    let path = current_fixed_binary_path().expect("fixed path");
    std::fs::create_dir_all(path.parent().expect("fixed dir")).expect("create fixed dir");
    std::fs::write(&path, contents).expect("write published binary");
    crate::platform_support::set_permissions_executable(&path).expect("chmod published binary");
    path
}

#[test]
fn build_manifest_default_is_empty_history() {
    let manifest = BuildManifest::default();
    assert!(manifest.history.is_empty());
}

#[test]
fn published_binary_sidecar_projects_exact_runtime_identity() {
    with_temp_jcode_home(|| {
        let source = source_state_fixture("fedcba9", "999999999999cccc");
        let published = publish_fixed("published binary");
        write_dev_binary_source_metadata(&published, &source).expect("write sidecar");

        let projection = runtime_identity_projection_for_binary(&published, "selfdev");

        assert_eq!(projection.version_label, "fedcba9-dirty-999999999999");
        assert_eq!(
            projection.source_fingerprint.as_deref(),
            Some("999999999999cccc")
        );
        assert_eq!(projection.source_dirty, Some(true));
        assert_eq!(projection.source_hash.as_deref(), Some("fedcba9"));
        assert_eq!(projection.source_full_hash.as_deref(), Some("fedcba9-full"));
        assert_eq!(projection.activation_channel, "selfdev");
        assert_eq!(
            projection.resolved_executable_payload,
            resolve_binary_payload(&published)
        );
    });
}

#[test]
fn client_and_shared_server_resolve_to_the_same_fixed_publish_target() {
    // F20c invariant: there is exactly ONE published binary, so the client and
    // the daemon can no longer diverge onto different channels. This is the
    // structural fix for the "new client, stale server" class of bugs.
    with_temp_jcode_home(|| {
        let published = publish_fixed("published binary");
        let canonical = std::fs::canonicalize(&published).expect("canonical published");

        for is_selfdev in [false, true] {
            let (client, client_label) =
                client_update_candidate(is_selfdev).expect("client candidate");
            let (server, server_label) =
                shared_server_update_candidate(is_selfdev).expect("server candidate");
            assert_eq!(client_label, "current-fixed");
            assert_eq!(server_label, "current-fixed");
            assert_eq!(
                std::fs::canonicalize(&client).expect("canonical client"),
                canonical,
                "client must resolve to the fixed publish target (is_selfdev={is_selfdev})"
            );
            assert_eq!(
                std::fs::canonicalize(&server).expect("canonical server"),
                canonical,
                "daemon must resolve to the fixed publish target (is_selfdev={is_selfdev})"
            );
        }
    });
}

#[test]
fn selfdev_falls_back_to_an_unpublished_repo_build() {
    // A self-dev session that has built but not yet published should still be
    // able to reload into its fresh repo build.
    with_temp_jcode_home(|| {
        let repo = tempfile::tempdir().expect("repo tempdir");
        let target = repo.path().join("target").join("selfdev");
        std::fs::create_dir_all(&target).expect("create target dir");
        let dev = target.join(binary_name());
        std::fs::write(&dev, "dev build").expect("write dev binary");
        let _repo_guard = EnvVarGuard::set("JCODE_REPO_DIR", repo.path());

        let (candidate, label) = client_update_candidate(true).expect("selfdev candidate");
        assert_eq!(label, "dev");
        assert_eq!(
            std::fs::canonicalize(candidate).expect("canonical candidate"),
            std::fs::canonicalize(&dev).expect("canonical dev")
        );
    });
}

#[test]
fn nix_managed_sessions_ignore_the_self_managed_publish_target() {
    // F20a/F20c: on a nix-managed install the package manager owns the binary,
    // so a stale self-dev publish must never shadow it for normal sessions.
    with_temp_jcode_home(|| {
        publish_fixed("self-managed publish");
        let launcher = launcher_binary_path().expect("launcher path");
        std::fs::create_dir_all(launcher.parent().expect("launcher dir"))
            .expect("create launcher dir");
        std::fs::write(&launcher, "nix profile binary").expect("write launcher");
        let _nix_guard = EnvVarGuard::set("JCODE_NIX_MANAGED", "1");

        let (candidate, label) = client_update_candidate(false).expect("nix candidate");
        assert_eq!(label, "nix-managed");
        assert_eq!(
            std::fs::canonicalize(candidate).expect("canonical candidate"),
            std::fs::canonicalize(&launcher).expect("canonical launcher")
        );

        // An explicit self-dev session still opts into the local publish.
        let (selfdev, selfdev_label) = client_update_candidate(true).expect("selfdev candidate");
        assert_eq!(selfdev_label, "current-fixed");
        assert_eq!(
            std::fs::canonicalize(selfdev).expect("canonical selfdev"),
            std::fs::canonicalize(current_fixed_binary_path().unwrap()).expect("canonical fixed")
        );
    });
}

#[test]
fn launcher_symlink_points_at_the_fixed_publish_target_and_stays_in_sandbox_home() {
    with_temp_jcode_home(|| {
        let published = publish_fixed("published binary");
        let launcher = update_launcher_symlink_to_current().expect("update launcher");

        let home = storage::jcode_dir().expect("jcode dir");
        assert!(
            launcher.starts_with(&home),
            "launcher {} must stay inside the sandbox home {}",
            launcher.display(),
            home.display()
        );
        assert_eq!(
            std::fs::canonicalize(&launcher).expect("canonical launcher"),
            std::fs::canonicalize(&published).expect("canonical published")
        );
    });
}
