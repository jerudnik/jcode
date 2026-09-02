use std::path::{Path, PathBuf};

const DIRECT_READ_PATTERNS: &[&str] = &["member.status"];

fn collect_rust_files(dir: &Path, files: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("read server source directory") {
        let path = entry.expect("server source entry").path();
        if path.is_dir() {
            if path.file_name().and_then(|name| name.to_str()) != Some("tests")
                && !path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.ends_with("_tests"))
            {
                collect_rust_files(&path, files);
            }
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("rs")
            && !path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name == "tests.rs" || name.ends_with("_tests.rs"))
        {
            files.push(path);
        }
    }
}

fn prohibited_reads(source: &str) -> Vec<(usize, &'static str)> {
    source
        .lines()
        .enumerate()
        .flat_map(|(line, text)| {
            DIRECT_READ_PATTERNS
                .iter()
                .filter(move |pattern| text.contains(**pattern) && !text.contains(".status ="))
                .map(move |pattern| (line + 1, *pattern))
        })
        .collect()
}

#[test]
fn server_swarm_member_compatibility_mirror_has_no_direct_output_reads() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let server = manifest.join("src/server");
    let mut files = Vec::new();
    collect_rust_files(&server, &mut files);

    let mut violations = Vec::new();
    for path in files {
        let source = std::fs::read_to_string(&path).expect("read server Rust source");
        for (line, pattern) in prohibited_reads(&source) {
            // `AwaitedMemberStatus` is a wire result, not the server-side
            // `SwarmMember` mirror guarded by this lint.
            if path.ends_with("comm_await.rs") && pattern == "member.status" {
                continue;
            }
            violations.push(format!("{}:{line}: {pattern}", path.display()));
        }
    }

    assert!(
        violations.is_empty(),
        "direct server SwarmMember.status output reads are forbidden; use lifecycle() or lifecycle_status():\n{}",
        violations.join("\n")
    );
}

#[test]
fn lint_fixture_rejects_a_direct_member_status_output_read() {
    let fixture = r#"let output = member.status.clone();"#;
    assert_eq!(prohibited_reads(fixture), vec![(1, "member.status")]);
}
