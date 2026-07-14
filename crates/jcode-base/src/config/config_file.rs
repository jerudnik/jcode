use super::*;
use crate::storage::jcode_dir;
use std::fs::{File, OpenOptions};
use std::os::fd::AsRawFd;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

static WARNED_PINNED_CONFIG_SAVE_KEYS: OnceLock<Mutex<BTreeSet<String>>> = OnceLock::new();
static LOGGED_CONFIG_LAYER_SUMMARY: WarnOnce = WarnOnce::new();

impl Config {
    /// Get the config file path
    pub fn path() -> Option<PathBuf> {
        jcode_dir().ok().map(|d| d.join("config.toml"))
    }

    /// Get the declarative policy config path.
    pub fn policy_path() -> Option<PathBuf> {
        jcode_dir().ok().map(|d| d.join("config.nix.toml"))
    }

    fn lock_path() -> Option<PathBuf> {
        jcode_dir().ok().map(|d| d.join("config.toml.lock"))
    }

    /// Load config from file, with environment variable overrides
    pub fn load() -> Self {
        let mut config = Self::load_from_file().unwrap_or_default();
        config.apply_env_overrides();
        config.normalize_memory_embedding_backend();
        config
    }

    /// Load config from file, with environment variable overrides.
    ///
    /// Unlike [`Self::load`], this returns TOML/read errors to callers that need
    /// to distinguish a malformed config from an absent config.
    pub fn load_strict() -> anyhow::Result<Self> {
        let mut config = Self::load_from_file_strict()?.unwrap_or_default();
        config.apply_env_overrides();
        config.normalize_memory_embedding_backend();
        Ok(config)
    }

    /// Load config from file only (no env overrides)
    fn load_from_file() -> Option<Self> {
        match Self::load_from_file_strict() {
            Ok(config) => config,
            Err(e) => {
                crate::logging::error(&format!("Failed to parse config file: {}", e));
                None
            }
        }
    }

    /// Load config from file only (no env overrides), preserving parse/read errors.
    fn load_from_file_strict() -> anyhow::Result<Option<Self>> {
        let Some(path) = Self::path() else {
            return Ok(None);
        };
        let policy_path = Self::policy_path();
        let policy_exists = policy_path.as_ref().is_some_and(|path| path.exists());
        if !path.exists() && !policy_exists {
            return Ok(None);
        }

        if !policy_exists {
            let content = std::fs::read_to_string(&path).map_err(|e| {
                anyhow::anyhow!("Failed to read config file {}: {}", path.display(), e)
            })?;

            // Parse permissively (version-skew tolerance by design) while collecting
            // any unknown/ignored key paths so we can surface them for observability.
            // Warnings are emitted ONLY after a successful deserialization: a
            // malformed TOML must return the original parse error with no partial
            // warning spam.
            let (mut config, unknown_keys) = Self::parse_toml_collecting_unknown(&content)
                .map_err(|e| {
                    anyhow::anyhow!("Failed to parse config file {}: {}", path.display(), e)
                })?;

            for key in &unknown_keys {
                crate::logging::warn(&format!("Unknown config key '{}' ignored", key));
            }

            config.display.apply_legacy_compat();
            return Ok(Some(config));
        }

        let policy_path = policy_path.expect("policy_exists implies policy path");
        let policy_content = std::fs::read_to_string(&policy_path).map_err(|e| {
            anyhow::anyhow!(
                "Failed to read policy config file {}: {}",
                policy_path.display(),
                e
            )
        })?;
        let policy_value = Self::parse_toml_value(&policy_content, &policy_path)?;

        let durable_value = if path.exists() {
            let content = std::fs::read_to_string(&path).map_err(|e| {
                anyhow::anyhow!("Failed to read config file {}: {}", path.display(), e)
            })?;
            Some(Self::parse_toml_value(&content, &path)?)
        } else {
            None
        };

        let mut effective_value = durable_value
            .clone()
            .unwrap_or_else(|| toml::Value::Table(toml::map::Map::new()));
        Self::merge_policy_over_durable(&mut effective_value, &policy_value, &mut Vec::new());

        let effective_content = toml::to_string(&effective_value)
            .map_err(|e| anyhow::anyhow!("Failed to render merged config: {}", e))?;

        // Parse permissively (version-skew tolerance by design) while collecting
        // any unknown/ignored key paths so we can surface them for observability.
        // Warnings are emitted ONLY after a successful deserialization: a
        // malformed TOML must return the original parse error with no partial
        // warning spam.
        let (mut config, unknown_keys) = Self::parse_toml_collecting_unknown(&effective_content)
            .map_err(|e| {
                anyhow::anyhow!(
                    "Failed to parse merged config from {} and {}: {}",
                    policy_path.display(),
                    path.display(),
                    e
                )
            })?;

        for key in &unknown_keys {
            crate::logging::warn(&format!("Unknown config key '{}' ignored", key));
        }

        config.display.apply_legacy_compat();
        config.layer_metadata =
            Self::build_layer_metadata(&policy_path, &policy_value, durable_value.as_ref());
        if LOGGED_CONFIG_LAYER_SUMMARY.should_fire() {
            crate::logging::info(&format!(
                "CONFIG_LAYER policy={} durable_keys={} policy_keys={}",
                policy_path.display(),
                config
                    .layer_metadata
                    .provenance
                    .values()
                    .filter(|value| **value == ConfigProvenance::Durable)
                    .count(),
                config
                    .layer_metadata
                    .provenance
                    .values()
                    .filter(|value| **value == ConfigProvenance::Policy)
                    .count()
            ));
        }
        Ok(Some(config))
    }

    fn parse_toml_value(content: &str, path: &std::path::Path) -> anyhow::Result<toml::Value> {
        content
            .parse::<toml::Value>()
            .map_err(|e| anyhow::anyhow!("Failed to parse config file {}: {}", path.display(), e))
    }

    /// Deserialize a config TOML string permissively, returning the parsed
    /// config alongside the sorted/deduped set of unknown (ignored) key paths.
    ///
    /// Extracted as a pure function so unknown-key observability is unit-testable
    /// without touching the on-disk config path or the logging sink. Malformed
    /// TOML returns the underlying parse error (and no keys); the caller is
    /// responsible for emitting one warning per returned key.
    pub(crate) fn parse_toml_collecting_unknown(
        content: &str,
    ) -> Result<(Self, std::collections::BTreeSet<String>), toml::de::Error> {
        let mut unknown_keys: std::collections::BTreeSet<String> =
            std::collections::BTreeSet::new();
        let config: Self = serde_ignored::deserialize(toml::Deserializer::new(content), |path| {
            unknown_keys.insert(path.to_string());
        })?;
        Ok((config, unknown_keys))
    }

    /// Normalize `agents.memory_embedding_backend` to exactly `"local"` or
    /// `"openai"`.
    ///
    /// This is the one genuine load-time normalization: WI-5 depends on the
    /// invariant that the stored value is always one of those two exact
    /// lowercase strings. Any other value warns and falls back to `"local"`.
    /// Applied after file parse AND after env overrides (env can reintroduce a
    /// bad value), so it must run in both `load` and `load_strict`.
    pub(crate) fn normalize_memory_embedding_backend(&mut self) {
        let raw = self.agents.memory_embedding_backend.trim();
        let normalized = raw.to_ascii_lowercase();
        match normalized.as_str() {
            "local" | "openai" => {
                if self.agents.memory_embedding_backend != normalized {
                    self.agents.memory_embedding_backend = normalized;
                }
            }
            _ => {
                crate::logging::warn(&format!(
                    "Invalid agents.memory_embedding_backend '{}'; expected 'local' or 'openai'. Using 'local'.",
                    raw
                ));
                self.agents.memory_embedding_backend = "local".to_string();
            }
        }
    }

    /// Save config to file
    pub fn save(&self) -> anyhow::Result<()> {
        Self::with_config_file_lock(|| self.save_unlocked())
    }

    fn save_unlocked(&self) -> anyhow::Result<()> {
        let path = Self::path().ok_or_else(|| anyhow::anyhow!("No config path"))?;

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = if self.layer_metadata.policy_path.is_some() || Self::policy_file_exists() {
            self.durable_policy_aware_toml()?
        } else {
            // Critical compatibility invariant: without a policy layer, write the
            // exact same pretty TOML as the historical implementation.
            toml::to_string_pretty(self)?
        };
        std::fs::write(&path, content)?;
        Self::invalidate_cache();
        Ok(())
    }

    fn policy_file_exists() -> bool {
        Self::policy_path()
            .as_ref()
            .is_some_and(|path| path.exists())
    }

    fn durable_policy_aware_toml(&self) -> anyhow::Result<String> {
        let mut candidate = toml::Value::try_from(self)?;
        let defaults = toml::Value::try_from(Config::default())?;

        let policy_values = if self.layer_metadata.policy_values.is_empty() {
            Self::policy_path()
                .filter(|path| path.exists())
                .map(|path| {
                    let content = std::fs::read_to_string(&path).map_err(|e| {
                        anyhow::anyhow!(
                            "Failed to read policy config file {}: {}",
                            path.display(),
                            e
                        )
                    })?;
                    Self::parse_toml_value(&content, &path).map(|value| {
                        let mut values = BTreeMap::new();
                        Self::collect_policy_values(&value, &mut Vec::new(), &mut values);
                        values
                    })
                })
                .transpose()?
                .unwrap_or_default()
        } else {
            self.layer_metadata.policy_values.clone()
        };

        Self::warn_for_pinned_save_attempts(&candidate, &policy_values);
        Self::remove_pinned_paths(&mut candidate, &policy_values);
        Self::remove_default_values(&mut candidate, &defaults);

        toml::to_string_pretty(&candidate)
            .map_err(|e| anyhow::anyhow!("Failed to serialize durable config: {}", e))
    }

    fn with_config_file_lock<T>(f: impl FnOnce() -> anyhow::Result<T>) -> anyhow::Result<T> {
        let lock_path = Self::lock_path().ok_or_else(|| anyhow::anyhow!("No config lock path"))?;
        if let Some(parent) = lock_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let lock_file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&lock_path)
            .map_err(|e| {
                anyhow::anyhow!("Failed to open config lock {}: {}", lock_path.display(), e)
            })?;
        let _guard = ConfigFileLock::lock(lock_file, &lock_path)?;
        f()
    }

    fn patch_config_file(mut patch: impl FnMut(&mut Self) -> bool) -> anyhow::Result<Self> {
        Self::with_config_file_lock(|| {
            let mut cfg = Self::load();
            if patch(&mut cfg) {
                cfg.save_unlocked()?;
            }
            Ok(cfg)
        })
    }

    fn build_layer_metadata(
        policy_path: &std::path::Path,
        policy_value: &toml::Value,
        durable_value: Option<&toml::Value>,
    ) -> ConfigLayerMetadata {
        let mut policy_values = BTreeMap::new();
        Self::collect_policy_values(policy_value, &mut Vec::new(), &mut policy_values);

        let mut provenance = BTreeMap::new();
        if let Some(durable_value) = durable_value {
            let mut durable_paths = BTreeSet::new();
            Self::collect_durable_paths(durable_value, &mut Vec::new(), &mut durable_paths);
            for path in durable_paths {
                if !Self::policy_values_pin_path(&policy_values, &path) {
                    provenance.insert(path, ConfigProvenance::Durable);
                }
            }
        }
        for path in policy_values.keys() {
            provenance.insert(path.clone(), ConfigProvenance::Policy);
        }

        ConfigLayerMetadata {
            policy_path: Some(policy_path.to_path_buf()),
            pinned_paths: policy_values.keys().cloned().collect(),
            provenance,
            policy_values,
        }
    }

    fn merge_policy_over_durable(
        durable: &mut toml::Value,
        policy: &toml::Value,
        path: &mut Vec<String>,
    ) {
        match (durable, policy) {
            (toml::Value::Table(durable_table), toml::Value::Table(policy_table)) => {
                for (key, policy_child) in policy_table {
                    path.push(key.clone());
                    if let Some(durable_child) = durable_table.get_mut(key) {
                        Self::merge_policy_over_durable(durable_child, policy_child, path);
                    } else {
                        durable_table.insert(key.clone(), policy_child.clone());
                    }
                    path.pop();
                }
            }
            (durable, policy) => {
                *durable = policy.clone();
            }
        }
    }

    fn collect_policy_values(
        value: &toml::Value,
        path: &mut Vec<String>,
        out: &mut BTreeMap<String, toml::Value>,
    ) {
        if path.is_empty() {
            if let toml::Value::Table(table) = value {
                for (key, child) in table {
                    path.push(key.clone());
                    Self::collect_policy_values(child, path, out);
                    path.pop();
                }
            }
            return;
        }

        match value {
            toml::Value::Table(table) => {
                if table.is_empty() {
                    out.insert(path.join("."), value.clone());
                    return;
                }
                for (key, child) in table {
                    path.push(key.clone());
                    Self::collect_policy_values(child, path, out);
                    path.pop();
                }
            }
            _ => {
                out.insert(path.join("."), value.clone());
            }
        }
    }

    fn collect_durable_paths(
        value: &toml::Value,
        path: &mut Vec<String>,
        out: &mut BTreeSet<String>,
    ) {
        if path.is_empty() {
            if let toml::Value::Table(table) = value {
                for (key, child) in table {
                    path.push(key.clone());
                    Self::collect_durable_paths(child, path, out);
                    path.pop();
                }
            }
            return;
        }

        match value {
            toml::Value::Table(table) => {
                if table.is_empty() {
                    out.insert(path.join("."));
                    return;
                }
                for (key, child) in table {
                    path.push(key.clone());
                    Self::collect_durable_paths(child, path, out);
                    path.pop();
                }
            }
            _ => {
                out.insert(path.join("."));
            }
        }
    }

    fn policy_values_pin_path(policy_values: &BTreeMap<String, toml::Value>, path: &str) -> bool {
        policy_values
            .keys()
            .any(|policy_path| path == policy_path || path.starts_with(&format!("{policy_path}.")))
    }

    fn warn_for_pinned_save_attempts(
        candidate: &toml::Value,
        policy_values: &BTreeMap<String, toml::Value>,
    ) {
        for (path, policy_value) in policy_values {
            if let Some(candidate_value) = Self::value_at_path(candidate, path)
                && candidate_value != policy_value
            {
                Self::warn_once_pinned_config_save(path);
            }
        }
    }

    pub(crate) fn warn_once_pinned_config_save(path: &str) -> bool {
        let warned = WARNED_PINNED_CONFIG_SAVE_KEYS.get_or_init(|| Mutex::new(BTreeSet::new()));
        if !warned.lock().unwrap().insert(path.to_string()) {
            return false;
        }
        crate::logging::warn(&format!(
            "config: `{}` is managed by config.nix.toml; runtime change ignored on save",
            path
        ));
        true
    }

    #[cfg(test)]
    pub(crate) fn reset_pinned_config_save_warnings_for_tests() {
        if let Some(warned) = WARNED_PINNED_CONFIG_SAVE_KEYS.get() {
            warned.lock().unwrap().clear();
        }
    }

    fn remove_pinned_paths(
        candidate: &mut toml::Value,
        policy_values: &BTreeMap<String, toml::Value>,
    ) {
        let mut paths = policy_values.keys().cloned().collect::<Vec<_>>();
        paths.sort_by_key(|path| std::cmp::Reverse(path.split('.').count()));
        for path in paths {
            Self::remove_value_at_path(candidate, &path);
        }
        Self::remove_empty_tables(candidate);
    }

    fn remove_default_values(candidate: &mut toml::Value, defaults: &toml::Value) -> bool {
        if candidate == defaults {
            return true;
        }
        match (candidate, defaults) {
            (toml::Value::Table(candidate_table), toml::Value::Table(default_table)) => {
                let keys = candidate_table.keys().cloned().collect::<Vec<_>>();
                for key in keys {
                    let remove = match (candidate_table.get_mut(&key), default_table.get(&key)) {
                        (Some(candidate_child), Some(default_child)) => {
                            Self::remove_default_values(candidate_child, default_child)
                        }
                        _ => false,
                    };
                    if remove {
                        candidate_table.remove(&key);
                    }
                }
                candidate_table.is_empty()
            }
            _ => false,
        }
    }

    fn remove_empty_tables(candidate: &mut toml::Value) -> bool {
        match candidate {
            toml::Value::Table(table) => {
                let keys = table.keys().cloned().collect::<Vec<_>>();
                for key in keys {
                    if let Some(child) = table.get_mut(&key)
                        && Self::remove_empty_tables(child)
                    {
                        table.remove(&key);
                    }
                }
                table.is_empty()
            }
            _ => false,
        }
    }

    fn value_at_path<'a>(value: &'a toml::Value, path: &str) -> Option<&'a toml::Value> {
        let mut current = value;
        for segment in path.split('.') {
            current = current.as_table()?.get(segment)?;
        }
        Some(current)
    }

    fn remove_value_at_path(value: &mut toml::Value, path: &str) -> Option<toml::Value> {
        let mut current = value;
        let mut segments = path.split('.').peekable();
        while let Some(segment) = segments.next() {
            let table = current.as_table_mut()?;
            if segments.peek().is_none() {
                return table.remove(segment);
            }
            current = table.get_mut(segment)?;
        }
        None
    }

    /// Mark the process-cached config as stale and notify dependent caches.
    pub fn invalidate_cache() {
        super::invalidate_config_cache();
    }

    /// Update the copilot premium mode in the config file.
    /// Reloads, patches, and saves so it doesn't clobber other fields.
    pub fn set_copilot_premium(mode: Option<&str>) -> anyhow::Result<()> {
        Self::patch_config_file(|cfg| {
            cfg.provider.copilot_premium = mode.map(|s| s.to_string());
            true
        })?;
        crate::logging::info(&format!(
            "Saved copilot_premium to config: {}",
            mode.unwrap_or("(none)")
        ));
        Ok(())
    }

    /// Update just the default model and provider in the config file.
    /// This reloads, patches, and saves so it doesn't clobber other fields.
    pub fn set_default_model(model: Option<&str>, provider: Option<&str>) -> anyhow::Result<()> {
        Self::patch_config_file(|cfg| {
            cfg.provider.default_model = model.map(|s| s.to_string());
            cfg.provider.default_provider = provider.map(|s| s.to_string());
            true
        })?;
        crate::logging::info(&format!(
            "Saved default model: {}, provider: {}",
            model.unwrap_or("(none)"),
            provider.unwrap_or("(auto)")
        ));
        Ok(())
    }

    /// Update just the default provider in the config file.
    pub fn set_default_provider(provider: Option<&str>) -> anyhow::Result<()> {
        Self::patch_config_file(|cfg| {
            cfg.provider.default_provider = provider.map(|s| s.to_string());
            true
        })?;
        crate::logging::info(&format!(
            "Saved default provider: {}",
            provider.unwrap_or("(auto)")
        ));
        Ok(())
    }

    /// Update just the default model in the config file.
    pub fn set_default_model_only(model: Option<&str>) -> anyhow::Result<()> {
        Self::patch_config_file(|cfg| {
            cfg.provider.default_model = model.map(|s| s.to_string());
            true
        })?;
        crate::logging::info(&format!(
            "Saved default model: {}",
            model.unwrap_or("(none)")
        ));
        Ok(())
    }

    /// Update the persisted OpenAI reasoning effort preference.
    pub fn set_openai_reasoning_effort(value: Option<&str>) -> anyhow::Result<()> {
        Self::patch_config_file(|cfg| {
            cfg.provider.openai_reasoning_effort = value.map(|s| s.to_string());
            true
        })?;
        crate::logging::info(&format!(
            "Saved openai_reasoning_effort to config: {}",
            value.unwrap_or("(none)")
        ));
        Ok(())
    }

    /// Update the persisted OpenAI transport preference.
    pub fn set_openai_transport(value: Option<&str>) -> anyhow::Result<()> {
        Self::patch_config_file(|cfg| {
            cfg.provider.openai_transport = value.map(|s| s.to_string());
            true
        })?;
        crate::logging::info(&format!(
            "Saved openai_transport to config: {}",
            value.unwrap_or("(none)")
        ));
        Ok(())
    }

    /// Update the persisted OpenAI service tier preference.
    pub fn set_openai_service_tier(value: Option<&str>) -> anyhow::Result<()> {
        Self::patch_config_file(|cfg| {
            cfg.provider.openai_service_tier = value.map(|s| s.to_string());
            true
        })?;
        crate::logging::info(&format!(
            "Saved openai_service_tier to config: {}",
            value.unwrap_or("(none)")
        ));
        Ok(())
    }

    /// Update the persisted default alignment preference.
    pub fn set_display_centered(centered: bool) -> anyhow::Result<()> {
        Self::patch_config_file(|cfg| {
            cfg.display.centered = centered;
            true
        })?;
        crate::logging::info(&format!("Saved display.centered to config: {}", centered));
        Ok(())
    }

    /// Update the persisted reasoning display mode preference.
    pub fn set_reasoning_display(mode: ReasoningDisplayMode) -> anyhow::Result<()> {
        Self::patch_config_file(|cfg| {
            cfg.display.set_reasoning_display(mode);
            true
        })?;
        crate::logging::info(&format!(
            "Saved display.reasoning_display to config: {}",
            mode.label()
        ));
        Ok(())
    }

    /// Update the persisted compact-notifications preference.
    pub fn set_compact_notifications(compact: bool) -> anyhow::Result<()> {
        Self::patch_config_file(|cfg| {
            cfg.display.compact_notifications = compact;
            true
        })?;
        crate::logging::info(&format!(
            "Saved display.compact_notifications to config: {}",
            compact
        ));
        Ok(())
    }

    /// Update the persisted show-agentgrep-output preference.
    pub fn set_show_agentgrep_output(show: bool) -> anyhow::Result<()> {
        Self::patch_config_file(|cfg| {
            cfg.display.show_agentgrep_output = show;
            true
        })?;
        crate::logging::info(&format!(
            "Saved display.show_agentgrep_output to config: {}",
            show
        ));
        Ok(())
    }

    /// Persist the baked global launch-hotkey mapping.
    ///
    /// Auto-import calls this once with the per-repo chord -> directory layout it
    /// inferred. `imported` is set so the bake never runs twice and later manual
    /// edits are not clobbered.
    pub fn set_launch_hotkeys(
        entries: Vec<jcode_config_types::LaunchHotkeyEntry>,
        enabled: bool,
    ) -> anyhow::Result<()> {
        let entry_count = entries.len();
        let mut entries = Some(entries);
        Self::patch_config_file(|cfg| {
            cfg.launch_hotkeys.entries = entries.take().unwrap_or_default();
            cfg.launch_hotkeys.enabled = Some(enabled);
            cfg.launch_hotkeys.imported = true;
            true
        })?;
        crate::logging::info(&format!(
            "Saved {} launch hotkey(s) to config (enabled={enabled})",
            entry_count
        ));
        Ok(())
    }

    /// One-time bake of per-repo launch hotkeys from session history.
    ///
    /// Scans `~/.jcode/sessions` for the directories the user works in most,
    /// ranks them (recency-weighted, git-root folded, home excluded), and writes
    /// a static chord -> directory mapping into config: top repo on `Cmd+;`, home
    /// on `Cmd+'`, and the next repos on `Cmd+[` / `Cmd+]` / `Cmd+\`.
    ///
    /// Idempotent and side-effect-light:
    /// - Runs only on platforms with global launch hotkeys (macOS, Linux,
    ///   Windows).
    /// - No-ops once `launch_hotkeys.imported` is set, so it bakes exactly once
    ///   and never overwrites later manual edits.
    /// - No-ops when there are not at least two rankable repos, so we do not
    ///   commit a degenerate "everything is home" layout on a fresh machine; the
    ///   built-in 3 hotkeys keep working until there is real history.
    ///
    /// Returns `true` when it wrote a baked mapping (so the caller can trigger a
    /// hotkey reinstall), `false` otherwise. Best-effort: errors are logged and
    /// swallowed.
    #[cfg(any(target_os = "macos", target_os = "linux", windows))]
    pub fn bake_launch_hotkeys_once() -> bool {
        use jcode_import_core::repo_ranking;

        let cfg = Self::load();
        if cfg.launch_hotkeys.imported {
            return false;
        }
        let Ok(jcode_dir) = jcode_dir() else {
            return false;
        };
        let sessions_dir = jcode_dir.join("sessions");
        let Some(home) = dirs::home_dir() else {
            return false;
        };

        // Cheap gate: count session files without reading them. Skip the full
        // scan until there is at least a little history, so brand-new installs do
        // not pay the read cost (and we do not bake a degenerate layout).
        let session_count = std::fs::read_dir(&sessions_dir)
            .map(|entries| {
                entries
                    .flatten()
                    .filter(|e| e.file_name().to_str().is_some_and(|n| n.ends_with(".json")))
                    .count()
            })
            .unwrap_or(0);
        const MIN_SESSIONS_TO_BAKE: usize = 3;
        const GIVE_UP_SESSION_COUNT: usize = 50;
        if session_count < MIN_SESSIONS_TO_BAKE {
            return false;
        }

        let plan = repo_ranking::plan_launch_hotkeys_from_sessions(
            &sessions_dir,
            &home,
            chrono::Utc::now(),
        );

        // `plan` always contains the home slot; a length of 1 means no rankable
        // repos were found.
        if plan.len() < 2 {
            // If the user has lots of history but still no rankable repos, stop
            // re-scanning on every launch: mark imported with no custom entries
            // (the built-in 3 hotkeys keep working).
            if session_count >= GIVE_UP_SESSION_COUNT
                && let Err(err) = Self::set_launch_hotkeys(Vec::new(), true)
            {
                crate::logging::warn(&format!("launch hotkey bake give-up persist failed: {err}"));
            }
            crate::logging::info(
                "launch hotkey bake: not enough repo history yet; keeping defaults",
            );
            return false;
        }

        let entries: Vec<jcode_config_types::LaunchHotkeyEntry> = plan
            .into_iter()
            .map(|p| jcode_config_types::LaunchHotkeyEntry {
                chord: p.chord,
                // Home keeps the dynamic sentinel so it tracks `$HOME`; repos are
                // baked to absolute paths.
                dir: if p.label == "home" {
                    "$HOME".to_string()
                } else {
                    p.dir
                },
                label: p.label,
                self_dev: false,
            })
            .collect();

        match Self::set_launch_hotkeys(entries, true) {
            Ok(()) => {
                crate::logging::info("launch hotkey bake: wrote per-repo mapping to config");
                true
            }
            Err(err) => {
                crate::logging::warn(&format!("launch hotkey bake failed to persist: {err}"));
                false
            }
        }
    }

    /// No-op bake on platforms without global launch hotkeys.
    #[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
    pub fn bake_launch_hotkeys_once() -> bool {
        false
    }

    fn normalize_external_auth_source_id(source_id: &str) -> String {
        source_id.trim().to_ascii_lowercase()
    }

    pub(crate) fn trusted_external_auth_path_entry(
        source_id: &str,
        path: &std::path::Path,
    ) -> anyhow::Result<String> {
        let source_id = Self::normalize_external_auth_source_id(source_id);
        if source_id.is_empty() {
            anyhow::bail!("External auth source id cannot be empty");
        }
        let canonical = crate::storage::validate_external_auth_file(path)?;
        Ok(format!(
            "{}|{}",
            source_id,
            canonical.to_string_lossy().to_ascii_lowercase()
        ))
    }

    pub fn external_auth_source_allowed(source_id: &str) -> bool {
        let source_id = Self::normalize_external_auth_source_id(source_id);
        if source_id.is_empty() {
            return false;
        }

        let cfg = Self::load();
        cfg.auth
            .trusted_external_sources
            .iter()
            .any(|value| value.trim().eq_ignore_ascii_case(&source_id))
    }

    pub fn external_auth_source_allowed_for_path(source_id: &str, path: &std::path::Path) -> bool {
        let Ok(entry) = Self::trusted_external_auth_path_entry(source_id, path) else {
            return false;
        };

        let cfg = Self::load();
        cfg.auth
            .trusted_external_source_paths
            .iter()
            .any(|value| value.trim().eq_ignore_ascii_case(&entry))
    }

    /// Startup-sensitive variant that uses the process-cached config snapshot.
    ///
    /// This avoids reloading config.toml repeatedly during cold-start probes.
    pub fn external_auth_source_allowed_for_path_cached(
        source_id: &str,
        path: &std::path::Path,
    ) -> bool {
        let Ok(entry) = Self::trusted_external_auth_path_entry(source_id, path) else {
            return false;
        };

        if config()
            .auth
            .trusted_external_source_paths
            .iter()
            .any(|value| value.trim().eq_ignore_ascii_case(&entry))
        {
            return true;
        }

        // The global config snapshot can be initialized before an auth flow saves
        // a new path-bound trust decision, or before tests switch JCODE_HOME. Fall
        // back to a fresh load on cache misses so fast auth probes remain correct
        // without penalizing the common already-trusted path.
        Self::load()
            .auth
            .trusted_external_source_paths
            .iter()
            .any(|value| value.trim().eq_ignore_ascii_case(&entry))
    }

    pub fn allow_external_auth_source(source_id: &str) -> anyhow::Result<()> {
        let source_id = Self::normalize_external_auth_source_id(source_id);
        if source_id.is_empty() {
            anyhow::bail!("External auth source id cannot be empty");
        }

        Self::patch_config_file(|cfg| {
            if cfg
                .auth
                .trusted_external_sources
                .iter()
                .any(|value| value.trim().eq_ignore_ascii_case(&source_id))
            {
                return false;
            }
            cfg.auth.trusted_external_sources.push(source_id.clone());
            cfg.auth.trusted_external_sources.sort();
            cfg.auth.trusted_external_sources.dedup();
            true
        })?;

        crate::logging::info(&format!(
            "Saved trusted external auth source to config: {}",
            source_id
        ));
        Ok(())
    }

    pub fn allow_external_auth_source_for_path(
        source_id: &str,
        path: &std::path::Path,
    ) -> anyhow::Result<()> {
        let entry = Self::trusted_external_auth_path_entry(source_id, path)?;
        Self::patch_config_file(|cfg| {
            if cfg
                .auth
                .trusted_external_source_paths
                .iter()
                .any(|value| value.trim().eq_ignore_ascii_case(&entry))
            {
                return false;
            }
            cfg.auth.trusted_external_source_paths.push(entry.clone());
            cfg.auth.trusted_external_source_paths.sort();
            cfg.auth.trusted_external_source_paths.dedup();
            true
        })?;
        crate::logging::info(&format!(
            "Saved trusted external auth source path: {}",
            entry
        ));
        Ok(())
    }

    pub fn revoke_external_auth_source_for_path(
        source_id: &str,
        path: &std::path::Path,
    ) -> anyhow::Result<()> {
        let entry = Self::trusted_external_auth_path_entry(source_id, path)?;
        let mut changed = false;
        Self::patch_config_file(|cfg| {
            let before = cfg.auth.trusted_external_source_paths.len();
            cfg.auth
                .trusted_external_source_paths
                .retain(|value| !value.trim().eq_ignore_ascii_case(&entry));
            changed = cfg.auth.trusted_external_source_paths.len() != before;
            changed
        })?;
        if changed {
            crate::logging::info(&format!(
                "Removed trusted external auth source path: {}",
                entry
            ));
        }
        Ok(())
    }

    /// Remove a source-level (non-path) trust decision, e.g. for credentials
    /// that have no stable on-disk path (macOS Keychain items).
    pub fn revoke_external_auth_source(source_id: &str) -> anyhow::Result<()> {
        let source_id = Self::normalize_external_auth_source_id(source_id);
        if source_id.is_empty() {
            return Ok(());
        }
        let mut changed = false;
        Self::patch_config_file(|cfg| {
            let before = cfg.auth.trusted_external_sources.len();
            cfg.auth
                .trusted_external_sources
                .retain(|value| !value.trim().eq_ignore_ascii_case(&source_id));
            changed = cfg.auth.trusted_external_sources.len() != before;
            changed
        })?;
        if changed {
            crate::logging::info(&format!(
                "Removed trusted external auth source: {}",
                source_id
            ));
        }
        Ok(())
    }
}

struct ConfigFileLock {
    file: File,
}

impl ConfigFileLock {
    fn lock(file: File, path: &std::path::Path) -> anyhow::Result<Self> {
        let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) };
        if rc != 0 {
            return Err(anyhow::anyhow!(
                "Failed to lock config lock {}: {}",
                path.display(),
                std::io::Error::last_os_error()
            ));
        }
        Ok(Self { file })
    }
}

impl Drop for ConfigFileLock {
    fn drop(&mut self) {
        let _ = unsafe { libc::flock(self.file.as_raw_fd(), libc::LOCK_UN) };
    }
}
