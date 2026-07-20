//! F13 concurrency probe: background live-task cap under concurrent spawn
//! bursts and mid-setup cancellation.
//!
//! Compiled OUTSIDE crates/ against the workspace's built rlibs (see
//! run_probe.sh). Uses only the public API of jcode_base::background.
//!
//! Gate 1 checks:
//!   A. Burst: 16 concurrent spawns at cap=2 -> exactly <=2 accepted, the
//!      rest carry an explicit `refused` reason. Never over-admission.
//!   B. No leaked capacity: cancel the accepted tasks, wait for pruning,
//!      then a fresh spawn must be accepted (a leaked SpawnSlot or live-map
//!      entry would refuse it forever).
//!   C. Cancellation mid-setup: race spawn futures against tiny timeouts so
//!      some are dropped between reservation and live-map insert. After the
//!      dust settles, capacity must be fully recoverable (spawn succeeds).
//!
//! Exit code 0 = all invariants held. Non-zero = violation (printed).

use jcode_base::background::{BackgroundTaskManager, TaskResult};
use std::sync::Arc;
use std::time::Duration;

const CAP: usize = 2;
const BURST: usize = 16;

fn main() {
    // Cap is read from env at each admission check.
    std::env::set_var("JCODE_MAX_BACKGROUND_TASKS", CAP.to_string());
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(8)
        .enable_all()
        .build()
        .expect("runtime");
    let code = rt.block_on(run());
    std::process::exit(code);
}

async fn run() -> i32 {
    let tmp = std::env::temp_dir().join(format!("f13-probe-{}", std::process::id()));
    let manager = Arc::new(BackgroundTaskManager::with_output_dir(tmp.clone()));

    // ---- A: concurrent burst at cap boundary --------------------------------
    let mut joins = Vec::new();
    for i in 0..BURST {
        let m = Arc::clone(&manager);
        joins.push(tokio::spawn(async move {
            m.spawn_with_notify("probe", None, &format!("s{i}"), false, false, |_p| async {
                tokio::time::sleep(Duration::from_secs(300)).await;
                Ok(TaskResult::completed(Some(0)))
            })
            .await
        }));
    }
    let mut accepted = Vec::new();
    let mut refused = 0usize;
    for j in joins {
        let info = j.await.expect("join");
        match &info.refused {
            None => accepted.push(info.task_id),
            Some(reason) => {
                assert!(
                    reason.contains("cap reached"),
                    "refusal must be the cap refusal, got: {reason}"
                );
                refused += 1;
            }
        }
    }
    println!("burst: accepted={} refused={}", accepted.len(), refused);
    if accepted.len() > CAP {
        eprintln!("VIOLATION: over-admission {} > cap {}", accepted.len(), CAP);
        return 1;
    }
    if accepted.len() + refused != BURST {
        eprintln!("VIOLATION: lost spawns");
        return 1;
    }
    let snap = manager.capacity_snapshot().await;
    if snap.current > CAP {
        eprintln!("VIOLATION: live map {} > cap {}", snap.current, CAP);
        return 1;
    }

    // ---- B: cancellation releases capacity, no leak -------------------------
    for id in &accepted {
        manager.cancel(id).await.expect("cancel");
    }
    if !wait_for_capacity(&manager, 0).await {
        eprintln!("VIOLATION: capacity leaked after cancel (current != 0)");
        return 2;
    }
    let re = manager
        .spawn_with_notify("probe", None, "reuse", false, false, |_p| async {
            Ok(TaskResult::completed(Some(0)))
        })
        .await;
    if re.refused.is_some() {
        eprintln!("VIOLATION: spawn refused after capacity release: {:?}", re.refused);
        return 2;
    }
    println!("release-after-cancel: ok");

    // ---- C: spawn futures dropped mid-setup must not leak reservations ------
    // Drop spawn_with_notify futures at random points via 0..=2ms timeouts.
    // A leaked SpawnSlot would permanently shrink capacity; verify full
    // capacity is still admittable afterwards.
    for round in 0..50u64 {
        let m = Arc::clone(&manager);
        let fut = m.spawn_with_notify("probe-c", None, "cancel-race", false, false, |_p| async {
            Ok(TaskResult::completed(Some(0)))
        });
        let _ = tokio::time::timeout(Duration::from_micros(round * 37 % 2500), fut).await;
    }
    // Let any admitted round-C tasks finish and prune.
    if !wait_for_capacity(&manager, 0).await {
        eprintln!("VIOLATION: capacity not restored after cancellation races");
        return 3;
    }
    // Full cap must still be admittable: no reservation leak.
    let mut final_accept = 0;
    for i in 0..CAP {
        let info = manager
            .spawn_with_notify("probe-final", None, &format!("f{i}"), false, false, |_p| async {
                tokio::time::sleep(Duration::from_secs(60)).await;
                Ok(TaskResult::completed(Some(0)))
            })
            .await;
        if info.refused.is_none() {
            final_accept += 1;
        }
    }
    if final_accept != CAP {
        eprintln!(
            "VIOLATION: leaked reservation, only {final_accept}/{CAP} admitted after races"
        );
        return 3;
    }
    println!("cancel-race: full cap ({CAP}) still admittable, no leak");
    println!("ALL INVARIANTS HELD");
    0
}

async fn wait_for_capacity(m: &BackgroundTaskManager, want: usize) -> bool {
    for _ in 0..300 {
        if m.capacity_snapshot().await.current == want {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    false
}
