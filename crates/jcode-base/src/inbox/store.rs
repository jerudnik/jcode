use super::{
    CURRENT_SCHEMA_VERSION, DeliveryPolicy, InboxClass, InboxEnvelope, InboxItemDraft, InboxItemId,
    InboxState,
};
use anyhow::{Context, Result, bail};
use jcode_storage::{SessionInboxId, durable_path, tag};
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

pub const DEFAULT_PAYLOAD_BYTE_CAP: u64 = 256 * 1024;
pub const DEFAULT_SESSION_COUNT_CAP: usize = 256;
pub const DEFAULT_SESSION_BYTE_CAP: u64 = 4 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InboxBounds {
    pub payload_byte_cap: u64,
    pub per_session_count_cap: usize,
    pub per_session_byte_cap: u64,
}

impl Default for InboxBounds {
    fn default() -> Self {
        Self {
            payload_byte_cap: DEFAULT_PAYLOAD_BYTE_CAP,
            per_session_count_cap: DEFAULT_SESSION_COUNT_CAP,
            per_session_byte_cap: DEFAULT_SESSION_BYTE_CAP,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub initial_backoff_ms: u64,
    pub max_backoff_ms: u64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 5,
            initial_backoff_ms: 1_000,
            max_backoff_ms: 60_000,
        }
    }
}

impl RetryPolicy {
    pub fn backoff_ms(&self, attempts: u32) -> u64 {
        let exponent = attempts.saturating_sub(1).min(63);
        self.initial_backoff_ms
            .saturating_mul(1_u64 << exponent)
            .min(self.max_backoff_ms)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetentionPolicy {
    pub acked_ms: u64,
    pub cancelled_ms: u64,
    pub expired_ms: u64,
    pub undeliverable_ms: u64,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        const SIX_HOURS_MS: u64 = 6 * 60 * 60 * 1_000;
        const DAY_MS: u64 = 24 * 60 * 60 * 1_000;
        Self {
            acked_ms: SIX_HOURS_MS,
            cancelled_ms: SIX_HOURS_MS,
            expired_ms: DAY_MS,
            undeliverable_ms: DAY_MS,
        }
    }
}

impl RetentionPolicy {
    fn ttl_for(&self, state: InboxState) -> Option<u64> {
        match state {
            InboxState::Acked => Some(self.acked_ms),
            InboxState::Cancelled => Some(self.cancelled_ms),
            InboxState::Expired => Some(self.expired_ms),
            InboxState::Undeliverable => Some(self.undeliverable_ms),
            InboxState::Pending
            | InboxState::Due
            | InboxState::Attempting
            | InboxState::DeliveredUnacked => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InboxTransition {
    MarkDue,
    BeginAttempt,
    AttemptFailed,
    DeliveryAccepted,
    Ack,
    Cancel,
    Expire,
    MarkUndeliverable,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct QuarantinedItem {
    pub original_path: PathBuf,
    pub quarantine_path: PathBuf,
    pub reason: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct InboxLoad {
    pub items: Vec<InboxEnvelope>,
    pub quarantined: Vec<QuarantinedItem>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RetentionSweep {
    pub removed: usize,
    pub quarantined: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct InboxDebugItem {
    pub schema_version: u32,
    pub inbox_item_id: InboxItemId,
    pub class: InboxClass,
    pub target_session_id: String,
    pub swarm_id: Option<String>,
    pub delivery_policy: DeliveryPolicy,
    pub created_at: u64,
    pub due_at: u64,
    pub expires_at: u64,
    pub attempts: u32,
    pub state: InboxState,
    pub state_changed_at: u64,
    pub payload_bytes: u64,
    pub payload: &'static str,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct InboxDebugSnapshot {
    pub items: Vec<InboxDebugItem>,
    pub quarantined: Vec<QuarantinedItem>,
}

#[derive(Clone, Debug)]
pub struct InboxStore {
    root: PathBuf,
    bounds: InboxBounds,
}

impl InboxStore {
    pub fn new(bounds: InboxBounds) -> Result<Self> {
        let root = durable_path::<tag::DurableInbox>(())?.into_path_buf();
        Ok(Self { root, bounds })
    }

    pub fn with_root(root: PathBuf, bounds: InboxBounds) -> Self {
        Self { root, bounds }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn enqueue(&self, draft: InboxItemDraft, now_ms: u64) -> Result<InboxItemId> {
        // Mint before validation, accounting, or mutable delivery state. A
        // rejected enqueue burns an ID rather than ever reusing identity.
        let inbox_item_id = InboxItemId::mint(now_ms);
        validate_session_id(&draft.target_session_id)?;
        let payload_bytes = serialized_payload_bytes(&draft.payload)?;
        if payload_bytes > self.bounds.payload_byte_cap {
            bail!(
                "inbox payload byte cap {} exceeded: {} bytes",
                self.bounds.payload_byte_cap,
                payload_bytes
            );
        }

        let envelope = draft.into_envelope(inbox_item_id, now_ms);
        let session_load = self.load_session(&envelope.target_session_id)?;
        if session_load.items.len() >= self.bounds.per_session_count_cap {
            bail!(
                "inbox per-session count cap {} exceeded for session {}",
                self.bounds.per_session_count_cap,
                envelope.target_session_id
            );
        }

        let bytes = serde_json::to_vec(&envelope)?;
        let current_bytes = session_load.items.iter().try_fold(0_u64, |total, item| {
            Ok::<u64, anyhow::Error>(total.saturating_add(serde_json::to_vec(item)?.len() as u64))
        })?;
        let projected_bytes = current_bytes.saturating_add(bytes.len() as u64);
        if projected_bytes > self.bounds.per_session_byte_cap {
            bail!(
                "inbox per-session byte cap {} exceeded for session {}: {} bytes",
                self.bounds.per_session_byte_cap,
                envelope.target_session_id,
                projected_bytes
            );
        }

        let path = self.item_path(&envelope.target_session_id, &envelope.inbox_item_id)?;
        persist_new_item(&path, &bytes)?;

        // Do not expose the ID until the durable name is visible and parseable.
        let persisted = self
            .load(&envelope.target_session_id, &envelope.inbox_item_id)?
            .context("newly enqueued inbox item was not readable after atomic rename")?;
        if persisted.inbox_item_id != envelope.inbox_item_id {
            bail!("newly enqueued inbox item identity changed on disk");
        }
        Ok(envelope.inbox_item_id)
    }

    pub fn load(&self, session_id: &str, item_id: &InboxItemId) -> Result<Option<InboxEnvelope>> {
        let path = self.item_path(session_id, item_id)?;
        if !path.exists() {
            return Ok(None);
        }
        match self.read_item(&path, session_id) {
            Ok(item) => Ok(Some(item)),
            Err(error) => {
                self.quarantine(&path, error.to_string())?;
                Ok(None)
            }
        }
    }

    pub fn load_session(&self, session_id: &str) -> Result<InboxLoad> {
        validate_session_id(session_id)?;
        let directory = self.session_dir(session_id)?;
        if !directory.exists() {
            return Ok(InboxLoad::default());
        }

        let mut load = InboxLoad::default();
        for entry in fs::read_dir(&directory)? {
            let entry = entry?;
            if !entry.file_type()?.is_file()
                || entry.path().extension().and_then(|v| v.to_str()) != Some("json")
            {
                continue;
            }
            let path = entry.path();
            match self.read_item(&path, session_id) {
                Ok(item) => load.items.push(item),
                Err(error) => load
                    .quarantined
                    .push(self.quarantine(&path, error.to_string())?),
            }
        }
        load.items.sort_by(|left, right| {
            (left.created_at, &left.inbox_item_id).cmp(&(right.created_at, &right.inbox_item_id))
        });
        Ok(load)
    }

    pub fn load_all(&self) -> Result<InboxLoad> {
        let items_root = self.items_root();
        if !items_root.exists() {
            return Ok(InboxLoad::default());
        }
        let mut all = InboxLoad::default();
        for entry in fs::read_dir(items_root)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let session_id = entry.file_name().to_string_lossy().into_owned();
            let mut loaded = self.load_session(&session_id)?;
            all.items.append(&mut loaded.items);
            all.quarantined.append(&mut loaded.quarantined);
        }
        all.items.sort_by(|left, right| {
            (left.created_at, &left.inbox_item_id).cmp(&(right.created_at, &right.inbox_item_id))
        });
        Ok(all)
    }

    pub fn transition(
        &self,
        session_id: &str,
        item_id: &InboxItemId,
        transition: InboxTransition,
        now_ms: u64,
        retry: RetryPolicy,
    ) -> Result<InboxEnvelope> {
        let mut item = self
            .load(session_id, item_id)?
            .with_context(|| format!("inbox item {item_id} does not exist"))?;
        apply_transition(&mut item, transition, now_ms, retry)?;
        let path = self.item_path(session_id, item_id)?;
        write_item_atomic(&path, &serde_json::to_vec(&item)?)?;
        Ok(item)
    }

    pub fn sweep_retention(
        &self,
        now_ms: u64,
        retention: RetentionPolicy,
    ) -> Result<RetentionSweep> {
        let loaded = self.load_all()?;
        let mut sweep = RetentionSweep {
            quarantined: loaded.quarantined.len(),
            ..RetentionSweep::default()
        };
        for item in loaded.items {
            let Some(ttl) = retention.ttl_for(item.state) else {
                continue;
            };
            if now_ms.saturating_sub(item.state_changed_at) < ttl {
                continue;
            }
            let path = self.item_path(&item.target_session_id, &item.inbox_item_id)?;
            match fs::remove_file(path) {
                Ok(()) => sweep.removed += 1,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        Ok(sweep)
    }

    pub fn debug_snapshot(&self, session_id: Option<&str>) -> Result<InboxDebugSnapshot> {
        let loaded = match session_id {
            Some(session_id) => self.load_session(session_id)?,
            None => self.load_all()?,
        };
        let items = loaded
            .items
            .into_iter()
            .map(|item| {
                let payload_bytes = serialized_payload_bytes(&item.payload).unwrap_or_default();
                InboxDebugItem {
                    schema_version: item.schema_version,
                    inbox_item_id: item.inbox_item_id,
                    class: item.class,
                    target_session_id: item.target_session_id,
                    swarm_id: item.swarm_id,
                    delivery_policy: item.delivery_policy,
                    created_at: item.created_at,
                    due_at: item.due_at,
                    expires_at: item.expires_at,
                    attempts: item.attempts,
                    state: item.state,
                    state_changed_at: item.state_changed_at,
                    payload_bytes,
                    payload: "[REDACTED]",
                }
            })
            .collect();
        Ok(InboxDebugSnapshot {
            items,
            quarantined: loaded.quarantined,
        })
    }

    fn read_item(&self, path: &Path, expected_session: &str) -> Result<InboxEnvelope> {
        let file_bytes = fs::metadata(path)?.len();
        if file_bytes > self.bounds.per_session_byte_cap {
            bail!(
                "inbox item file exceeds per-session byte cap {}: {} bytes",
                self.bounds.per_session_byte_cap,
                file_bytes
            );
        }
        let bytes = fs::read(path)?;
        let value: serde_json::Value = serde_json::from_slice(&bytes)?;
        let schema_version = value
            .get("schema_version")
            .and_then(serde_json::Value::as_u64)
            .context("inbox item is missing numeric schema_version")?;
        if schema_version > u64::from(CURRENT_SCHEMA_VERSION) {
            bail!("unsupported future inbox schema version {schema_version}");
        }
        let item = match schema_version {
            1 => migrate_v1(serde_json::from_value(value)?),
            version if version == u64::from(CURRENT_SCHEMA_VERSION) => {
                serde_json::from_value(value)?
            }
            version => bail!("unsupported inbox schema version {version}"),
        };
        if item.target_session_id != expected_session {
            bail!(
                "inbox target session {} does not match directory {}",
                item.target_session_id,
                expected_session
            );
        }
        let expected_name = format!("{}.json", item.inbox_item_id);
        if path.file_name().and_then(|name| name.to_str()) != Some(expected_name.as_str()) {
            bail!("inbox item ID does not match its filename");
        }
        let payload_bytes = serialized_payload_bytes(&item.payload)?;
        if payload_bytes > self.bounds.payload_byte_cap {
            bail!(
                "inbox payload byte cap {} exceeded: {} bytes",
                self.bounds.payload_byte_cap,
                payload_bytes
            );
        }
        Ok(item)
    }

    fn quarantine(&self, path: &Path, reason: String) -> Result<QuarantinedItem> {
        let session = path
            .parent()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            .unwrap_or("unknown");
        let quarantine_dir = self.root.join("quarantine").join(session);
        fs::create_dir_all(&quarantine_dir)?;
        let filename = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("poison");
        let quarantine_path = quarantine_dir.join(format!(
            "{}.{}.quarantine",
            filename,
            uuid::Uuid::new_v4().simple()
        ));
        fs::rename(path, &quarantine_path).with_context(|| {
            format!(
                "quarantine inbox item {} to {}",
                path.display(),
                quarantine_path.display()
            )
        })?;
        Ok(QuarantinedItem {
            original_path: path.to_path_buf(),
            quarantine_path,
            reason,
        })
    }

    fn items_root(&self) -> PathBuf {
        self.root.join("items")
    }

    fn session_dir(&self, session_id: &str) -> Result<PathBuf> {
        let session_id = SessionInboxId::new(session_id.to_string())?;
        Ok(self.items_root().join(session_id.as_str()))
    }

    fn item_path(&self, session_id: &str, item_id: &InboxItemId) -> Result<PathBuf> {
        Ok(self
            .session_dir(session_id)?
            .join(format!("{item_id}.json")))
    }
}

fn validate_session_id(session_id: &str) -> Result<()> {
    SessionInboxId::new(session_id.to_string()).map(|_| ())
}

fn serialized_payload_bytes(payload: &serde_json::Value) -> Result<u64> {
    Ok(serde_json::to_vec(payload)?.len() as u64)
}

fn persist_new_item(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    if path.exists() {
        bail!(
            "refusing to overwrite existing inbox item {}",
            path.display()
        );
    }

    let temp_path = path.with_extension(format!(
        "json.tmp.{}.{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    let result = (|| -> Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&temp_path, path)?;
        sync_parent(path);
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    result
}

fn write_item_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temp_path = path.with_extension(format!(
        "json.tmp.{}.{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    let result = (|| -> Result<()> {
        let mut file = File::create(&temp_path)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&temp_path, path)?;
        sync_parent(path);
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    result
}

fn sync_parent(path: &Path) {
    #[cfg(unix)]
    if let Some(parent) = path.parent()
        && let Ok(directory) = File::open(parent)
    {
        let _ = directory.sync_all();
    }
}

fn apply_transition(
    item: &mut InboxEnvelope,
    transition: InboxTransition,
    now_ms: u64,
    retry: RetryPolicy,
) -> Result<()> {
    if item.state.is_terminal() {
        bail!(
            "cannot transition terminal inbox item from {:?}",
            item.state
        );
    }
    if now_ms >= item.expires_at && transition != InboxTransition::Cancel {
        item.state = InboxState::Expired;
        item.state_changed_at = now_ms;
        return Ok(());
    }

    match (item.state, transition) {
        (InboxState::Pending, InboxTransition::MarkDue) if now_ms >= item.due_at => {
            item.state = InboxState::Due;
        }
        (InboxState::Due, InboxTransition::BeginAttempt) => {
            if item.attempts >= retry.max_attempts {
                item.state = InboxState::Undeliverable;
            } else {
                item.attempts = item.attempts.saturating_add(1);
                item.state = InboxState::Attempting;
            }
        }
        (InboxState::Attempting, InboxTransition::AttemptFailed) => {
            if item.attempts >= retry.max_attempts {
                item.state = InboxState::Undeliverable;
            } else {
                item.due_at = now_ms.saturating_add(retry.backoff_ms(item.attempts));
                item.state = InboxState::Pending;
            }
        }
        (InboxState::Attempting, InboxTransition::DeliveryAccepted) => {
            item.state = InboxState::DeliveredUnacked;
        }
        (InboxState::DeliveredUnacked, InboxTransition::Ack) => {
            item.state = InboxState::Acked;
        }
        (_, InboxTransition::Cancel) => {
            item.state = InboxState::Cancelled;
        }
        (_, InboxTransition::Expire) if now_ms >= item.expires_at => {
            item.state = InboxState::Expired;
        }
        (_, InboxTransition::MarkUndeliverable) => {
            item.state = InboxState::Undeliverable;
        }
        (state, operation) => {
            bail!("invalid inbox transition {operation:?} from {state:?}");
        }
    }
    item.state_changed_at = now_ms;
    Ok(())
}

#[derive(Deserialize)]
struct InboxEnvelopeV1 {
    inbox_item_id: InboxItemId,
    class: InboxClass,
    target_session_id: String,
    #[serde(default)]
    swarm_id: Option<String>,
    payload: serde_json::Value,
    #[serde(default)]
    delivery_policy: DeliveryPolicy,
    created_at: u64,
    due_at: u64,
    #[serde(default)]
    attempts: u32,
    #[serde(default = "pending_state")]
    state: InboxState,
}

fn pending_state() -> InboxState {
    InboxState::Pending
}

fn migrate_v1(item: InboxEnvelopeV1) -> InboxEnvelope {
    const LEGACY_TTL_MS: u64 = 24 * 60 * 60 * 1_000;
    InboxEnvelope {
        schema_version: CURRENT_SCHEMA_VERSION,
        inbox_item_id: item.inbox_item_id,
        class: item.class,
        target_session_id: item.target_session_id,
        swarm_id: item.swarm_id,
        payload: item.payload,
        delivery_policy: item.delivery_policy,
        created_at: item.created_at,
        due_at: item.due_at,
        expires_at: item.created_at.saturating_add(LEGACY_TTL_MS),
        attempts: item.attempts,
        state: item.state,
        state_changed_at: item.created_at,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn draft(
        session: &str,
        payload: serde_json::Value,
        due_at: u64,
        ttl_ms: u64,
    ) -> InboxItemDraft {
        InboxItemDraft {
            class: InboxClass::BackgroundCompletion,
            target_session_id: session.to_string(),
            swarm_id: Some("swarm-1".to_string()),
            payload,
            delivery_policy: DeliveryPolicy::default(),
            due_at,
            ttl_ms,
        }
    }

    fn store(bounds: InboxBounds) -> (tempfile::TempDir, InboxStore) {
        let temp = tempfile::tempdir().unwrap();
        let store = InboxStore::with_root(temp.path().join("inbox"), bounds);
        (temp, store)
    }

    #[test]
    fn enqueue_returns_only_after_atomic_rename_is_visible_to_second_handle() {
        let (_temp, first) = store(InboxBounds::default());
        let second = InboxStore::with_root(first.root().to_path_buf(), InboxBounds::default());
        let id = first
            .enqueue(draft("session-a", json!({"body": "ready"}), 10, 1_000), 10)
            .unwrap();

        let loaded = second.load("session-a", &id).unwrap().unwrap();
        assert_eq!(loaded.inbox_item_id, id);
        assert_eq!(loaded.payload, json!({"body": "ready"}));
    }

    #[test]
    fn retry_and_ttl_table_uses_caller_supplied_clock() {
        let (_temp, store) = store(InboxBounds::default());
        let retry = RetryPolicy {
            max_attempts: 3,
            initial_backoff_ms: 10,
            max_backoff_ms: 25,
        };
        let id = store
            .enqueue(draft("session-a", json!("payload"), 100, 1_000), 100)
            .unwrap();

        let due = store
            .transition("session-a", &id, InboxTransition::MarkDue, 100, retry)
            .unwrap();
        assert_eq!(due.state, InboxState::Due);

        let attempt_1 = store
            .transition("session-a", &id, InboxTransition::BeginAttempt, 100, retry)
            .unwrap();
        assert_eq!(attempt_1.attempts, 1);
        let retry_1 = store
            .transition("session-a", &id, InboxTransition::AttemptFailed, 101, retry)
            .unwrap();
        assert_eq!((retry_1.state, retry_1.due_at), (InboxState::Pending, 111));

        for (due_at, failed_at, expected_next) in [(111, 112, 132), (132, 133, 133)] {
            store
                .transition("session-a", &id, InboxTransition::MarkDue, due_at, retry)
                .unwrap();
            store
                .transition(
                    "session-a",
                    &id,
                    InboxTransition::BeginAttempt,
                    due_at,
                    retry,
                )
                .unwrap();
            let failed = store
                .transition(
                    "session-a",
                    &id,
                    InboxTransition::AttemptFailed,
                    failed_at,
                    retry,
                )
                .unwrap();
            if failed.state == InboxState::Pending {
                assert_eq!(failed.due_at, expected_next);
            } else {
                assert_eq!(failed.state, InboxState::Undeliverable);
            }
        }
        assert_eq!(store.load("session-a", &id).unwrap().unwrap().attempts, 3);

        let expiry_id = store
            .enqueue(draft("session-a", json!("expires"), 200, 5), 200)
            .unwrap();
        let expired = store
            .transition(
                "session-a",
                &expiry_id,
                InboxTransition::MarkDue,
                205,
                retry,
            )
            .unwrap();
        assert_eq!(expired.state, InboxState::Expired);
    }

    #[test]
    fn state_machine_reaches_delivered_unacked_then_acked() {
        let (_temp, store) = store(InboxBounds::default());
        let retry = RetryPolicy::default();
        let id = store
            .enqueue(draft("session-a", json!("payload"), 100, 1_000), 100)
            .unwrap();
        store
            .transition("session-a", &id, InboxTransition::MarkDue, 100, retry)
            .unwrap();
        store
            .transition("session-a", &id, InboxTransition::BeginAttempt, 100, retry)
            .unwrap();
        let delivered = store
            .transition(
                "session-a",
                &id,
                InboxTransition::DeliveryAccepted,
                101,
                retry,
            )
            .unwrap();
        assert_eq!(delivered.state, InboxState::DeliveredUnacked);
        let acked = store
            .transition("session-a", &id, InboxTransition::Ack, 102, retry)
            .unwrap();
        assert_eq!(acked.state, InboxState::Acked);
        assert!(
            store
                .transition("session-a", &id, InboxTransition::Cancel, 103, retry)
                .unwrap_err()
                .to_string()
                .contains("terminal inbox item")
        );
    }

    #[test]
    fn v1_loads_and_future_schema_is_quarantined() {
        let (_temp, store) = store(InboxBounds::default());
        let session_dir = store.session_dir("session-a").unwrap();
        fs::create_dir_all(&session_dir).unwrap();
        let v1_id = InboxItemId::mint(10);
        fs::write(
            session_dir.join(format!("{v1_id}.json")),
            serde_json::to_vec(&json!({
                "schema_version": 1,
                "inbox_item_id": v1_id,
                "class": "dm",
                "target_session_id": "session-a",
                "payload": {"body": "legacy"},
                "created_at": 10,
                "due_at": 11
            }))
            .unwrap(),
        )
        .unwrap();
        let future_id = InboxItemId::mint(20);
        let future_path = session_dir.join(format!("{future_id}.json"));
        fs::write(
            &future_path,
            serde_json::to_vec(&json!({
                "schema_version": CURRENT_SCHEMA_VERSION + 1,
                "inbox_item_id": future_id,
                "class": "dm",
                "target_session_id": "session-a",
                "payload": {"body": "future"}
            }))
            .unwrap(),
        )
        .unwrap();

        let loaded = store.load_session("session-a").unwrap();
        assert_eq!(loaded.items.len(), 1);
        assert_eq!(loaded.items[0].schema_version, CURRENT_SCHEMA_VERSION);
        assert_eq!(loaded.quarantined.len(), 1);
        assert!(!future_path.exists());
        assert!(loaded.quarantined[0].quarantine_path.exists());
        assert!(loaded.quarantined[0].reason.contains("future inbox schema"));
    }

    #[test]
    fn per_session_byte_cap_names_cap_and_poison_does_not_block_queue() {
        let bounds = InboxBounds {
            payload_byte_cap: 1_024,
            per_session_count_cap: 10,
            per_session_byte_cap: 350,
        };
        let (_temp, store) = store(bounds);
        let first = store
            .enqueue(draft("session-a", json!({"body": "small"}), 10, 1_000), 10)
            .unwrap();
        let error = store
            .enqueue(
                draft("session-a", json!({"body": "x".repeat(300)}), 11, 1_000),
                11,
            )
            .unwrap_err();
        assert!(error.to_string().contains("per-session byte cap 350"));

        let poison_path = store.session_dir("session-a").unwrap().join("poison.json");
        fs::write(&poison_path, b"not json").unwrap();
        let loaded = store.load_session("session-a").unwrap();
        assert_eq!(loaded.items.len(), 1);
        assert_eq!(loaded.items[0].inbox_item_id, first);
        assert_eq!(loaded.quarantined.len(), 1);
        assert!(!poison_path.exists());
    }

    #[test]
    fn payload_and_per_session_count_caps_reject_enqueue() {
        let bounds = InboxBounds {
            payload_byte_cap: 8,
            per_session_count_cap: 1,
            per_session_byte_cap: 1_024,
        };
        let (_temp, store) = store(bounds);
        let payload_error = store
            .enqueue(draft("session-a", json!("too-large"), 10, 1_000), 10)
            .unwrap_err();
        assert!(payload_error.to_string().contains("payload byte cap 8"));

        store
            .enqueue(draft("session-a", json!("ok"), 11, 1_000), 11)
            .unwrap();
        let count_error = store
            .enqueue(draft("session-a", json!("ok"), 12, 1_000), 12)
            .unwrap_err();
        assert!(count_error.to_string().contains("per-session count cap 1"));
    }

    #[test]
    fn debug_output_contains_id_and_redacts_payload_body() {
        let (_temp, store) = store(InboxBounds::default());
        let secret = "payload-body-must-not-leak";
        let id = store
            .enqueue(draft("session-a", json!({"body": secret}), 10, 1_000), 10)
            .unwrap();
        let output = serde_json::to_string(&store.debug_snapshot(None).unwrap()).unwrap();
        assert!(output.contains(id.as_str()));
        assert!(output.contains("[REDACTED]"));
        assert!(!output.contains(secret));
    }

    #[test]
    fn retention_sweep_removes_only_old_terminal_items() {
        let (_temp, store) = store(InboxBounds::default());
        let retry = RetryPolicy::default();
        let id = store
            .enqueue(draft("session-a", json!("done"), 10, 1_000), 10)
            .unwrap();
        store
            .transition("session-a", &id, InboxTransition::Cancel, 20, retry)
            .unwrap();
        let sweep = store
            .sweep_retention(
                30,
                RetentionPolicy {
                    acked_ms: 100,
                    cancelled_ms: 10,
                    expired_ms: 100,
                    undeliverable_ms: 100,
                },
            )
            .unwrap();
        assert_eq!(sweep.removed, 1);
        assert!(store.load("session-a", &id).unwrap().is_none());
    }
}
