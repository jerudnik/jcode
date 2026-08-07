use std::ffi::OsStr;

/// Mutate the process environment for jcode runtime configuration.
///
/// Rust 2024 makes environment mutation unsafe because it can race with
/// concurrent environment access in foreign code. jcode intentionally mutates
/// process-local env vars to coordinate provider/runtime bootstrap before or
/// during task execution. We centralize that unsafety here so call sites remain
/// auditable.
pub fn set_var<K, V>(key: K, value: V)
where
    K: AsRef<OsStr>,
    V: AsRef<OsStr>,
{
    // SAFETY: jcode treats these mutations as process-global configuration.
    // They are a pre-existing design choice used throughout startup, auth,
    // provider bootstrap, tests, and self-dev flows. Centralizing the unsafe
    // operation here makes the Rust 2024 requirement explicit without
    // scattering unsafe blocks across hundreds of call sites.
    unsafe {
        std::env::set_var(key, value);
    }
}

/// Parse a boolean environment flag with the canonical jcode truthiness rule:
/// set, non-empty (after trimming), and not `"0"`/`"false"` (case-insensitive)
/// means enabled. Unset or empty means disabled.
///
/// This is THE flag parser. Do not hand-roll the truthiness check at call
/// sites; a dozen divergent copies of this closure (some forgetting `trim`,
/// some inverting the empty-string default) is exactly the drift this helper
/// exists to prevent.
pub fn flag_enabled(name: &str) -> bool {
    flag_enabled_or(name, false)
}

/// Like [`flag_enabled`], but unset/empty resolves to `default` instead of
/// `false`. Use for opt-out flags that default on (e.g. `JCODE_SHOW_DIFFS`).
pub fn flag_enabled_or(name: &str, default: bool) -> bool {
    match std::env::var(name) {
        Ok(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                default
            } else {
                trimmed != "0" && !trimmed.eq_ignore_ascii_case("false")
            }
        }
        Err(_) => default,
    }
}

/// Remove a process environment variable used by jcode runtime configuration.
pub fn remove_var<K>(key: K)
where
    K: AsRef<OsStr>,
{
    // SAFETY: see `set_var` above; this is the corresponding centralized
    // removal operation for the same process-global configuration surface.
    unsafe {
        std::env::remove_var(key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flag_truthiness_rule() {
        let key = "JCODE_TEST_FLAG_TRUTHINESS_RULE";
        // Unset: default wins.
        remove_var(key);
        assert!(!flag_enabled(key));
        assert!(flag_enabled_or(key, true));

        for (value, expected) in [
            ("1", true),
            ("true", true),
            ("yes", true),
            ("0", false),
            ("false", false),
            ("FALSE", false),
            (" 0 ", false), // trimmed
            ("", false),    // empty = unset
            ("   ", false), // whitespace-only = unset
        ] {
            set_var(key, value);
            assert_eq!(flag_enabled(key), expected, "value {value:?}");
        }

        // Empty string also falls back to the default, like unset.
        set_var(key, "");
        assert!(flag_enabled_or(key, true));
        remove_var(key);
    }
}
