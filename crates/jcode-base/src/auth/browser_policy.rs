use std::path::Path;

pub fn browser_suppressed(cli_no_browser: bool) -> bool {
    cli_no_browser
        || env_truthy("NO_BROWSER")
        || env_truthy("JCODE_NO_BROWSER")
        || running_in_test_harness()
}

/// True when the current process is a Rust test binary (`cargo test` or
/// `cargo nextest`). Cargo places executable test artifacts directly in a
/// `deps` directory even when `CARGO_TARGET_DIR` has a custom name.
///
/// This keeps OAuth and file openers from escaping onto the developer's
/// desktop. Set `JCODE_ALLOW_BROWSER_IN_TESTS=1` only for an intentionally
/// interactive live test.
pub fn running_in_test_harness() -> bool {
    static IN_TEST_HARNESS: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *IN_TEST_HARNESS.get_or_init(|| {
        if env_truthy("JCODE_ALLOW_BROWSER_IN_TESTS") {
            return false;
        }
        match std::env::current_exe() {
            Ok(exe) => is_cargo_test_binary_path(&exe),
            Err(_) => false,
        }
    })
}

pub(super) fn is_cargo_test_binary_path(exe: &Path) -> bool {
    let normalized = exe.to_string_lossy().replace('\\', "/");
    let mut parts = normalized.rsplit('/');
    let Some(file_name) = parts.next() else {
        return false;
    };
    let file_name = file_name.trim_end_matches(".exe");
    if parts.next() != Some("deps") {
        return false;
    }
    file_name.rsplit_once('-').is_some_and(|(stem, hash)| {
        !stem.is_empty() && hash.len() >= 16 && hash.chars().all(|ch| ch.is_ascii_hexdigit())
    })
}

pub(super) fn env_truthy(key: &str) -> bool {
    match std::env::var(key) {
        Ok(value) => {
            let trimmed = value.trim();
            !trimmed.is_empty() && trimmed != "0" && !trimmed.eq_ignore_ascii_case("false")
        }
        Err(_) => false,
    }
}
