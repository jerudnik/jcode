//! Durable inbox delivery state machine.
//!
//! The deployment has one daemon owning a durable inbox root. This module does
//! not implement leases or multi-origin arbitration. A future multi-daemon
//! topology must add ownership fencing before sharing an inbox root.
//!
//! Every attempt is durably recorded before [`DeliveryAdapter::inject`] is
//! called. An injector must call [`DeliveryEngine::record_turn_start`] with the
//! carried `inbox_item_id` immediately before it starts the model turn. That
//! identity-bearing record, rather than an unrelated later turn, is the replay
//! deduplication key.

use super::store::{InboxStore, InboxTransition, RetryPolicy};
use super::{InboxClass, InboxEnvelope, InboxItemId, InboxState};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TargetState {
    Live,
    StoppedValid,
    ReclaimedTerminal,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct EvidenceCursor {
    pub control_log_offset: u64,
    pub transcript_len: u64,
}

impl EvidenceCursor {
    fn advanced_from(self, baseline: Self) -> bool {
        self.control_log_offset > baseline.control_log_offset
            || self.transcript_len > baseline.transcript_len
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DeliveryBatch {
    pub target_session_id: String,
    pub items: Vec<InboxEnvelope>,
}

pub trait DeliveryAdapter {
    fn target_state(&mut self, item: &InboxEnvelope) -> Result<TargetState>;
    /// Cursor over approved evidence only: ArtifactFiled/TaskHeartbeat events
    /// and transcript growth. Status strings are not evidence.
    fn evidence_cursor(&mut self, item: &InboxEnvelope) -> Result<EvidenceCursor>;
    /// Inject the materialized payload. Before beginning a model turn, the
    /// adapter must call `record_turn_start` with every carried item ID.
    fn inject(&mut self, batch: DeliveryBatch) -> Result<()>;
    fn delivered(&mut self, item: &InboxEnvelope) -> Result<()>;
    fn terminal(&mut self, item: &InboxEnvelope) -> Result<()>;
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DeliveryTick {
    pub injected_batches: usize,
    pub injected_items: usize,
    pub acked: usize,
    pub terminal: usize,
    pub pending_stopped: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct AttemptRecord {
    item_id: InboxItemId,
    session_id: String,
    attempt: u32,
    started_at: u64,
    baseline: EvidenceCursor,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct TurnStartRecord {
    item_id: InboxItemId,
    session_id: String,
    started_at: u64,
}

pub struct DeliveryEngine {
    store: InboxStore,
    retry: RetryPolicy,
}

impl DeliveryEngine {
    pub fn new(store: InboxStore, retry: RetryPolicy) -> Self {
        Self { store, retry }
    }

    pub fn store(&self) -> &InboxStore {
        &self.store
    }

    /// Re-anchor all unfinished attempts to current evidence. This deliberately
    /// mirrors await-state's `scan_offset == 0 -> current tail` rule: history
    /// that predates daemon replay is never treated as fresh evidence.
    pub fn reanchor_replay<A: DeliveryAdapter>(&self, adapter: &mut A, now_ms: u64) -> Result<()> {
        for item in self.store.load_all()?.items {
            if item.state.is_terminal() {
                continue;
            }
            if item.state == InboxState::Attempting && self.has_turn_start(&item)? {
                continue;
            }
            if matches!(
                item.state,
                InboxState::Attempting | InboxState::DeliveredUnacked
            ) {
                let baseline = adapter.evidence_cursor(&item)?;
                self.write_attempt(&item, now_ms, baseline)?;
            }
        }
        Ok(())
    }

    /// Identity-bearing turn-start hook. Returns false for a duplicate start,
    /// allowing the caller to suppress the second model turn.
    pub fn record_turn_start(
        &self,
        session_id: &str,
        item_id: &InboxItemId,
        now_ms: u64,
    ) -> Result<bool> {
        let item = self
            .store
            .load(session_id, item_id)?
            .with_context(|| format!("inbox item {item_id} does not exist"))?;
        if item.state.is_terminal() {
            return Ok(false);
        }
        let path = self.turn_start_path(&item)?;
        let record = TurnStartRecord {
            item_id: item_id.clone(),
            session_id: session_id.to_string(),
            started_at: now_ms,
        };
        match persist_new(&path, &serde_json::to_vec(&record)?) {
            Ok(()) => {
                if item.state == InboxState::Attempting {
                    self.store.transition(
                        session_id,
                        item_id,
                        InboxTransition::DeliveryAccepted,
                        now_ms,
                        self.retry,
                    )?;
                }
                Ok(true)
            }
            Err(_error) if path.exists() => Ok(false),
            Err(error) => Err(error),
        }
    }

    pub fn ack(&self, session_id: &str, item_id: &InboxItemId, now_ms: u64) -> Result<bool> {
        let Some(item) = self.store.load(session_id, item_id)? else {
            return Ok(false);
        };
        if item.state.is_terminal() {
            return Ok(false);
        }
        if !self.has_turn_start(&item)? {
            bail!("cannot ack inbox item {item_id} without correlated turn start");
        }
        if item.state == InboxState::Attempting {
            self.store.transition(
                session_id,
                item_id,
                InboxTransition::DeliveryAccepted,
                now_ms,
                self.retry,
            )?;
        }
        self.store.transition(
            session_id,
            item_id,
            InboxTransition::Ack,
            now_ms,
            self.retry,
        )?;
        Ok(true)
    }

    pub fn tick<A: DeliveryAdapter>(&self, adapter: &mut A, now_ms: u64) -> Result<DeliveryTick> {
        let mut result = DeliveryTick::default();
        self.coalesce(now_ms, adapter, &mut result)?;
        let items = self.store.load_all()?.items;
        let mut batches: BTreeMap<(String, u8), Vec<InboxEnvelope>> = BTreeMap::new();

        for mut item in items {
            if item.state.is_terminal() {
                continue;
            }
            if now_ms >= item.expires_at {
                item = self.store.transition(
                    &item.target_session_id,
                    &item.inbox_item_id,
                    InboxTransition::Expire,
                    now_ms,
                    self.retry,
                )?;
                adapter.terminal(&item)?;
                result.terminal += 1;
                continue;
            }
            if self.has_turn_start(&item)? {
                if self.ack(&item.target_session_id, &item.inbox_item_id, now_ms)? {
                    let acked = self
                        .store
                        .load(&item.target_session_id, &item.inbox_item_id)?
                        .context("acked item disappeared")?;
                    adapter.delivered(&acked)?;
                    adapter.terminal(&acked)?;
                    result.acked += 1;
                    result.terminal += 1;
                }
                continue;
            }
            match adapter.target_state(&item)? {
                TargetState::ReclaimedTerminal => {
                    let terminal = self.store.transition(
                        &item.target_session_id,
                        &item.inbox_item_id,
                        InboxTransition::MarkUndeliverable,
                        now_ms,
                        self.retry,
                    )?;
                    adapter.terminal(&terminal)?;
                    result.terminal += 1;
                    continue;
                }
                TargetState::StoppedValid => {
                    result.pending_stopped += 1;
                    continue;
                }
                TargetState::Live => {}
            }
            if item.state == InboxState::Attempting {
                let attempt = self.read_attempt(&item)?;
                let evidence = adapter.evidence_cursor(&item)?;
                if item.attempts >= self.retry.max_attempts
                    && !evidence.advanced_from(attempt.baseline)
                {
                    let terminal = self.store.transition(
                        &item.target_session_id,
                        &item.inbox_item_id,
                        InboxTransition::MarkUndeliverable,
                        now_ms,
                        self.retry,
                    )?;
                    adapter.terminal(&terminal)?;
                    result.terminal += 1;
                    continue;
                }
                item = self.store.transition(
                    &item.target_session_id,
                    &item.inbox_item_id,
                    InboxTransition::AttemptFailed,
                    now_ms,
                    self.retry,
                )?;
            }
            if item.state == InboxState::Pending && now_ms >= item.due_at {
                item = self.store.transition(
                    &item.target_session_id,
                    &item.inbox_item_id,
                    InboxTransition::MarkDue,
                    now_ms,
                    self.retry,
                )?;
            }
            if item.state != InboxState::Due {
                continue;
            }
            item = self.store.transition(
                &item.target_session_id,
                &item.inbox_item_id,
                InboxTransition::BeginAttempt,
                now_ms,
                self.retry,
            )?;
            if item.state == InboxState::Undeliverable {
                adapter.terminal(&item)?;
                result.terminal += 1;
                continue;
            }
            let baseline = adapter.evidence_cursor(&item)?;
            self.write_attempt(&item, now_ms, baseline)?;
            let batch_class = if item.class == InboxClass::Dm {
                class_order(item.class)
            } else {
                class_order(item.class).saturating_add((item.attempts % 2) as u8 * 16)
            };
            batches
                .entry((item.target_session_id.clone(), batch_class))
                .or_default()
                .push(item);
        }

        for ((_session, _class), mut items) in batches {
            items.sort_by(|a, b| (a.due_at, &a.inbox_item_id).cmp(&(b.due_at, &b.inbox_item_id)));
            if items
                .first()
                .is_some_and(|item| item.class != InboxClass::Dm)
            {
                for item in items {
                    adapter.inject(DeliveryBatch {
                        target_session_id: item.target_session_id.clone(),
                        items: vec![item],
                    })?;
                    result.injected_batches += 1;
                    result.injected_items += 1;
                }
            } else {
                result.injected_items += items.len();
                result.injected_batches += 1;
                adapter.inject(DeliveryBatch {
                    target_session_id: items[0].target_session_id.clone(),
                    items,
                })?;
            }
        }
        Ok(result)
    }

    fn coalesce<A: DeliveryAdapter>(
        &self,
        now_ms: u64,
        adapter: &mut A,
        result: &mut DeliveryTick,
    ) -> Result<()> {
        let mut newest: HashMap<(String, InboxClass, String), InboxItemId> = HashMap::new();
        let items = self.store.load_all()?.items;
        for item in &items {
            if item.state.is_terminal()
                || !matches!(
                    item.class,
                    InboxClass::AwaitResult | InboxClass::BackgroundCompletion
                )
            {
                continue;
            }
            let Some(key) = coalesce_key(item) else {
                continue;
            };
            newest.insert(
                (item.target_session_id.clone(), item.class, key),
                item.inbox_item_id.clone(),
            );
        }
        let keep: HashSet<InboxItemId> = newest.into_values().collect();
        for item in items {
            if item.state.is_terminal()
                || !matches!(
                    item.class,
                    InboxClass::AwaitResult | InboxClass::BackgroundCompletion
                )
                || keep.contains(&item.inbox_item_id)
            {
                continue;
            }
            let cancelled = self.store.transition(
                &item.target_session_id,
                &item.inbox_item_id,
                InboxTransition::Cancel,
                now_ms,
                self.retry,
            )?;
            adapter.terminal(&cancelled)?;
            result.terminal += 1;
        }
        Ok(())
    }

    fn write_attempt(
        &self,
        item: &InboxEnvelope,
        now_ms: u64,
        baseline: EvidenceCursor,
    ) -> Result<()> {
        let record = AttemptRecord {
            item_id: item.inbox_item_id.clone(),
            session_id: item.target_session_id.clone(),
            attempt: item.attempts,
            started_at: now_ms,
            baseline,
        };
        write_atomic(&self.attempt_path(item)?, &serde_json::to_vec(&record)?)
    }

    fn read_attempt(&self, item: &InboxEnvelope) -> Result<AttemptRecord> {
        Ok(serde_json::from_slice(&fs::read(
            self.attempt_path(item)?,
        )?)?)
    }

    fn has_turn_start(&self, item: &InboxEnvelope) -> Result<bool> {
        Ok(self.turn_start_path(item)?.exists())
    }

    fn attempt_path(&self, item: &InboxEnvelope) -> Result<PathBuf> {
        Ok(self
            .store
            .root()
            .join("attempts")
            .join(&item.target_session_id)
            .join(format!("{}.json", item.inbox_item_id)))
    }

    fn turn_start_path(&self, item: &InboxEnvelope) -> Result<PathBuf> {
        Ok(self
            .store
            .root()
            .join("turn-starts")
            .join(&item.target_session_id)
            .join(format!("{}.json", item.inbox_item_id)))
    }
}

fn class_order(class: InboxClass) -> u8 {
    match class {
        InboxClass::ScheduledWake => 0,
        InboxClass::AwaitResult => 1,
        InboxClass::BackgroundCompletion => 2,
        InboxClass::Dm => 3,
        InboxClass::LegacyInterrupt => 4,
    }
}

fn coalesce_key(item: &InboxEnvelope) -> Option<String> {
    let field = match item.class {
        InboxClass::AwaitResult => "await_key",
        InboxClass::BackgroundCompletion => "task_id",
        _ => return None,
    };
    item.payload.get(field)?.as_str().map(str::to_string)
}

fn persist_new(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    sync_parent(path);
    Ok(())
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temp = path.with_extension(format!("tmp.{}", uuid::Uuid::new_v4().simple()));
    let mut file = File::create(&temp)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    fs::rename(&temp, path)?;
    sync_parent(path);
    Ok(())
}

fn sync_parent(path: &Path) {
    #[cfg(unix)]
    if let Some(parent) = path.parent()
        && let Ok(directory) = File::open(parent)
    {
        let _ = directory.sync_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inbox::store::InboxBounds;
    use crate::inbox::{DeliveryPolicy, InboxItemDraft};
    use serde_json::json;

    struct FakeAdapter {
        state: TargetState,
        evidence: EvidenceCursor,
        injected: Vec<DeliveryBatch>,
        delivered: Vec<String>,
        terminal: Vec<String>,
    }

    impl Default for FakeAdapter {
        fn default() -> Self {
            Self {
                state: TargetState::Live,
                evidence: EvidenceCursor::default(),
                injected: Vec::new(),
                delivered: Vec::new(),
                terminal: Vec::new(),
            }
        }
    }

    impl DeliveryAdapter for FakeAdapter {
        fn target_state(&mut self, _: &InboxEnvelope) -> Result<TargetState> {
            Ok(self.state)
        }
        fn evidence_cursor(&mut self, _: &InboxEnvelope) -> Result<EvidenceCursor> {
            Ok(self.evidence)
        }
        fn inject(&mut self, batch: DeliveryBatch) -> Result<()> {
            self.injected.push(batch);
            Ok(())
        }
        fn delivered(&mut self, item: &InboxEnvelope) -> Result<()> {
            self.delivered.push(item.inbox_item_id.to_string());
            Ok(())
        }
        fn terminal(&mut self, item: &InboxEnvelope) -> Result<()> {
            self.terminal.push(item.inbox_item_id.to_string());
            Ok(())
        }
    }

    fn setup() -> (tempfile::TempDir, DeliveryEngine) {
        let temp = tempfile::tempdir().unwrap();
        let store = InboxStore::with_root(temp.path().join("inbox"), InboxBounds::default());
        (
            temp,
            DeliveryEngine::new(
                store,
                RetryPolicy {
                    max_attempts: 2,
                    initial_backoff_ms: 10,
                    max_backoff_ms: 10,
                },
            ),
        )
    }

    fn enqueue(
        engine: &DeliveryEngine,
        class: InboxClass,
        payload: serde_json::Value,
        now: u64,
    ) -> InboxItemId {
        engine
            .store
            .enqueue(
                InboxItemDraft {
                    class,
                    target_session_id: "session-a".into(),
                    swarm_id: Some("swarm-a".into()),
                    payload,
                    delivery_policy: DeliveryPolicy::default(),
                    due_at: now,
                    ttl_ms: 1_000,
                },
                now,
            )
            .unwrap()
    }

    #[test]
    fn crash_after_enqueue_replay_delivers_once() {
        let (_temp, engine) = setup();
        let id = enqueue(
            &engine,
            InboxClass::ScheduledWake,
            json!({"body":"wake"}),
            10,
        );
        let mut adapter = FakeAdapter::default();
        engine.reanchor_replay(&mut adapter, 20).unwrap();
        assert_eq!(engine.tick(&mut adapter, 20).unwrap().injected_items, 1);
        assert_eq!(
            engine.record_turn_start("session-a", &id, 21).unwrap(),
            true
        );
        assert_eq!(engine.tick(&mut adapter, 22).unwrap().acked, 1);
        assert_eq!(engine.tick(&mut adapter, 23).unwrap().injected_items, 0);
    }

    #[test]
    fn turn_start_id_dedup_prevents_second_model_turn() {
        let (_temp, engine) = setup();
        let id = enqueue(&engine, InboxClass::ScheduledWake, json!({}), 10);
        let mut adapter = FakeAdapter::default();
        engine.tick(&mut adapter, 10).unwrap();
        assert!(engine.record_turn_start("session-a", &id, 11).unwrap());
        assert!(!engine.record_turn_start("session-a", &id, 12).unwrap());
        engine.tick(&mut adapter, 13).unwrap();
        assert_eq!(adapter.injected.len(), 1);
    }

    #[test]
    fn crash_mid_attempt_replays_at_most_one_turn() {
        let (_temp, engine) = setup();
        let id = enqueue(&engine, InboxClass::ScheduledWake, json!({}), 10);
        let mut adapter = FakeAdapter::default();
        engine.tick(&mut adapter, 10).unwrap();
        assert_eq!(
            engine.store.load("session-a", &id).unwrap().unwrap().state,
            InboxState::Attempting
        );
        engine.reanchor_replay(&mut adapter, 11).unwrap();
        engine.tick(&mut adapter, 20).unwrap();
        engine.tick(&mut adapter, 30).unwrap();
        assert_eq!(adapter.injected.len(), 2);
        assert!(engine.record_turn_start("session-a", &id, 31).unwrap());
        engine.tick(&mut adapter, 32).unwrap();
        assert_eq!(
            engine.store.load("session-a", &id).unwrap().unwrap().state,
            InboxState::Acked
        );
    }

    #[test]
    fn no_life_reaches_undeliverable_and_stops() {
        let (_temp, engine) = setup();
        let id = enqueue(&engine, InboxClass::ScheduledWake, json!({}), 10);
        let mut adapter = FakeAdapter::default();
        engine.tick(&mut adapter, 10).unwrap();
        engine.tick(&mut adapter, 20).unwrap();
        engine.tick(&mut adapter, 30).unwrap();
        engine.tick(&mut adapter, 40).unwrap();
        assert_eq!(
            engine.store.load("session-a", &id).unwrap().unwrap().state,
            InboxState::Undeliverable
        );
        assert_eq!(engine.tick(&mut adapter, 40).unwrap().injected_items, 0);
    }

    #[test]
    fn cancelled_item_wins_race_with_due_delivery() {
        let (_temp, engine) = setup();
        let id = enqueue(&engine, InboxClass::ScheduledWake, json!({}), 10);
        engine
            .store
            .transition("session-a", &id, InboxTransition::Cancel, 10, engine.retry)
            .unwrap();
        assert_eq!(
            engine
                .tick(&mut FakeAdapter::default(), 10)
                .unwrap()
                .injected_items,
            0
        );
    }

    #[test]
    fn expiry_wins_race_with_late_ack() {
        let (_temp, engine) = setup();
        let id = enqueue(&engine, InboxClass::ScheduledWake, json!({}), 10);
        let mut adapter = FakeAdapter::default();
        engine.tick(&mut adapter, 10).unwrap();
        engine.record_turn_start("session-a", &id, 11).unwrap();
        engine.tick(&mut adapter, 1_010).unwrap();
        assert!(!engine.ack("session-a", &id, 1_011).unwrap());
    }

    #[test]
    fn manual_turn_does_not_ack_queued_item() {
        let (_temp, engine) = setup();
        let id = enqueue(&engine, InboxClass::ScheduledWake, json!({}), 100);
        let mut adapter = FakeAdapter::default();
        assert_eq!(engine.tick(&mut adapter, 50).unwrap().acked, 0);
        assert_eq!(
            engine.store.load("session-a", &id).unwrap().unwrap().state,
            InboxState::Pending
        );
    }

    #[test]
    fn reload_mid_attempt_reanchors_old_evidence() {
        let (_temp, engine) = setup();
        enqueue(&engine, InboxClass::ScheduledWake, json!({}), 10);
        let mut adapter = FakeAdapter::default();
        engine.tick(&mut adapter, 10).unwrap();
        adapter.evidence = EvidenceCursor {
            control_log_offset: 50,
            transcript_len: 8,
        };
        engine.reanchor_replay(&mut adapter, 11).unwrap();
        engine.tick(&mut adapter, 20).unwrap();
        engine.tick(&mut adapter, 30).unwrap();
        assert_eq!(adapter.injected.len(), 2);
    }

    #[test]
    fn stopped_valid_stays_pending_and_reclaimed_is_terminal() {
        let (_temp, engine) = setup();
        let id = enqueue(&engine, InboxClass::ScheduledWake, json!({}), 10);
        let mut adapter = FakeAdapter {
            state: TargetState::StoppedValid,
            ..FakeAdapter::default()
        };
        assert_eq!(engine.tick(&mut adapter, 10).unwrap().pending_stopped, 1);
        adapter.state = TargetState::ReclaimedTerminal;
        engine.tick(&mut adapter, 11).unwrap();
        assert_eq!(
            engine.store.load("session-a", &id).unwrap().unwrap().state,
            InboxState::Undeliverable
        );
    }

    #[test]
    fn coalesces_snapshots_but_batches_all_dms_in_order() {
        let (_temp, engine) = setup();
        enqueue(
            &engine,
            InboxClass::AwaitResult,
            json!({"await_key":"a","v":1}),
            10,
        );
        enqueue(
            &engine,
            InboxClass::AwaitResult,
            json!({"await_key":"a","v":2}),
            11,
        );
        enqueue(&engine, InboxClass::Dm, json!({"body":"one"}), 12);
        enqueue(&engine, InboxClass::Dm, json!({"body":"two"}), 13);
        let mut adapter = FakeAdapter::default();
        engine.tick(&mut adapter, 20).unwrap();
        let dm = adapter
            .injected
            .iter()
            .find(|batch| batch.items[0].class == InboxClass::Dm)
            .unwrap();
        assert_eq!(dm.items.len(), 2);
        assert_eq!(dm.items[0].payload["body"], "one");
        assert_eq!(
            adapter
                .injected
                .iter()
                .filter(|batch| batch.items[0].class == InboxClass::AwaitResult)
                .count(),
            1
        );
    }
}
