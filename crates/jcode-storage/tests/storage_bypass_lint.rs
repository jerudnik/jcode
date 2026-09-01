use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const PATTERNS: &[(&str, &str)] = &[
    ("jcode-dir", concat!("jcode", "_dir(")),
    ("app-config-dir", concat!("app_config", "_dir(")),
    ("app-cache-dir", concat!("app_cache", "_dir(")),
    ("durable-state-dir", concat!("durable_state", "_dir(")),
    ("runtime-dir", concat!("runtime", "_dir(")),
    ("join-cache", concat!("join(\"", "cache\")")),
];

fn collect_rust_files(dir: &Path, files: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("read workspace source directory") {
        let path = entry.expect("workspace source entry").path();
        if path.is_dir() {
            if !matches!(
                path.file_name().and_then(|name| name.to_str()),
                Some(".git" | "target")
            ) {
                collect_rust_files(&path, files);
            }
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
            files.push(path);
        }
    }
}

fn workspace_root() -> PathBuf {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let Some(root) = manifest.parent().and_then(Path::parent) else {
        if std::env::var("JCODE_ALLOW_SKIP_STORAGE_LINT").as_deref() == Ok("1") {
            eprintln!("storage bypass lint skipped by JCODE_ALLOW_SKIP_STORAGE_LINT=1");
            return PathBuf::new();
        }
        panic!("storage bypass lint could not locate the workspace root");
    };
    root.to_path_buf()
}

fn observed_call_sites(root: &Path) -> BTreeMap<(String, String), usize> {
    let mut files = Vec::new();
    collect_rust_files(&root.join("crates"), &mut files);
    let this_test = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/storage_bypass_lint.rs");
    let mut observed = BTreeMap::new();

    for path in files {
        if path == this_test {
            continue;
        }
        let source = std::fs::read_to_string(&path).expect("read Rust source");
        let relative = path
            .strip_prefix(root)
            .expect("source path under workspace")
            .to_string_lossy()
            .replace('\\', "/");
        for (label, pattern) in PATTERNS {
            let count = source.match_indices(pattern).count();
            if count > 0 {
                observed.insert((relative.clone(), (*label).to_owned()), count);
            }
        }
    }

    observed
}

fn allowed_call_sites() -> BTreeMap<(String, String), usize> {
    let mut allowed = BTreeMap::new();
    for (line_number, line) in include_str!("storage_call_site_allowlist.txt")
        .lines()
        .enumerate()
    {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut fields = line.split('\t');
        let path = fields.next().expect("allowlist path");
        let pattern = fields.next().expect("allowlist pattern");
        let count: usize = fields
            .next()
            .expect("allowlist count")
            .parse()
            .unwrap_or_else(|_| panic!("invalid count on allowlist line {}", line_number + 1));
        assert!(
            fields.next().is_none(),
            "extra field on allowlist line {}",
            line_number + 1
        );
        assert!(
            PATTERNS.iter().any(|(label, _)| *label == pattern),
            "unknown pattern {pattern} on allowlist line {}",
            line_number + 1
        );
        assert!(
            allowed
                .insert((path.to_owned(), pattern.to_owned()), count)
                .is_none(),
            "duplicate allowlist entry for {path} {pattern}"
        );
    }
    allowed
}

#[test]
fn raw_storage_resolver_call_sites_match_the_shrinking_allowlist() {
    let root = workspace_root();
    if root.as_os_str().is_empty() {
        return;
    }
    let observed = observed_call_sites(&root);
    let allowed = allowed_call_sites();
    if observed != allowed {
        let unexpected: Vec<_> = observed
            .iter()
            .filter(|(key, count)| allowed.get(key) != Some(count))
            .collect();
        let stale: Vec<_> = allowed
            .iter()
            .filter(|(key, count)| observed.get(key) != Some(count))
            .collect();
        panic!(
            "raw storage resolver call sites changed\nunexpected or changed: {unexpected:#?}\nstale allowlist entries: {stale:#?}"
        );
    }
}
