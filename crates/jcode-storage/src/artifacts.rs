use anyhow::{Result, anyhow, bail};
use std::fmt;
use std::path::{Component, Path, PathBuf};

use crate::RuntimePaths;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DurableKind {
    State,
    Inbox,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TemporaryKind {
    Cache,
    Scratch,
    Lock,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ArtifactTier {
    Sensitive,
    Durable(DurableKind),
    Temporary(TemporaryKind),
    ExternalSecret,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CanonicalRoot {
    Sensitive,
    DurableState,
    DurableInbox,
    JcodeHomeInbox,
    TemporaryCache,
    TemporaryScratch,
    TemporaryLock,
    ExternalSecret,
}

impl CanonicalRoot {
    pub const fn tier(self) -> ArtifactTier {
        match self {
            Self::Sensitive => ArtifactTier::Sensitive,
            Self::DurableState => ArtifactTier::Durable(DurableKind::State),
            Self::DurableInbox => ArtifactTier::Durable(DurableKind::Inbox),
            Self::JcodeHomeInbox => ArtifactTier::Durable(DurableKind::Inbox),
            Self::TemporaryCache => ArtifactTier::Temporary(TemporaryKind::Cache),
            Self::TemporaryScratch => ArtifactTier::Temporary(TemporaryKind::Scratch),
            Self::TemporaryLock => ArtifactTier::Temporary(TemporaryKind::Lock),
            Self::ExternalSecret => ArtifactTier::ExternalSecret,
        }
    }

    fn base_path(self, paths: &RuntimePaths) -> Result<PathBuf> {
        // TODO(W10): review the canonical Windows and Linux roots before c1 lands
        // on those platforms. The classification is platform-independent, but the
        // W10 investigation validated the concrete root choices on macOS only.
        match self {
            Self::Sensitive => Ok(jcode_root(paths)?.join("secrets")),
            Self::DurableState => Ok(paths.durable_state_dir()),
            Self::DurableInbox => Ok(paths.durable_state_dir().join("inbox")),
            Self::JcodeHomeInbox => jcode_root(paths),
            Self::TemporaryCache => paths.app_cache_dir(),
            Self::TemporaryScratch => Ok(paths.runtime_dir()),
            Self::TemporaryLock => Ok(paths.runtime_dir().join("locks")),
            Self::ExternalSecret => bail!("external secrets do not have a managed root"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LegacyRoot {
    Jcode,
    AppConfig,
    AppCache,
    DurableState,
    Runtime,
    SystemTemp,
    UserHome,
    DaemonSocket,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LegacyLocation {
    pub root: LegacyRoot,
    pub template: &'static str,
}

impl LegacyLocation {
    pub const fn new(root: LegacyRoot, template: &'static str) -> Self {
        Self { root, template }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PinnedLegacyPath {
    pub location: LegacyLocation,
    pub reason: &'static str,
}

impl PinnedLegacyPath {
    pub const fn new(root: LegacyRoot, template: &'static str, reason: &'static str) -> Self {
        Self {
            location: LegacyLocation::new(root, template),
            reason,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArtifactSpec {
    pub id: ArtifactId,
    pub name: &'static str,
    pub tier: ArtifactTier,
    pub canonical_root: CanonicalRoot,
    pub canonical_template: &'static str,
    pub legacy: Option<LegacyLocation>,
    pub pinned: Option<PinnedLegacyPath>,
    pub legacy_companions: &'static [&'static str],
    pub may_contain_secrets: bool,
    pub example_key: &'static str,
}

impl ArtifactSpec {
    pub fn canonical_path(&self, paths: &RuntimePaths, key: &dyn ArtifactKey) -> Result<PathBuf> {
        resolve_canonical_location(paths, self.canonical_root, self.canonical_template, key)
    }

    pub fn active_path(&self, paths: &RuntimePaths, key: &dyn ArtifactKey) -> Result<PathBuf> {
        if let Some(pin) = self.pinned {
            return resolve_legacy_location(paths, pin.location, key);
        }
        if let Some(legacy) = self.legacy {
            return resolve_legacy_location(paths, legacy, key);
        }
        self.canonical_path(paths, key)
    }
}

pub trait ArtifactKey {
    fn path_fragment(&self) -> Result<String>;
}

impl ArtifactKey for () {
    fn path_fragment(&self) -> Result<String> {
        Ok(String::new())
    }
}

impl ArtifactKey for str {
    fn path_fragment(&self) -> Result<String> {
        validate_relative_fragment(self)?;
        Ok(self.to_owned())
    }
}

impl ArtifactKey for String {
    fn path_fragment(&self) -> Result<String> {
        self.as_str().path_fragment()
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SessionInboxId(String);

impl SessionInboxId {
    pub fn new(value: impl Into<String>) -> Result<Self> {
        let value = value.into();
        let mut components = Path::new(&value).components();
        if !matches!(components.next(), Some(Component::Normal(_))) || components.next().is_some() {
            bail!("session inbox ID must be one path component: {value}");
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SessionInboxId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl ArtifactKey for SessionInboxId {
    fn path_fragment(&self) -> Result<String> {
        Ok(self.0.clone())
    }
}

fn validate_relative_fragment(value: &str) -> Result<()> {
    if value.is_empty() {
        bail!("artifact key must not be empty");
    }
    let path = Path::new(value);
    if path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        bail!("artifact key must be a relative path fragment: {value}");
    }
    Ok(())
}

fn render_template(template: &str, key: &dyn ArtifactKey) -> Result<PathBuf> {
    let fragment = key.path_fragment()?;
    if template.contains("{key}") {
        if fragment.is_empty() {
            bail!("artifact template requires a key: {template}");
        }
        Ok(PathBuf::from(template.replace("{key}", &fragment)))
    } else if fragment.is_empty() {
        Ok(PathBuf::from(template))
    } else {
        bail!("artifact template does not accept a key: {template}");
    }
}

fn jcode_root(paths: &RuntimePaths) -> Result<PathBuf> {
    paths
        .jcode_dir()
        .ok_or_else(|| anyhow!("No home directory"))
}

fn resolve_canonical_location(
    paths: &RuntimePaths,
    root: CanonicalRoot,
    template: &str,
    key: &dyn ArtifactKey,
) -> Result<PathBuf> {
    let relative = render_template(template, key)?;
    if root == CanonicalRoot::ExternalSecret {
        return paths.user_home_path(relative);
    }
    Ok(root.base_path(paths)?.join(relative))
}

fn resolve_legacy_location(
    paths: &RuntimePaths,
    location: LegacyLocation,
    key: &dyn ArtifactKey,
) -> Result<PathBuf> {
    let relative = render_template(location.template, key)?;
    match location.root {
        LegacyRoot::Jcode => Ok(jcode_root(paths)?.join(relative)),
        LegacyRoot::AppConfig => Ok(paths.app_config_dir()?.join(relative)),
        LegacyRoot::AppCache => Ok(paths.app_cache_dir()?.join(relative)),
        LegacyRoot::DurableState => Ok(paths.durable_state_dir().join(relative)),
        LegacyRoot::Runtime => Ok(paths.runtime_dir().join(relative)),
        LegacyRoot::SystemTemp => Ok(std::env::temp_dir().join(relative)),
        LegacyRoot::UserHome => paths.user_home_path(relative),
        LegacyRoot::DaemonSocket => {
            if let Ok(custom) = std::env::var("JCODE_SOCKET") {
                Ok(PathBuf::from(custom))
            } else {
                Ok(paths.runtime_dir().join("jcode.sock"))
            }
        }
    }
}

pub fn canonical_tier_for_path(paths: &RuntimePaths, path: &Path) -> Result<Option<ArtifactTier>> {
    // Test harnesses intentionally collapse otherwise distinct roots under one
    // isolated directory. Exact registry entries must win over root prefixes so
    // those aliases do not change an artifact's declared tier.
    for spec in ARTIFACTS {
        let dynamic_key = spec.example_key.to_owned();
        let key: &dyn ArtifactKey = if dynamic_key.is_empty() {
            &()
        } else {
            &dynamic_key
        };
        if spec.canonical_path(paths, key)? == path {
            return Ok(Some(spec.tier));
        }
    }

    let managed_roots = [
        CanonicalRoot::Sensitive,
        CanonicalRoot::DurableInbox,
        CanonicalRoot::DurableState,
        CanonicalRoot::TemporaryLock,
        CanonicalRoot::TemporaryCache,
        CanonicalRoot::TemporaryScratch,
        CanonicalRoot::JcodeHomeInbox,
    ];
    for root in managed_roots {
        if path.starts_with(root.base_path(paths)?) {
            return Ok(Some(root.tier()));
        }
    }

    Ok(None)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecretPath(PathBuf);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DurablePath(PathBuf);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TemporaryPath {
    path: PathBuf,
    kind: TemporaryKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReadOnlyPath(PathBuf);

macro_rules! impl_path_accessors {
    ($type:ident) => {
        impl $type {
            pub fn as_path(&self) -> &Path {
                &self.0
            }

            pub fn into_path_buf(self) -> PathBuf {
                self.0
            }
        }

        impl AsRef<Path> for $type {
            fn as_ref(&self) -> &Path {
                self.as_path()
            }
        }
    };
}

impl_path_accessors!(SecretPath);
impl_path_accessors!(DurablePath);
impl_path_accessors!(ReadOnlyPath);

fn backup_sidecar_path(path: &Path) -> PathBuf {
    let mut value = path.as_os_str().to_os_string();
    value.push(".bak");
    PathBuf::from(value)
}

impl SecretPath {
    pub fn backup_sidecar(&self) -> Self {
        Self(backup_sidecar_path(self.as_path()))
    }
}

impl DurablePath {
    pub fn backup_sidecar(&self) -> Self {
        Self(backup_sidecar_path(self.as_path()))
    }
}

impl TemporaryPath {
    pub fn as_path(&self) -> &Path {
        &self.path
    }

    pub fn into_path_buf(self) -> PathBuf {
        self.path
    }

    pub const fn kind(&self) -> TemporaryKind {
        self.kind
    }

    pub fn backup_sidecar(&self) -> Self {
        Self {
            path: backup_sidecar_path(self.as_path()),
            kind: self.kind,
        }
    }
}

impl AsRef<Path> for TemporaryPath {
    fn as_ref(&self) -> &Path {
        self.as_path()
    }
}

mod sealed {
    pub trait Sealed {}
}

pub trait ArtifactTag: sealed::Sealed {
    type Key: ArtifactKey;
    const ID: ArtifactId;
}

pub trait SensitiveArtifact: ArtifactTag {}
pub trait DurableArtifact: ArtifactTag {}
pub trait TemporaryArtifact: ArtifactTag {
    const KIND: TemporaryKind;
}
pub trait ExternalArtifact: ArtifactTag {}

fn spec_for(id: ArtifactId) -> &'static ArtifactSpec {
    &ARTIFACTS[id as usize]
}

pub fn secret_path<A: SensitiveArtifact>(key: A::Key) -> Result<SecretPath> {
    Ok(SecretPath(
        spec_for(A::ID).active_path(&RuntimePaths::current(), &key)?,
    ))
}

pub fn durable_path<A: DurableArtifact>(key: A::Key) -> Result<DurablePath> {
    Ok(DurablePath(
        spec_for(A::ID).active_path(&RuntimePaths::current(), &key)?,
    ))
}

pub fn temporary_path<A: TemporaryArtifact>(key: A::Key) -> Result<TemporaryPath> {
    Ok(TemporaryPath {
        path: spec_for(A::ID).active_path(&RuntimePaths::current(), &key)?,
        kind: A::KIND,
    })
}

pub fn external_secret<A: ExternalArtifact>(key: A::Key) -> Result<ReadOnlyPath> {
    Ok(ReadOnlyPath(
        spec_for(A::ID).active_path(&RuntimePaths::current(), &key)?,
    ))
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProviderEnvId {
    Kimi,
    Ollama,
    OpenAi,
    Anthropic,
    OpenRouter,
    OpenAiCompatible,
    Cursor,
}

impl fmt::Display for ProviderEnvId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Kimi => "kimi",
            Self::Ollama => "ollama",
            Self::OpenAi => "openai",
            Self::Anthropic => "anthropic",
            Self::OpenRouter => "openrouter",
            Self::OpenAiCompatible => "openai-compatible",
            Self::Cursor => "cursor",
        })
    }
}

impl ArtifactKey for ProviderEnvId {
    fn path_fragment(&self) -> Result<String> {
        Ok(self.to_string())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ModelCatalogId {
    Anthropic,
    Antigravity,
    Bedrock,
    Copilot,
    Cursor,
    Gemini,
    OpenAi,
    Remote,
}

impl fmt::Display for ModelCatalogId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Anthropic => "anthropic_model_catalog_cache.json",
            Self::Antigravity => "antigravity_models_cache.json",
            Self::Bedrock => "bedrock_models_cache.json",
            Self::Copilot => "copilot_models_cache.json",
            Self::Cursor => "cursor_models_cache.json",
            Self::Gemini => "gemini_models_cache.json",
            Self::OpenAi => "openai_model_catalog_cache.json",
            Self::Remote => "remote_model_catalog_cache.json",
        })
    }
}

impl ArtifactKey for ModelCatalogId {
    fn path_fragment(&self) -> Result<String> {
        Ok(self.to_string())
    }
}

macro_rules! impl_marker_trait {
    ($name:ident, Sensitive) => {
        impl SensitiveArtifact for tag::$name {}
    };
    ($name:ident, Durable) => {
        impl DurableArtifact for tag::$name {}
    };
    ($name:ident, Cache) => {
        impl TemporaryArtifact for tag::$name {
            const KIND: TemporaryKind = TemporaryKind::Cache;
        }
    };
    ($name:ident, Scratch) => {
        impl TemporaryArtifact for tag::$name {
            const KIND: TemporaryKind = TemporaryKind::Scratch;
        }
    };
    ($name:ident, Lock) => {
        impl TemporaryArtifact for tag::$name {
            const KIND: TemporaryKind = TemporaryKind::Lock;
        }
    };
    ($name:ident, External) => {
        impl ExternalArtifact for tag::$name {}
    };
}

macro_rules! legacy_companions {
    (ConfigToml) => {
        &["config.toml.hm-backup"]
    };
    (SessionLock) => {
        &["streaming_pids/{key}", ".pid-markers.lock"]
    };
    (LegacySessionRoot) => {
        &[
            "sessions/{key}.json",
            "sessions/{key}.journal.jsonl",
            "sessions/{key}.evidence.jsonl",
        ]
    };
    ($name:ident) => {
        &[]
    };
}

macro_rules! artifact_map {
    (
        $(
            $name:ident {
                marker: $marker:ident,
                key: $key:ty,
                tier: $tier:expr,
                canonical_root: $canonical_root:expr,
                canonical: $canonical:expr,
                legacy: $legacy:expr,
                pinned: $pinned:expr,
                may_contain_secrets: $may_contain_secrets:expr,
                example_key: $example_key:expr,
            }
        ),+ $(,)?
    ) => {
        #[repr(usize)]
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub enum ArtifactId {
            $($name),+
        }

        impl ArtifactId {
            pub const ALL: &'static [Self] = &[$(Self::$name),+];
        }

        pub mod tag {
            $(
                #[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
                pub struct $name;
            )+
        }

        $(
            impl sealed::Sealed for tag::$name {}
            impl ArtifactTag for tag::$name {
                type Key = $key;
                const ID: ArtifactId = ArtifactId::$name;
            }
            impl_marker_trait!($name, $marker);
        )+

        pub static ARTIFACTS: &[ArtifactSpec] = &[
            $(
                ArtifactSpec {
                    id: ArtifactId::$name,
                    name: stringify!($name),
                    tier: $tier,
                    canonical_root: $canonical_root,
                    canonical_template: $canonical,
                    legacy: $legacy,
                    pinned: $pinned,
                    legacy_companions: legacy_companions!($name),
                    may_contain_secrets: $may_contain_secrets,
                    example_key: $example_key,
                }
            ),+
        ];
    };
}

const CONFIG_PIN: PinnedLegacyPath = PinnedLegacyPath::new(
    LegacyRoot::Jcode,
    "config.toml",
    "user-facing path contract; nix-managed symlink; home-manager backup sibling",
);
const SESSION_LOCK_PIN: PinnedLegacyPath = PinnedLegacyPath::new(
    LegacyRoot::Jcode,
    "active_pids/{key}",
    "per-session lock and PID paths are consumed by existing tooling and scripts",
);
const LEGACY_SESSION_ROOT_PIN: PinnedLegacyPath = PinnedLegacyPath::new(
    LegacyRoot::Jcode,
    "sessions",
    "session history readers and external tools depend on the legacy root",
);
const DAEMON_SOCKET_PIN: PinnedLegacyPath = PinnedLegacyPath::new(
    LegacyRoot::DaemonSocket,
    "",
    "daemon discovery contract, including the JCODE_SOCKET override",
);

artifact_map! {
    ClaudeOauth {
        marker: Sensitive,
        key: (),
        tier: ArtifactTier::Sensitive,
        canonical_root: CanonicalRoot::Sensitive,
        canonical: "auth.json",
        legacy: Some(LegacyLocation::new(LegacyRoot::Jcode, "auth.json")),
        pinned: None,
        may_contain_secrets: true,
        example_key: "",
    },
    OpenAiOauth {
        marker: Sensitive,
        key: (),
        tier: ArtifactTier::Sensitive,
        canonical_root: CanonicalRoot::Sensitive,
        canonical: "openai-auth.json",
        legacy: Some(LegacyLocation::new(LegacyRoot::Jcode, "openai-auth.json")),
        pinned: None,
        may_contain_secrets: true,
        example_key: "",
    },
    GoogleCredentials {
        marker: Sensitive,
        key: (),
        tier: ArtifactTier::Sensitive,
        canonical_root: CanonicalRoot::Sensitive,
        canonical: "google_credentials.json",
        legacy: Some(LegacyLocation::new(LegacyRoot::Jcode, "google_credentials.json")),
        pinned: None,
        may_contain_secrets: true,
        example_key: "",
    },
    GoogleOauth {
        marker: Sensitive,
        key: (),
        tier: ArtifactTier::Sensitive,
        canonical_root: CanonicalRoot::Sensitive,
        canonical: "google_oauth.json",
        legacy: Some(LegacyLocation::new(LegacyRoot::Jcode, "google_oauth.json")),
        pinned: None,
        may_contain_secrets: true,
        example_key: "",
    },
    KimiCredentials {
        marker: Sensitive,
        key: (),
        tier: ArtifactTier::Sensitive,
        canonical_root: CanonicalRoot::Sensitive,
        canonical: "kimi/credentials.json",
        legacy: Some(LegacyLocation::new(LegacyRoot::AppConfig, "kimi/credentials.json")),
        pinned: None,
        may_contain_secrets: true,
        example_key: "",
    },
    KimiDeviceId {
        marker: Sensitive,
        key: (),
        tier: ArtifactTier::Sensitive,
        canonical_root: CanonicalRoot::Sensitive,
        canonical: "kimi/device_id",
        legacy: Some(LegacyLocation::new(LegacyRoot::AppConfig, "kimi/device_id")),
        pinned: None,
        may_contain_secrets: true,
        example_key: "",
    },
    GrokDirectCredentials {
        marker: Sensitive,
        key: (),
        tier: ArtifactTier::Sensitive,
        canonical_root: CanonicalRoot::Sensitive,
        canonical: "grok-direct/credentials.json",
        legacy: Some(LegacyLocation::new(LegacyRoot::AppConfig, "grok-direct/credentials.json")),
        pinned: None,
        may_contain_secrets: true,
        example_key: "",
    },
    ProviderEnvFile {
        marker: Sensitive,
        key: ProviderEnvId,
        tier: ArtifactTier::Sensitive,
        canonical_root: CanonicalRoot::Sensitive,
        canonical: "{key}.env",
        legacy: Some(LegacyLocation::new(LegacyRoot::AppConfig, "{key}.env")),
        pinned: None,
        may_contain_secrets: true,
        example_key: "openai",
    },
    ExternalClaudeCredentials {
        marker: External,
        key: (),
        tier: ArtifactTier::ExternalSecret,
        canonical_root: CanonicalRoot::ExternalSecret,
        canonical: ".claude/.credentials.json",
        legacy: Some(LegacyLocation::new(LegacyRoot::UserHome, ".claude/.credentials.json")),
        pinned: None,
        may_contain_secrets: true,
        example_key: "",
    },
    ExternalOpenCodeCredentials {
        marker: External,
        key: (),
        tier: ArtifactTier::ExternalSecret,
        canonical_root: CanonicalRoot::ExternalSecret,
        canonical: ".local/share/opencode/auth.json",
        legacy: Some(LegacyLocation::new(LegacyRoot::UserHome, ".local/share/opencode/auth.json")),
        pinned: None,
        may_contain_secrets: true,
        example_key: "",
    },
    ConfigToml {
        marker: Durable,
        key: (),
        tier: ArtifactTier::Durable(DurableKind::State),
        canonical_root: CanonicalRoot::DurableState,
        canonical: "config.toml",
        legacy: Some(LegacyLocation::new(LegacyRoot::Jcode, "config.toml")),
        pinned: Some(CONFIG_PIN),
        may_contain_secrets: true,
        example_key: "",
    },
    ProviderActivity {
        marker: Durable,
        key: (),
        tier: ArtifactTier::Durable(DurableKind::State),
        canonical_root: CanonicalRoot::DurableState,
        canonical: "provider_activity.json",
        legacy: Some(LegacyLocation::new(LegacyRoot::Jcode, "provider_activity.json")),
        pinned: None,
        may_contain_secrets: false,
        example_key: "",
    },
    AuthRefreshState {
        marker: Durable,
        key: (),
        tier: ArtifactTier::Durable(DurableKind::State),
        canonical_root: CanonicalRoot::DurableState,
        canonical: "auth-refresh-state.json",
        legacy: Some(LegacyLocation::new(LegacyRoot::Jcode, "auth-refresh-state.json")),
        pinned: None,
        may_contain_secrets: true,
        example_key: "",
    },
    AmbientState {
        marker: Durable,
        key: (),
        tier: ArtifactTier::Durable(DurableKind::State),
        canonical_root: CanonicalRoot::DurableState,
        canonical: "ambient/state.json",
        legacy: Some(LegacyLocation::new(LegacyRoot::Jcode, "ambient/state.json")),
        pinned: None,
        may_contain_secrets: false,
        example_key: "",
    },
    AmbientQueue {
        marker: Durable,
        key: (),
        tier: ArtifactTier::Durable(DurableKind::State),
        canonical_root: CanonicalRoot::DurableState,
        canonical: "ambient/queue.json",
        legacy: Some(LegacyLocation::new(LegacyRoot::Jcode, "ambient/queue.json")),
        pinned: None,
        may_contain_secrets: false,
        example_key: "",
    },
    AmbientTranscripts {
        marker: Durable,
        key: String,
        tier: ArtifactTier::Durable(DurableKind::State),
        canonical_root: CanonicalRoot::DurableState,
        canonical: "ambient/transcripts/{key}",
        legacy: Some(LegacyLocation::new(LegacyRoot::Jcode, "ambient/transcripts/{key}")),
        pinned: None,
        may_contain_secrets: false,
        example_key: "example.jsonl",
    },
    PendingSoftInterrupt {
        marker: Durable,
        key: String,
        tier: ArtifactTier::Durable(DurableKind::Inbox),
        canonical_root: CanonicalRoot::DurableInbox,
        canonical: "soft-interrupts/{key}.json",
        legacy: Some(LegacyLocation::new(LegacyRoot::Jcode, "pending-soft-interrupts/{key}.json")),
        pinned: None,
        may_contain_secrets: false,
        example_key: "session",
    },
    SessionInboxItem {
        marker: Durable,
        key: SessionInboxId,
        tier: ArtifactTier::Durable(DurableKind::Inbox),
        canonical_root: CanonicalRoot::JcodeHomeInbox,
        canonical: "{key}",
        legacy: None,
        pinned: None,
        may_contain_secrets: false,
        example_key: "session",
    },
    LegacySessionRoot {
        marker: Durable,
        key: (),
        tier: ArtifactTier::Durable(DurableKind::State),
        canonical_root: CanonicalRoot::DurableState,
        canonical: "sessions",
        legacy: Some(LegacyLocation::new(LegacyRoot::Jcode, "sessions")),
        pinned: Some(LEGACY_SESSION_ROOT_PIN),
        may_contain_secrets: true,
        example_key: "",
    },
    SwarmState {
        marker: Durable,
        key: String,
        tier: ArtifactTier::Durable(DurableKind::State),
        canonical_root: CanonicalRoot::DurableState,
        canonical: "swarm/{key}.json",
        legacy: Some(LegacyLocation::new(LegacyRoot::DurableState, "swarm/{key}.json")),
        pinned: None,
        may_contain_secrets: false,
        example_key: "swarm",
    },
    SwarmControlLog {
        marker: Durable,
        key: String,
        tier: ArtifactTier::Durable(DurableKind::State),
        canonical_root: CanonicalRoot::DurableState,
        canonical: "swarm/{key}.control.jsonl",
        legacy: Some(LegacyLocation::new(LegacyRoot::DurableState, "swarm/{key}.control.jsonl")),
        pinned: None,
        may_contain_secrets: false,
        example_key: "swarm",
    },
    ServerBeacon {
        marker: Durable,
        key: (),
        tier: ArtifactTier::Durable(DurableKind::State),
        canonical_root: CanonicalRoot::DurableState,
        canonical: "server-beacon.json",
        legacy: Some(LegacyLocation::new(LegacyRoot::DurableState, "server-beacon.json")),
        pinned: None,
        may_contain_secrets: false,
        example_key: "",
    },
    DeliveryCampaign {
        marker: Durable,
        key: (),
        tier: ArtifactTier::Durable(DurableKind::State),
        canonical_root: CanonicalRoot::DurableState,
        canonical: "delivery-campaign",
        legacy: Some(LegacyLocation::new(LegacyRoot::Jcode, "delivery-campaign")),
        pinned: None,
        may_contain_secrets: false,
        example_key: "",
    },
    BackgroundTaskStatus {
        marker: Durable,
        key: String,
        tier: ArtifactTier::Durable(DurableKind::State),
        canonical_root: CanonicalRoot::DurableState,
        canonical: "background-tasks/{key}.status.json",
        legacy: Some(LegacyLocation::new(LegacyRoot::SystemTemp, "jcode-bg-tasks/{key}.status.json")),
        pinned: None,
        may_contain_secrets: false,
        example_key: "task",
    },
    DebugControl {
        marker: Durable,
        key: (),
        tier: ArtifactTier::Durable(DurableKind::State),
        canonical_root: CanonicalRoot::DurableState,
        canonical: "debug_control",
        legacy: Some(LegacyLocation::new(LegacyRoot::Jcode, "debug_control")),
        pinned: None,
        may_contain_secrets: false,
        example_key: "",
    },
    LastFocusedClientSession {
        marker: Durable,
        key: (),
        tier: ArtifactTier::Durable(DurableKind::State),
        canonical_root: CanonicalRoot::DurableState,
        canonical: "last_focused_client_session",
        legacy: Some(LegacyLocation::new(LegacyRoot::Jcode, "last_focused_client_session")),
        pinned: None,
        may_contain_secrets: false,
        example_key: "",
    },
    LastSeenChangelog {
        marker: Durable,
        key: (),
        tier: ArtifactTier::Durable(DurableKind::State),
        canonical_root: CanonicalRoot::DurableState,
        canonical: "last_seen_changelog",
        legacy: Some(LegacyLocation::new(LegacyRoot::Jcode, "last_seen_changelog")),
        pinned: None,
        may_contain_secrets: false,
        example_key: "",
    },
    ModelCatalogCache {
        marker: Cache,
        key: ModelCatalogId,
        tier: ArtifactTier::Temporary(TemporaryKind::Cache),
        canonical_root: CanonicalRoot::TemporaryCache,
        canonical: "model-catalogs/{key}",
        legacy: Some(LegacyLocation::new(LegacyRoot::AppConfig, "{key}")),
        pinned: None,
        may_contain_secrets: false,
        example_key: "openai_model_catalog_cache.json",
    },
    ModelCache {
        marker: Cache,
        key: String,
        tier: ArtifactTier::Temporary(TemporaryKind::Cache),
        canonical_root: CanonicalRoot::TemporaryCache,
        canonical: "models/{key}",
        legacy: Some(LegacyLocation::new(LegacyRoot::Jcode, "cache/{key}")),
        pinned: None,
        may_contain_secrets: false,
        example_key: "models.json",
    },
    EndpointCache {
        marker: Cache,
        key: String,
        tier: ArtifactTier::Temporary(TemporaryKind::Cache),
        canonical_root: CanonicalRoot::TemporaryCache,
        canonical: "endpoints/{key}",
        legacy: Some(LegacyLocation::new(LegacyRoot::Jcode, "cache/{key}")),
        pinned: None,
        may_contain_secrets: false,
        example_key: "endpoints.json",
    },
    SessionSearchCache {
        marker: Cache,
        key: String,
        tier: ArtifactTier::Temporary(TemporaryKind::Cache),
        canonical_root: CanonicalRoot::TemporaryCache,
        canonical: "session-search/{key}",
        legacy: Some(LegacyLocation::new(LegacyRoot::Jcode, "cache/{key}")),
        pinned: None,
        may_contain_secrets: false,
        example_key: "session_search_jcode_index_v2.bin",
    },
    SessionPickerListCache {
        marker: Cache,
        key: (),
        tier: ArtifactTier::Temporary(TemporaryKind::Cache),
        canonical_root: CanonicalRoot::TemporaryCache,
        canonical: "session-picker/session-picker-list-v1.json",
        legacy: Some(LegacyLocation::new(LegacyRoot::Jcode, "cache/session-picker-list-v1.json")),
        pinned: None,
        may_contain_secrets: false,
        example_key: "",
    },
    MermaidRender {
        marker: Cache,
        key: String,
        tier: ArtifactTier::Temporary(TemporaryKind::Cache),
        canonical_root: CanonicalRoot::TemporaryCache,
        canonical: "mermaid/{key}",
        legacy: Some(LegacyLocation::new(LegacyRoot::AppCache, "mermaid/{key}")),
        pinned: None,
        may_contain_secrets: false,
        example_key: "render.png",
    },
    LatexRender {
        marker: Cache,
        key: String,
        tier: ArtifactTier::Temporary(TemporaryKind::Cache),
        canonical_root: CanonicalRoot::TemporaryCache,
        canonical: "latex/{key}",
        legacy: Some(LegacyLocation::new(LegacyRoot::AppCache, "latex/{key}")),
        pinned: None,
        may_contain_secrets: false,
        example_key: "render.png",
    },
    ClientInputScratch {
        marker: Scratch,
        key: String,
        tier: ArtifactTier::Temporary(TemporaryKind::Scratch),
        canonical_root: CanonicalRoot::TemporaryScratch,
        canonical: "client-input/{key}",
        legacy: Some(LegacyLocation::new(LegacyRoot::Jcode, "client-input-session_{key}")),
        pinned: None,
        may_contain_secrets: false,
        example_key: "session",
    },
    VisualDebug {
        marker: Scratch,
        key: (),
        tier: ArtifactTier::Temporary(TemporaryKind::Scratch),
        canonical_root: CanonicalRoot::TemporaryScratch,
        canonical: "visual-debug.txt",
        legacy: Some(LegacyLocation::new(LegacyRoot::AppConfig, "visual-debug.txt")),
        pinned: None,
        may_contain_secrets: false,
        example_key: "",
    },
    LiveTestCoverage {
        marker: Scratch,
        key: (),
        tier: ArtifactTier::Temporary(TemporaryKind::Scratch),
        canonical_root: CanonicalRoot::TemporaryScratch,
        canonical: "live-tests/coverage.json",
        legacy: Some(LegacyLocation::new(LegacyRoot::AppConfig, "live-tests/coverage.json")),
        pinned: None,
        may_contain_secrets: false,
        example_key: "",
    },
    LiveTestEvents {
        marker: Scratch,
        key: (),
        tier: ArtifactTier::Temporary(TemporaryKind::Scratch),
        canonical_root: CanonicalRoot::TemporaryScratch,
        canonical: "live-tests/events.jsonl",
        legacy: Some(LegacyLocation::new(LegacyRoot::AppConfig, "live-tests/events.jsonl")),
        pinned: None,
        may_contain_secrets: false,
        example_key: "",
    },
    SessionLock {
        marker: Lock,
        key: String,
        tier: ArtifactTier::Temporary(TemporaryKind::Lock),
        canonical_root: CanonicalRoot::TemporaryLock,
        canonical: "active-pids/{key}",
        legacy: Some(LegacyLocation::new(LegacyRoot::Jcode, "active_pids/{key}")),
        pinned: Some(SESSION_LOCK_PIN),
        may_contain_secrets: false,
        example_key: "session",
    },
    ConfigLock {
        marker: Lock,
        key: (),
        tier: ArtifactTier::Temporary(TemporaryKind::Lock),
        canonical_root: CanonicalRoot::TemporaryLock,
        canonical: "config.toml.lock",
        legacy: Some(LegacyLocation::new(LegacyRoot::Jcode, "config.toml.lock")),
        pinned: None,
        may_contain_secrets: false,
        example_key: "",
    },
    PidMarkerLock {
        marker: Lock,
        key: (),
        tier: ArtifactTier::Temporary(TemporaryKind::Lock),
        canonical_root: CanonicalRoot::TemporaryLock,
        canonical: ".pid-markers.lock",
        legacy: Some(LegacyLocation::new(LegacyRoot::Jcode, ".pid-markers.lock")),
        pinned: None,
        may_contain_secrets: false,
        example_key: "",
    },
    GrokDirectCredentialsLock {
        marker: Lock,
        key: (),
        tier: ArtifactTier::Temporary(TemporaryKind::Lock),
        canonical_root: CanonicalRoot::TemporaryLock,
        canonical: "grok-direct/credentials.lock",
        legacy: Some(LegacyLocation::new(LegacyRoot::AppConfig, "grok-direct/credentials.lock")),
        pinned: None,
        may_contain_secrets: false,
        example_key: "",
    },
    DaemonLock {
        marker: Lock,
        key: (),
        tier: ArtifactTier::Temporary(TemporaryKind::Lock),
        canonical_root: CanonicalRoot::TemporaryLock,
        canonical: "jcode-daemon.lock",
        legacy: Some(LegacyLocation::new(LegacyRoot::Runtime, "jcode-daemon.lock")),
        pinned: None,
        may_contain_secrets: false,
        example_key: "",
    },
    DaemonSocket {
        marker: Scratch,
        key: (),
        tier: ArtifactTier::Temporary(TemporaryKind::Scratch),
        canonical_root: CanonicalRoot::TemporaryScratch,
        canonical: "jcode.sock",
        legacy: Some(LegacyLocation::new(LegacyRoot::DaemonSocket, "")),
        pinned: Some(DAEMON_SOCKET_PIN),
        may_contain_secrets: false,
        example_key: "",
    },
}
