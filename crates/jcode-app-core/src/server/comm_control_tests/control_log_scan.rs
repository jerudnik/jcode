// W2 plumbing: the server-facing await-on-offset scan wrapper
// (`scan_swarm_control_log`) anchored at a tail cursor
// (`current_control_log_offset`). Exercises the nudge-vs-truth contract the W2
// await watcher will build on: anchor at the tail, and an ArtifactFiled past
// that cursor wakes exactly once (a re-scan past the match sees nothing).

use crate::server::control_log_sync::{
    append_control_event, current_control_log_offset, scan_swarm_control_log,
};
use jcode_swarm_core::control_log::{ScanOutcome, SwarmControlEvent};

#[test]
fn scan_from_tail_offset_finds_artifact_once() {
    let (_env, _runtime) = RuntimeEnvGuard::new();
    let swarm_id = "swarm-scan-plumbing";

    // A couple of pre-await control events: these must NOT satisfy the artifact
    // await (they are exactly the pre-await history a tail anchor skips).
    append_control_event(
        swarm_id,
        SwarmControlEvent::TaskAssigned {
            task_id: "t1".to_string(),
            assigned_to: Some("w1".to_string()),
        },
    )
    .expect("append TaskAssigned");
    append_control_event(
        swarm_id,
        SwarmControlEvent::MemberStatusChanged {
            session_id: "w1".to_string(),
            status: "running".to_string(),
        },
    )
    .expect("append MemberStatusChanged");

    // Anchor a new await at the tail: everything above is pre-await history.
    let tail = current_control_log_offset(swarm_id);
    assert!(tail > 0, "tail offset must reflect the appended events");

    let wants_artifact = |envelope: &jcode_swarm_core::control_log::SwarmControlEnvelope| {
        matches!(
            &envelope.event,
            SwarmControlEvent::ArtifactFiled { task_id, .. } if task_id == "t1"
        )
    };

    // Nothing past the tail yet: NotYet, and the re-arm offset does not rewind.
    match scan_swarm_control_log(swarm_id, tail, wants_artifact).expect("scan tail") {
        ScanOutcome::NotYet { resume_offset } => {
            assert_eq!(resume_offset, tail, "empty scan must not rewind the cursor")
        }
        ScanOutcome::Found { .. } => panic!("pre-await events must not satisfy the artifact await"),
    }

    // Evidence arrives after the await was armed.
    append_control_event(
        swarm_id,
        SwarmControlEvent::ArtifactFiled {
            task_id: "t1".to_string(),
            session_id: "w1".to_string(),
            confidence: Some("high".to_string()),
        },
    )
    .expect("append ArtifactFiled");

    // The scan from the captured tail offset wakes on the artifact exactly.
    let next = match scan_swarm_control_log(swarm_id, tail, wants_artifact).expect("scan found") {
        ScanOutcome::Found {
            next_offset,
            envelope,
        } => {
            assert!(
                matches!(
                    &envelope.event,
                    SwarmControlEvent::ArtifactFiled { session_id, confidence: Some(c), .. }
                        if session_id == "w1" && c == "high"
                ),
                "matched envelope must be the ArtifactFiled evidence"
            );
            next_offset
        }
        ScanOutcome::NotYet { .. } => panic!("artifact past the tail must wake the await"),
    };

    // Re-scanning past the match sees nothing further: no double-wake.
    match scan_swarm_control_log(swarm_id, next, wants_artifact).expect("scan post-match") {
        ScanOutcome::NotYet { resume_offset } => {
            assert_eq!(resume_offset, next, "post-match scan must hold the cursor")
        }
        ScanOutcome::Found { .. } => panic!("the same artifact must not double-wake the await"),
    }
}
