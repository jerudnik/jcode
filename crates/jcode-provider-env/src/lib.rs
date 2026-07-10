use std::sync::{LazyLock, RwLock};

use jcode_provider_metadata::{
    OpenAiCompatibleProfile, ResolvedOpenAiCompatibleProfile, is_safe_env_file_name,
    is_safe_env_key_name,
};

/// Provenance-preserving API-key lookup description.
///
/// Catalog sources retain their ordered compatibility aliases. Primary-only
/// sources are reserved for genuinely non-catalog inputs such as explicit user
/// overrides and named profiles.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ApiKeyCredentialSource {
    primary_env: String,
    env_file: String,
    aliases: Vec<String>,
    catalog: bool,
}

impl ApiKeyCredentialSource {
    pub fn from_catalog_profile(profile: OpenAiCompatibleProfile) -> Self {
        Self {
            primary_env: profile.api_key_env.to_string(),
            env_file: profile.env_file.to_string(),
            aliases: profile
                .api_key_aliases
                .iter()
                .map(|alias| (*alias).to_string())
                .collect(),
            catalog: true,
        }
    }

    pub fn from_resolved_catalog_profile(profile: &ResolvedOpenAiCompatibleProfile) -> Self {
        Self {
            primary_env: profile.api_key_env.clone(),
            env_file: profile.env_file.clone(),
            aliases: profile.api_key_aliases.clone(),
            catalog: true,
        }
    }

    pub fn primary_only(env_key: impl Into<String>, env_file: impl Into<String>) -> Self {
        Self {
            primary_env: env_key.into(),
            env_file: env_file.into(),
            aliases: Vec::new(),
            catalog: false,
        }
    }

    pub fn primary_env(&self) -> &str {
        &self.primary_env
    }

    pub fn env_file(&self) -> &str {
        &self.env_file
    }

    pub fn candidate_env_keys(&self) -> impl Iterator<Item = &str> {
        std::iter::once(self.primary_env()).chain(self.aliases.iter().map(String::as_str))
    }

    pub fn is_catalog(&self) -> bool {
        self.catalog
    }
}

/// Fallback resolvers consulted by [`load_api_key`] after the
/// environment and config-file lookups fail. Higher-level crates register
/// resolvers at startup so this leaf crate does not need to depend on auth.
type ApiKeyFallbackResolver = fn(&str) -> Option<String>;

static API_KEY_FALLBACK_RESOLVERS: LazyLock<RwLock<Vec<ApiKeyFallbackResolver>>> =
    LazyLock::new(|| RwLock::new(Vec::new()));

/// Register a fallback API-key resolver consulted when env/config lookups miss.
pub fn register_api_key_fallback_resolver(resolver: ApiKeyFallbackResolver) {
    API_KEY_FALLBACK_RESOLVERS
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .push(resolver);
}

fn resolve_api_key_fallback(env_key: &str) -> Option<String> {
    let resolvers = API_KEY_FALLBACK_RESOLVERS
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    for resolver in resolvers.iter() {
        if let Some(key) = resolver(env_key) {
            return Some(key);
        }
    }
    None
}

/// Characters that editors, terminals, and `cat` render invisibly but that
/// corrupt a credential when embedded in it. Rust's [`str::trim`] only removes
/// ASCII whitespace, so these survive a plain trim and silently break auth
/// (see GitHub issue #376). [`char::is_whitespace`] covers Unicode White_Space
/// (NBSP U+00A0, the en/em spaces U+2002-U+200A, line/paragraph separators,
/// etc.); the explicit cases below are zero-width characters and the BOM, which
/// are not classified as whitespace.
fn is_invisible_boundary_char(c: char) -> bool {
    c.is_whitespace()
        || matches!(
            c,
            '\u{200B}' // zero-width space
                | '\u{200C}' // zero-width non-joiner
                | '\u{200D}' // zero-width joiner
                | '\u{2060}' // word joiner
                | '\u{FEFF}' // BOM / zero-width no-break space
        )
}

/// Strip leading/trailing invisible (Unicode whitespace and zero-width)
/// characters and one optional layer of surrounding quotes from a loaded
/// secret or config value.
///
/// Exposed so other credential loaders (e.g. the Cursor key reader) can apply
/// the same sanitizing as [`load_api_key`].
pub fn sanitize_secret_value(raw: &str) -> &str {
    raw.trim_matches(is_invisible_boundary_char)
        .trim_matches('"')
        .trim_matches('\'')
        .trim_matches(is_invisible_boundary_char)
}

/// Sanitize a loaded value and surface a warning when Unicode invisible
/// characters were present, so the failure mode in issue #376 is no longer
/// silent. Returns `None` for values that are empty after sanitizing.
fn clean_loaded_value(raw: &str, env_key: &str) -> Option<String> {
    let cleaned = sanitize_secret_value(raw);
    if cleaned.is_empty() {
        return None;
    }
    // A plain ASCII trim is what we previously did; if it leaves a different
    // result than the Unicode-aware sanitize, hidden characters were stripped.
    let ascii_only = raw.trim().trim_matches('"').trim_matches('\'').trim();
    if ascii_only != cleaned {
        jcode_logging::warn(&format!(
            "Stripped Unicode invisible or non-ASCII whitespace characters from '{}' while loading credentials; verify the value contains no hidden characters",
            env_key
        ));
    }
    Some(cleaned.to_string())
}

/// Load an API key while preserving the source's catalog provenance.
///
/// Lookup order is primary process env, primary env-file entry, then each
/// catalog alias in declaration order with process env before env-file entry.
pub fn load_api_key(source: &ApiKeyCredentialSource) -> Option<String> {
    let env_key = source.primary_env();
    let file_name = source.env_file();
    if source
        .candidate_env_keys()
        .any(|candidate| !is_safe_env_key_name(candidate))
    {
        jcode_logging::warn(&format!(
            "Ignoring invalid API key variable name in source for '{}' while loading credentials",
            env_key
        ));
        return None;
    }
    if !is_safe_env_file_name(file_name) {
        jcode_logging::warn(&format!(
            "Ignoring invalid env file name '{}' while loading credentials",
            file_name
        ));
        return None;
    }

    let content = jcode_storage::app_config_dir().ok().and_then(|config_dir| {
        let config_path = config_dir.join(file_name);
        jcode_storage::harden_secret_file_permissions(&config_path);
        std::fs::read_to_string(config_path).ok()
    });

    for candidate in source.candidate_env_keys() {
        if let Ok(key) = std::env::var(candidate)
            && let Some(key) = clean_loaded_value(&key, candidate)
        {
            return Some(key);
        }

        if let Some(content) = content.as_deref() {
            let prefix = format!("{}=", candidate);
            for line in content.lines() {
                if let Some(key) = line.strip_prefix(&prefix)
                    && let Some(key) = clean_loaded_value(key, candidate)
                {
                    return Some(key);
                }
            }
        }
    }

    if let Some(key) = resolve_api_key_fallback(env_key) {
        return Some(key);
    }

    None
}

#[cfg(test)]
fn load_api_key_from_env_or_config(env_key: &str, file_name: &str) -> Option<String> {
    load_api_key(&ApiKeyCredentialSource::primary_only(env_key, file_name))
}

pub fn load_env_value_from_env_or_config(env_key: &str, file_name: &str) -> Option<String> {
    if !is_safe_env_key_name(env_key) {
        jcode_logging::warn(&format!(
            "Ignoring invalid variable name '{}' while loading config value",
            env_key
        ));
        return None;
    }
    if !is_safe_env_file_name(file_name) {
        jcode_logging::warn(&format!(
            "Ignoring invalid env file name '{}' while loading config value",
            file_name
        ));
        return None;
    }

    if let Ok(value) = std::env::var(env_key)
        && let Some(value) = clean_loaded_value(&value, env_key)
    {
        return Some(value);
    }

    load_env_value_from_config_file(env_key, file_name)
}

/// Load a value only from the saved env file under the jcode config dir,
/// ignoring the process environment.
///
/// [`load_env_value_from_env_or_config`] prefers the process env var, which is
/// correct for ambient configuration but wrong right after an explicit
/// `/login`: a stale env var inherited by a long-lived server process would
/// silently win over the credential the user just saved (issue #453). This
/// reader lets the auth-change path resolve what the file actually contains.
pub fn load_env_value_from_config_file(env_key: &str, file_name: &str) -> Option<String> {
    if !is_safe_env_key_name(env_key) {
        jcode_logging::warn(&format!(
            "Ignoring invalid variable name '{}' while loading config value",
            env_key
        ));
        return None;
    }
    if !is_safe_env_file_name(file_name) {
        jcode_logging::warn(&format!(
            "Ignoring invalid env file name '{}' while loading config value",
            file_name
        ));
        return None;
    }

    let config_path = jcode_storage::app_config_dir().ok()?.join(file_name);
    jcode_storage::harden_secret_file_permissions(&config_path);
    let content = std::fs::read_to_string(config_path).ok()?;
    let prefix = format!("{}=", env_key);

    for line in content.lines() {
        if let Some(value) = line.strip_prefix(&prefix)
            && let Some(value) = clean_loaded_value(value, env_key)
        {
            return Some(value);
        }
    }

    None
}

pub fn save_env_value_to_env_file(
    env_key: &str,
    file_name: &str,
    value: Option<&str>,
) -> anyhow::Result<()> {
    if !is_safe_env_key_name(env_key) {
        anyhow::bail!("Invalid variable name: {}", env_key);
    }
    if !is_safe_env_file_name(file_name) {
        anyhow::bail!("Invalid env file name: {}", file_name);
    }

    let config_dir = jcode_storage::app_config_dir()?;
    let file_path = config_dir.join(file_name);
    jcode_storage::upsert_env_file_value(&file_path, env_key, value)?;

    if let Some(value) = value {
        jcode_core::env::set_var(env_key, value);
    } else {
        jcode_core::env::remove_var(env_key);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use std::sync::{Mutex, MutexGuard};

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn rust_sources(root: &std::path::Path, files: &mut Vec<std::path::PathBuf>) {
        for entry in std::fs::read_dir(root).expect("read source directory") {
            let path = entry.expect("source entry").path();
            if path.is_dir() {
                if !matches!(
                    path.file_name().and_then(|name| name.to_str()),
                    Some("target" | ".git")
                ) {
                    rust_sources(&path, files);
                }
            } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
                files.push(path);
            }
        }
    }

    #[test]
    fn catalog_consumers_cannot_reach_or_reconstruct_the_bare_pair_loader() {
        let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(std::path::Path::parent)
            .expect("workspace root");
        let provider_env_lib = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs");
        let mut files = Vec::new();
        rust_sources(workspace, &mut files);

        for path in files {
            if path == provider_env_lib {
                continue;
            }
            let source = std::fs::read_to_string(&path).expect("read Rust source");
            assert!(
                !source.contains("load_api_key_from_env_or_config"),
                "bare pair loader escaped provider-env: {}",
                path.display()
            );
            for suffix in source
                .split("ApiKeyCredentialSource::primary_only(")
                .skip(1)
            {
                let arguments = &suffix[..suffix.find(')').unwrap_or(suffix.len())];
                assert!(
                    !arguments.contains(".api_key_env") && !arguments.contains(".env_file"),
                    "catalog profile fields were laundered into a primary-only source: {}",
                    path.display()
                );
            }
        }
    }

    struct EnvGuard {
        _lock: MutexGuard<'static, ()>,
        saved: Vec<(&'static str, Option<OsString>)>,
    }

    impl EnvGuard {
        fn new(keys: &[&'static str]) -> Self {
            let lock = ENV_LOCK
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let saved = keys
                .iter()
                .map(|key| (*key, std::env::var_os(key)))
                .collect::<Vec<_>>();
            for key in keys {
                jcode_core::env::remove_var(key);
            }
            Self { _lock: lock, saved }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (key, value) in self.saved.drain(..) {
                match value {
                    Some(value) => jcode_core::env::set_var(key, value),
                    None => jcode_core::env::remove_var(key),
                }
            }
        }
    }

    #[test]
    fn loads_api_key_from_env_before_config_file() {
        let temp = tempfile::tempdir().expect("tempdir");
        let _guard = EnvGuard::new(&["JCODE_HOME", "JCODE_PROVIDER_ENV_TEST_KEY"]);
        jcode_core::env::set_var("JCODE_HOME", temp.path());

        save_env_value_to_env_file(
            "JCODE_PROVIDER_ENV_TEST_KEY",
            "provider-env-test.env",
            Some("file-key"),
        )
        .expect("save file key");
        jcode_core::env::set_var("JCODE_PROVIDER_ENV_TEST_KEY", "env-key");

        assert_eq!(
            load_api_key_from_env_or_config("JCODE_PROVIDER_ENV_TEST_KEY", "provider-env-test.env")
                .as_deref(),
            Some("env-key")
        );
    }

    #[test]
    fn loads_and_removes_values_from_sandboxed_config_file() {
        let temp = tempfile::tempdir().expect("tempdir");
        let _guard = EnvGuard::new(&["JCODE_HOME", "JCODE_PROVIDER_ENV_TEST_VALUE"]);
        jcode_core::env::set_var("JCODE_HOME", temp.path());

        save_env_value_to_env_file(
            "JCODE_PROVIDER_ENV_TEST_VALUE",
            "provider-env-test.env",
            Some("file-value"),
        )
        .expect("save file value");

        jcode_core::env::remove_var("JCODE_PROVIDER_ENV_TEST_VALUE");
        assert_eq!(
            load_env_value_from_env_or_config(
                "JCODE_PROVIDER_ENV_TEST_VALUE",
                "provider-env-test.env"
            )
            .as_deref(),
            Some("file-value")
        );

        save_env_value_to_env_file(
            "JCODE_PROVIDER_ENV_TEST_VALUE",
            "provider-env-test.env",
            None,
        )
        .expect("remove file value");
        assert_eq!(
            load_env_value_from_env_or_config(
                "JCODE_PROVIDER_ENV_TEST_VALUE",
                "provider-env-test.env"
            ),
            None
        );
    }

    #[test]
    fn catalog_source_loads_legacy_zai_alias_from_file() {
        let temp = tempfile::tempdir().expect("tempdir");
        let _guard = EnvGuard::new(&["JCODE_HOME", "ZHIPU_API_KEY", "ZAI_API_KEY"]);
        jcode_core::env::set_var("JCODE_HOME", temp.path());

        save_env_value_to_env_file("ZAI_API_KEY", "zai.env", Some("legacy-zai-key"))
            .expect("save legacy key");
        jcode_core::env::remove_var("ZAI_API_KEY");

        assert_eq!(
            load_api_key(&ApiKeyCredentialSource::from_catalog_profile(
                jcode_provider_metadata::ZAI_PROFILE,
            ))
            .as_deref(),
            Some("legacy-zai-key")
        );
    }

    #[test]
    fn catalog_source_preserves_primary_then_ordered_alias_precedence() {
        let temp = tempfile::tempdir().expect("tempdir");
        let _guard = EnvGuard::new(&[
            "JCODE_HOME",
            "TEST_PRIMARY_API_KEY",
            "TEST_ALIAS_ONE_API_KEY",
            "TEST_ALIAS_TWO_API_KEY",
        ]);
        jcode_core::env::set_var("JCODE_HOME", temp.path());
        let source = ApiKeyCredentialSource {
            primary_env: "TEST_PRIMARY_API_KEY".to_string(),
            env_file: "test-aliases.env".to_string(),
            aliases: vec![
                "TEST_ALIAS_ONE_API_KEY".to_string(),
                "TEST_ALIAS_TWO_API_KEY".to_string(),
            ],
            catalog: true,
        };

        save_env_value_to_env_file(
            "TEST_PRIMARY_API_KEY",
            "test-aliases.env",
            Some("primary-file"),
        )
        .expect("primary file");
        jcode_core::env::set_var("TEST_PRIMARY_API_KEY", "primary-env");
        jcode_core::env::set_var("TEST_ALIAS_ONE_API_KEY", "alias-one-env");
        assert_eq!(load_api_key(&source).as_deref(), Some("primary-env"));

        jcode_core::env::remove_var("TEST_PRIMARY_API_KEY");
        assert_eq!(load_api_key(&source).as_deref(), Some("primary-file"));

        save_env_value_to_env_file("TEST_PRIMARY_API_KEY", "test-aliases.env", None)
            .expect("remove primary file");
        save_env_value_to_env_file(
            "TEST_ALIAS_ONE_API_KEY",
            "test-aliases.env",
            Some("alias-one-file"),
        )
        .expect("alias one file");
        jcode_core::env::remove_var("TEST_ALIAS_ONE_API_KEY");
        jcode_core::env::set_var("TEST_ALIAS_TWO_API_KEY", "alias-two-env");
        assert_eq!(load_api_key(&source).as_deref(), Some("alias-one-file"));

        jcode_core::env::set_var("TEST_ALIAS_ONE_API_KEY", "  \u{200B}alias-one-env\u{FEFF} ");
        assert_eq!(load_api_key(&source).as_deref(), Some("alias-one-env"));
    }

    #[test]
    fn primary_only_source_does_not_probe_catalog_aliases() {
        let temp = tempfile::tempdir().expect("tempdir");
        let _guard = EnvGuard::new(&["JCODE_HOME", "ZHIPU_API_KEY", "ZAI_API_KEY"]);
        jcode_core::env::set_var("JCODE_HOME", temp.path());
        jcode_core::env::set_var("ZAI_API_KEY", "legacy-zai-key");

        let source = ApiKeyCredentialSource::primary_only("ZHIPU_API_KEY", "zai.env");
        assert_eq!(load_api_key(&source), None);
        assert!(!source.is_catalog());
    }

    #[test]
    fn minimax_and_openai_catalog_credentials_are_isolated() {
        let temp = tempfile::tempdir().expect("tempdir");
        let _guard = EnvGuard::new(&["JCODE_HOME", "MINIMAX_API_KEY", "OPENAI_API_KEY"]);
        jcode_core::env::set_var("JCODE_HOME", temp.path());
        jcode_core::env::set_var("OPENAI_API_KEY", "openai-only");

        let minimax =
            ApiKeyCredentialSource::from_catalog_profile(jcode_provider_metadata::MINIMAX_PROFILE);
        let openai = ApiKeyCredentialSource::from_catalog_profile(
            jcode_provider_metadata::OPENAI_NATIVE_OPENAI_COMPAT_PROFILE,
        );
        assert_eq!(load_api_key(&minimax), None);
        assert_eq!(load_api_key(&openai).as_deref(), Some("openai-only"));
    }

    #[test]
    fn sanitize_strips_unicode_invisible_characters() {
        // Zero-width space, BOM, NBSP, en space around the value.
        assert_eq!(
            sanitize_secret_value("\u{200B}sk-key123\u{FEFF}"),
            "sk-key123"
        );
        assert_eq!(sanitize_secret_value("\u{00A0}sk-key\u{2002}"), "sk-key");
        // Quotes plus invisible padding both stripped.
        assert_eq!(
            sanitize_secret_value("\u{FEFF}\"sk-quoted\"\u{200B}"),
            "sk-quoted"
        );
        // Interior characters are preserved.
        assert_eq!(
            sanitize_secret_value("sk-mid\u{200B}dle"),
            "sk-mid\u{200B}dle"
        );
        // Empty after sanitize.
        assert_eq!(sanitize_secret_value("\u{200B}\u{FEFF}"), "");
    }

    #[test]
    fn loads_api_key_with_zero_width_space_from_config_file() {
        let temp = tempfile::tempdir().expect("tempdir");
        let _guard = EnvGuard::new(&["JCODE_HOME", "JCODE_PROVIDER_FOO_API_KEY"]);
        jcode_core::env::set_var("JCODE_HOME", temp.path());

        // Write an env file with a U+200B zero-width space prefixed onto the key,
        // mirroring issue #376's reproduction.
        let config_dir = jcode_storage::app_config_dir().expect("config dir");
        std::fs::create_dir_all(&config_dir).expect("create config dir");
        std::fs::write(
            config_dir.join("provider-foo.env"),
            "JCODE_PROVIDER_FOO_API_KEY=\u{200B}sk-mykey123\n",
        )
        .expect("write env file");

        assert_eq!(
            load_api_key_from_env_or_config("JCODE_PROVIDER_FOO_API_KEY", "provider-foo.env")
                .as_deref(),
            Some("sk-mykey123")
        );
    }

    #[test]
    fn loads_api_key_with_invisible_chars_from_env_var() {
        let temp = tempfile::tempdir().expect("tempdir");
        let _guard = EnvGuard::new(&["JCODE_HOME", "JCODE_PROVIDER_BAR_API_KEY"]);
        jcode_core::env::set_var("JCODE_HOME", temp.path());
        // NBSP + BOM padding around the env-provided key.
        jcode_core::env::set_var("JCODE_PROVIDER_BAR_API_KEY", "\u{00A0}sk-env-key\u{FEFF}");

        assert_eq!(
            load_api_key_from_env_or_config("JCODE_PROVIDER_BAR_API_KEY", "provider-bar.env")
                .as_deref(),
            Some("sk-env-key")
        );
    }
}
