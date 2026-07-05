//! W1 command-as-state: the swarm control-plane event log.
//!
//! Decision record: `~/notes/projects/jcode/proposals/orchestration-hardening/
//! w1-store-schema-comparison.md` (operator sign-off 2026-07-05, path B).
//! The log is the ONLY writable surface for swarm control state; current
//! state is a fold over events. The audited failures (F1-F5) were all
//! stale-snapshot bugs: decisions made against partial or outdated views.
//! A log makes staleness detectable (consumers carry offsets) and doubles
//! as the W2 resume stream, the wedge-recovery replay, and the future
//! cross-host sync feed.
//!
//! Multi-host trajectory (deliberate, per the sign-off): the ENVELOPE is
//! host-agnostic from day one. Every event carries an `origin` (host/daemon
//! identity, a single-host constant today) and a per-origin monotonic
//! sequence, so per-swarm logs can later be merged or shipped across hosts
//! without rewriting history. The fold is a pure function with no server
//! coupling, so a daemon on another machine reuses it unchanged.
//!
//! Storage is JSONL: one envelope per line. Offsets are byte offsets into
//! the file, which survive process restarts and partial reads (a torn final
//! line is ignored until completed). Do not hand-edit log files.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufRead, Seek, Write};
use std::path::{Path, PathBuf};

/// Identity of the process/host that appended an event. Single-host installs
/// use [`LOCAL_ORIGIN`]; a future daemon hierarchy assigns real identities.
pub const LOCAL_ORIGIN: &str = "local";

/// A control-plane state transition. These are the ONLY ways swarm control
/// state changes; anything not expressible here is not control state.
///
/// Payload discipline: events carry the transition, not the world. The fold
/// derives the world. Keep variants small and append-only (add variants
/// freely; never repurpose or remove one that shipped - old logs must
/// replay forever).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SwarmControlEvent {
    /// A session joined the swarm.
    MemberJoined {
        session_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        friendly_name: Option<String>,
        role: String,
    },
    /// A session left (or was evicted from) the swarm.
    MemberLeft { session_id: String },
    /// A member's role changed (coordinator handoff, self-promotion).
    RoleChanged { session_id: String, role: String },
    /// A member's lifecycle status changed (ready/running/failed/...).
    MemberStatusChanged { session_id: String, status: String },
    /// A plan task was assigned to a session (or unassigned with `None`).
    TaskAssigned {
        task_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        assigned_to: Option<String>,
    },
    /// A plan task's status changed (queued/running/completed/failed/...).
    TaskStatusChanged { task_id: String, status: String },
    /// Liveness signal for an in-flight task.
    TaskHeartbeat { task_id: String, wall_ms: u64 },
}

/// The host-agnostic envelope every event ships in.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SwarmControlEnvelope {
    /// Which host/daemon appended this event. [`LOCAL_ORIGIN`] today.
    pub origin: String,
    /// Per-origin monotonic sequence number. Merging logs from multiple
    /// origins preserves per-origin order; cross-origin order is by
    /// (wall_ms, origin) and is advisory only.
    pub seq: u64,
    /// Wall-clock milliseconds since the unix epoch when appended.
    pub wall_ms: u64,
    pub swarm_id: String,
    pub event: SwarmControlEvent,
}

/// Fold output: the current control-plane state of one swarm. This is the
/// same information the server's in-memory maps hold, derived instead of
/// mutated. Anything readable here MUST be derivable from the log alone.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SwarmControlState {
    /// session_id -> (role, status, friendly_name)
    pub members: HashMap<String, MemberControlState>,
    /// task_id -> (assigned_to, status)
    pub tasks: HashMap<String, TaskControlState>,
    /// Events folded so far (diagnostic; lets callers assert progress).
    pub events_applied: u64,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MemberControlState {
    pub role: String,
    pub status: String,
    pub friendly_name: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TaskControlState {
    pub assigned_to: Option<String>,
    pub status: String,
    pub last_heartbeat_ms: Option<u64>,
}

impl SwarmControlState {
    /// The current coordinator: derived, not stored. One home for the truth
    /// (F4 was two stores disagreeing about exactly this). Deterministic
    /// under multiple coordinators (a transient hand-off state): lowest
    /// session id wins until the fold sees the demotion event.
    pub fn coordinator(&self) -> Option<&str> {
        self.members
            .iter()
            .filter(|(_, member)| member.role == "coordinator")
            .map(|(session_id, _)| session_id.as_str())
            .min()
    }

    /// Apply one event. Total: unknown references are tolerated (a
    /// `TaskAssigned` for a task the fold has not seen creates it), so a
    /// compacted log prefix or merged partial log still folds.
    pub fn apply(&mut self, event: &SwarmControlEvent) {
        match event {
            SwarmControlEvent::MemberJoined {
                session_id,
                friendly_name,
                role,
            } => {
                self.members.insert(
                    session_id.clone(),
                    MemberControlState {
                        role: role.clone(),
                        status: "ready".to_string(),
                        friendly_name: friendly_name.clone(),
                    },
                );
            }
            SwarmControlEvent::MemberLeft { session_id } => {
                self.members.remove(session_id);
            }
            SwarmControlEvent::RoleChanged { session_id, role } => {
                self.members.entry(session_id.clone()).or_default().role = role.clone();
            }
            SwarmControlEvent::MemberStatusChanged { session_id, status } => {
                self.members.entry(session_id.clone()).or_default().status = status.clone();
            }
            SwarmControlEvent::TaskAssigned {
                task_id,
                assigned_to,
            } => {
                self.tasks.entry(task_id.clone()).or_default().assigned_to =
                    assigned_to.clone();
            }
            SwarmControlEvent::TaskStatusChanged { task_id, status } => {
                self.tasks.entry(task_id.clone()).or_default().status = status.clone();
            }
            SwarmControlEvent::TaskHeartbeat { task_id, wall_ms } => {
                self.tasks
                    .entry(task_id.clone())
                    .or_default()
                    .last_heartbeat_ms = Some(*wall_ms);
            }
        }
        self.events_applied += 1;
    }
}

/// Fold a sequence of envelopes into current state. Pure; the whole point.
pub fn fold<'a>(events: impl IntoIterator<Item = &'a SwarmControlEnvelope>) -> SwarmControlState {
    let mut state = SwarmControlState::default();
    for envelope in events {
        state.apply(&envelope.event);
    }
    state
}

/// Append-only JSONL writer for one swarm's control log. Tracks the
/// per-origin sequence; callers hold one writer per (swarm, origin).
pub struct ControlLogWriter {
    path: PathBuf,
    origin: String,
    swarm_id: String,
    next_seq: u64,
}

impl ControlLogWriter {
    /// Open (creating if needed) the log at `path`. The next sequence number
    /// continues from the highest existing seq for this origin, so reopening
    /// after a restart never reuses a sequence number.
    pub fn open(path: &Path, swarm_id: &str, origin: &str) -> std::io::Result<Self> {
        let next_seq = match read_from(path, 0) {
            Ok(read) => read
                .envelopes
                .iter()
                .filter(|(_, envelope)| envelope.origin == origin)
                .map(|(_, envelope)| envelope.seq + 1)
                .max()
                .unwrap_or(0),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => 0,
            Err(error) => return Err(error),
        };
        Ok(Self {
            path: path.to_path_buf(),
            origin: origin.to_string(),
            swarm_id: swarm_id.to_string(),
            next_seq,
        })
    }

    /// Append one event; returns the envelope as written. The write is a
    /// single `write_all` of one line + flush, so concurrent readers never
    /// observe a torn line as valid JSON (they skip trailing partial lines).
    pub fn append(&mut self, event: SwarmControlEvent) -> std::io::Result<SwarmControlEnvelope> {
        let envelope = SwarmControlEnvelope {
            origin: self.origin.clone(),
            seq: self.next_seq,
            wall_ms: now_wall_ms(),
            swarm_id: self.swarm_id.clone(),
            event,
        };
        let mut line = serde_json::to_string(&envelope)
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
        line.push('\n');
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        file.write_all(line.as_bytes())?;
        file.flush()?;
        self.next_seq += 1;
        Ok(envelope)
    }
}

/// Result of reading a log from an offset.
pub struct ControlLogRead {
    /// `(next_offset, envelope)` pairs: `next_offset` is the byte offset to
    /// resume from AFTER consuming that envelope.
    pub envelopes: Vec<(u64, SwarmControlEnvelope)>,
    /// Offset to resume from to see only events appended after this read.
    pub next_offset: u64,
}

/// Read all complete events at/after byte `offset`. A trailing partial line
/// (torn concurrent write) is not consumed: `next_offset` stops before it,
/// so the next read picks it up once completed. Corrupt COMPLETE lines are
/// skipped with their bytes consumed (logged upstream by callers if they
/// care); one bad line must not wedge every future replay.
pub fn read_from(path: &Path, offset: u64) -> std::io::Result<ControlLogRead> {
    let mut file = std::fs::File::open(path)?;
    file.seek(std::io::SeekFrom::Start(offset))?;
    let mut reader = std::io::BufReader::new(file);
    let mut envelopes = Vec::new();
    let mut consumed = offset;
    let mut buffer = String::new();
    loop {
        buffer.clear();
        let bytes = reader.read_line(&mut buffer)?;
        if bytes == 0 {
            break;
        }
        if !buffer.ends_with('\n') {
            // Torn final line: leave it for the next read.
            break;
        }
        consumed += bytes as u64;
        match serde_json::from_str::<SwarmControlEnvelope>(buffer.trim_end()) {
            Ok(envelope) => envelopes.push((consumed, envelope)),
            Err(_) => continue, // skip corrupt complete line, bytes consumed
        }
    }
    Ok(ControlLogRead {
        envelopes,
        next_offset: consumed,
    })
}

/// Replay an entire log into a folded state plus the resume offset.
pub fn replay(path: &Path) -> std::io::Result<(SwarmControlState, u64)> {
    let read = read_from(path, 0)?;
    let state = fold(read.envelopes.iter().map(|(_, envelope)| envelope));
    Ok((state, read.next_offset))
}

fn now_wall_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_log() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let path = dir.path().join("swarm-test.control.jsonl");
        (dir, path)
    }

    #[test]
    fn append_read_fold_roundtrip() {
        let (_dir, path) = temp_log();
        let mut writer = ControlLogWriter::open(&path, "swarm-1", LOCAL_ORIGIN).expect("open");
        writer
            .append(SwarmControlEvent::MemberJoined {
                session_id: "coord".into(),
                friendly_name: Some("falcon".into()),
                role: "coordinator".into(),
            })
            .expect("append");
        writer
            .append(SwarmControlEvent::MemberJoined {
                session_id: "w1".into(),
                friendly_name: None,
                role: "agent".into(),
            })
            .expect("append");
        writer
            .append(SwarmControlEvent::TaskAssigned {
                task_id: "t1".into(),
                assigned_to: Some("w1".into()),
            })
            .expect("append");
        writer
            .append(SwarmControlEvent::TaskStatusChanged {
                task_id: "t1".into(),
                status: "running".into(),
            })
            .expect("append");

        let (state, offset) = replay(&path).expect("replay");
        assert_eq!(state.events_applied, 4);
        assert_eq!(state.coordinator(), Some("coord"));
        assert_eq!(state.members["w1"].role, "agent");
        assert_eq!(state.tasks["t1"].assigned_to.as_deref(), Some("w1"));
        assert_eq!(state.tasks["t1"].status, "running");
        assert!(offset > 0);

        // Offset resume: appending after a read is visible from next_offset
        // and ONLY the new event is returned.
        writer
            .append(SwarmControlEvent::TaskStatusChanged {
                task_id: "t1".into(),
                status: "completed".into(),
            })
            .expect("append");
        let incremental = read_from(&path, offset).expect("incremental read");
        assert_eq!(incremental.envelopes.len(), 1);
        assert!(matches!(
            &incremental.envelopes[0].1.event,
            SwarmControlEvent::TaskStatusChanged { status, .. } if status == "completed"
        ));
    }

    #[test]
    fn sequence_numbers_survive_reopen() {
        let (_dir, path) = temp_log();
        {
            let mut writer = ControlLogWriter::open(&path, "s", LOCAL_ORIGIN).expect("open");
            writer
                .append(SwarmControlEvent::MemberLeft {
                    session_id: "x".into(),
                })
                .expect("append");
        }
        let mut writer = ControlLogWriter::open(&path, "s", LOCAL_ORIGIN).expect("reopen");
        let envelope = writer
            .append(SwarmControlEvent::MemberLeft {
                session_id: "y".into(),
            })
            .expect("append");
        assert_eq!(envelope.seq, 1, "seq must continue after reopen, not reset");
    }

    #[test]
    fn torn_final_line_is_not_consumed_and_completes_later() {
        let (_dir, path) = temp_log();
        let mut writer = ControlLogWriter::open(&path, "s", LOCAL_ORIGIN).expect("open");
        writer
            .append(SwarmControlEvent::MemberLeft {
                session_id: "a".into(),
            })
            .expect("append");
        // Simulate a torn concurrent write: half a line, no newline.
        {
            use std::io::Write as _;
            let mut file = std::fs::OpenOptions::new()
                .append(true)
                .open(&path)
                .expect("open raw");
            file.write_all(b"{\"origin\":\"local\",\"seq\":9").expect("torn");
        }
        let read = read_from(&path, 0).expect("read");
        assert_eq!(read.envelopes.len(), 1, "torn line must not be parsed");
        let stop = read.next_offset;
        // Complete the line into valid JSON; resume from the stored offset.
        {
            use std::io::Write as _;
            let mut file = std::fs::OpenOptions::new()
                .append(true)
                .open(&path)
                .expect("open raw");
            file.write_all(
                b",\"wall_ms\":1,\"swarm_id\":\"s\",\"event\":{\"type\":\"member_left\",\"session_id\":\"b\"}}\n",
            )
            .expect("complete");
        }
        let resumed = read_from(&path, stop).expect("resume");
        assert_eq!(resumed.envelopes.len(), 1);
        assert!(matches!(
            &resumed.envelopes[0].1.event,
            SwarmControlEvent::MemberLeft { session_id } if session_id == "b"
        ));
    }

    #[test]
    fn corrupt_complete_line_is_skipped_without_wedging_replay() {
        let (_dir, path) = temp_log();
        let mut writer = ControlLogWriter::open(&path, "s", LOCAL_ORIGIN).expect("open");
        writer
            .append(SwarmControlEvent::MemberLeft {
                session_id: "a".into(),
            })
            .expect("append");
        {
            use std::io::Write as _;
            let mut file = std::fs::OpenOptions::new()
                .append(true)
                .open(&path)
                .expect("open raw");
            file.write_all(b"this is not json\n").expect("garbage");
        }
        writer
            .append(SwarmControlEvent::MemberLeft {
                session_id: "b".into(),
            })
            .expect("append");
        let (state, _) = replay(&path).expect("replay");
        assert_eq!(state.events_applied, 2, "good events on both sides of garbage");
    }

    #[test]
    fn coordinator_is_derived_and_survives_handoff() {
        // F4's class by construction: role has exactly one home (the log),
        // and the coordinator view is a pure derivation.
        let mut state = SwarmControlState::default();
        state.apply(&SwarmControlEvent::MemberJoined {
            session_id: "a".into(),
            friendly_name: None,
            role: "coordinator".into(),
        });
        state.apply(&SwarmControlEvent::MemberJoined {
            session_id: "b".into(),
            friendly_name: None,
            role: "agent".into(),
        });
        assert_eq!(state.coordinator(), Some("a"));
        // Handoff: b promoted, a demoted. Between the two events the fold is
        // deterministic (min session id), never "no coordinator".
        state.apply(&SwarmControlEvent::RoleChanged {
            session_id: "b".into(),
            role: "coordinator".into(),
        });
        assert_eq!(state.coordinator(), Some("a"), "deterministic during handoff");
        state.apply(&SwarmControlEvent::RoleChanged {
            session_id: "a".into(),
            role: "agent".into(),
        });
        assert_eq!(state.coordinator(), Some("b"));
        // Coordinator leaving leaves no coordinator (self-promotion path
        // exists at the server layer; the fold never invents one).
        state.apply(&SwarmControlEvent::MemberLeft {
            session_id: "b".into(),
        });
        assert_eq!(state.coordinator(), None);
    }
}
