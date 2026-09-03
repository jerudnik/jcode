//! Durable, identity-bearing messages waiting to be delivered to sessions.
//!
//! Inbox item identity is minted before the mutable delivery state is built.
//! Routing fields and the fully materialized payload are frozen at enqueue;
//! notify/wake/background flags are delivery policy and do not participate in
//! identity. The delivery engine is intentionally outside this module.

pub mod delivery;
pub mod store;

use serde::{Deserialize, Serialize};
use std::fmt;

pub const CURRENT_SCHEMA_VERSION: u32 = 2;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct InboxItemId(String);

impl InboxItemId {
    /// Mint a time-sortable, ULID-style 128-bit identifier.
    pub fn mint(now_ms: u64) -> Self {
        const CROCKFORD: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

        let mut bytes = [0_u8; 16];
        let timestamp = now_ms.to_be_bytes();
        bytes[..6].copy_from_slice(&timestamp[2..]);
        bytes[6..].copy_from_slice(&rand::random::<[u8; 10]>());

        let mut value = u128::from_be_bytes(bytes);
        let mut encoded = [b'0'; 26];
        for byte in encoded.iter_mut().rev() {
            *byte = CROCKFORD[(value & 0x1f) as usize];
            value >>= 5;
        }
        Self(String::from_utf8(encoded.to_vec()).expect("ULID alphabet is valid UTF-8"))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for InboxItemId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InboxClass {
    ScheduledWake,
    AwaitResult,
    BackgroundCompletion,
    Dm,
    LegacyInterrupt,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DeliveryPolicy {
    pub notify: bool,
    pub wake: bool,
    pub background: bool,
}

impl Default for DeliveryPolicy {
    fn default() -> Self {
        Self {
            notify: true,
            wake: true,
            background: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InboxState {
    Pending,
    Due,
    Attempting,
    DeliveredUnacked,
    Acked,
    Cancelled,
    Expired,
    Undeliverable,
}

impl InboxState {
    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Acked | Self::Cancelled | Self::Expired | Self::Undeliverable
        )
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InboxEnvelope {
    pub schema_version: u32,
    pub inbox_item_id: InboxItemId,
    pub class: InboxClass,
    pub target_session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub swarm_id: Option<String>,
    pub payload: serde_json::Value,
    pub delivery_policy: DeliveryPolicy,
    pub created_at: u64,
    pub due_at: u64,
    pub expires_at: u64,
    pub attempts: u32,
    pub state: InboxState,
    pub state_changed_at: u64,
}

#[derive(Clone, Debug)]
pub struct InboxItemDraft {
    pub class: InboxClass,
    pub target_session_id: String,
    pub swarm_id: Option<String>,
    pub payload: serde_json::Value,
    pub delivery_policy: DeliveryPolicy,
    pub due_at: u64,
    pub ttl_ms: u64,
}

impl InboxItemDraft {
    pub(super) fn into_envelope(self, inbox_item_id: InboxItemId, now_ms: u64) -> InboxEnvelope {
        InboxEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            inbox_item_id,
            class: self.class,
            target_session_id: self.target_session_id,
            swarm_id: self.swarm_id,
            payload: self.payload,
            delivery_policy: self.delivery_policy,
            created_at: now_ms,
            due_at: self.due_at,
            expires_at: now_ms.saturating_add(self.ttl_ms),
            attempts: 0,
            state: InboxState::Pending,
            state_changed_at: now_ms,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minted_ids_are_ulid_style_and_time_sortable() {
        let early = InboxItemId::mint(1_000);
        let late = InboxItemId::mint(2_000);
        assert_eq!(early.as_str().len(), 26);
        assert!(early < late);
    }

    #[test]
    fn terminal_states_are_explicit() {
        assert!(!InboxState::DeliveredUnacked.is_terminal());
        assert!(InboxState::Acked.is_terminal());
        assert!(InboxState::Cancelled.is_terminal());
        assert!(InboxState::Expired.is_terminal());
        assert!(InboxState::Undeliverable.is_terminal());
    }
}
