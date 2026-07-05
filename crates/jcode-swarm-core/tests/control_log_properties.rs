//! W1 step 2: property tests for the control log.
//!
//! Two claims are load-bearing for the whole event-log design and must hold
//! before any server code writes to the log:
//!
//! 1. **Replay fidelity**: for ANY sequence of control events, folding the
//!    replayed JSONL file equals folding the in-memory sequence. (The file
//!    round-trip loses nothing and reorders nothing.)
//! 2. **Fold/graph agreement**: driving a task DAG through its real engine
//!    ops while mirroring each transition into the log yields a fold whose
//!    task view matches the graph's terminal state. (The event vocabulary is
//!    sufficient to reconstruct control state - if a transition can't be
//!    expressed, this test can't pass.)
//!
//! Randomness is a seeded LCG so failures reproduce exactly; no new deps.

use jcode_plan::dag::{
    self, Mode, NodeKind, NodeSpec, NodeStatus, TaskGraph, dispatch, ready_nodes, seed,
};
use jcode_swarm_core::control_log::{
    ControlLogWriter, LOCAL_ORIGIN, SwarmControlEvent, SwarmControlState, fold, replay,
};

/// Minimal deterministic PRNG (LCG, Numerical Recipes constants).
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.0 >> 33
    }

    fn pick(&mut self, bound: usize) -> usize {
        (self.next() % bound as u64) as usize
    }
}

fn arbitrary_event(rng: &mut Lcg) -> SwarmControlEvent {
    let sessions = ["s-a", "s-b", "s-c", "s-d"];
    let tasks = ["t-1", "t-2", "t-3"];
    let roles = ["agent", "coordinator", "worktree_manager"];
    let statuses = ["ready", "running", "queued", "failed", "completed"];
    match rng.pick(7) {
        0 => SwarmControlEvent::MemberJoined {
            session_id: sessions[rng.pick(sessions.len())].to_string(),
            friendly_name: (rng.pick(2) == 0).then(|| "name".to_string()),
            role: roles[rng.pick(roles.len())].to_string(),
        },
        1 => SwarmControlEvent::MemberLeft {
            session_id: sessions[rng.pick(sessions.len())].to_string(),
        },
        2 => SwarmControlEvent::RoleChanged {
            session_id: sessions[rng.pick(sessions.len())].to_string(),
            role: roles[rng.pick(roles.len())].to_string(),
        },
        3 => SwarmControlEvent::MemberStatusChanged {
            session_id: sessions[rng.pick(sessions.len())].to_string(),
            status: statuses[rng.pick(statuses.len())].to_string(),
        },
        4 => SwarmControlEvent::TaskAssigned {
            task_id: tasks[rng.pick(tasks.len())].to_string(),
            assigned_to: (rng.pick(3) != 0)
                .then(|| sessions[rng.pick(sessions.len())].to_string()),
        },
        5 => SwarmControlEvent::TaskStatusChanged {
            task_id: tasks[rng.pick(tasks.len())].to_string(),
            status: statuses[rng.pick(statuses.len())].to_string(),
        },
        _ => SwarmControlEvent::TaskHeartbeat {
            task_id: tasks[rng.pick(tasks.len())].to_string(),
            wall_ms: rng.next(),
        },
    }
}

/// Claim 1: fold(replay(written log)) == fold(in-memory events), across many
/// seeds and sequence lengths, including reopening the writer mid-sequence
/// (a process restart in miniature).
#[test]
fn replay_matches_in_memory_fold_for_arbitrary_sequences() {
    for seed_value in 0..25u64 {
        let mut rng = Lcg(seed_value.wrapping_mul(0x9E3779B97F4A7C15) | 1);
        let dir = tempfile::TempDir::new().expect("tempdir");
        let path = dir.path().join("prop.control.jsonl");
        let event_count = 1 + rng.pick(120);
        let reopen_at = rng.pick(event_count.max(1));

        let mut expected = SwarmControlState::default();
        let mut writer =
            ControlLogWriter::open(&path, "swarm-prop", LOCAL_ORIGIN).expect("open");
        for index in 0..event_count {
            if index == reopen_at {
                // Restart in miniature: drop and reopen the writer.
                writer =
                    ControlLogWriter::open(&path, "swarm-prop", LOCAL_ORIGIN).expect("reopen");
            }
            let event = arbitrary_event(&mut rng);
            expected.apply(&event);
            writer.append(event).expect("append");
        }

        let (replayed, _offset) = replay(&path).expect("replay");
        assert_eq!(
            replayed, expected,
            "seed {seed_value}: replayed fold diverged from in-memory fold \
             after {event_count} events (reopen at {reopen_at})"
        );

        // Sequence numbers must be strictly monotonic across the reopen.
        let read = jcode_swarm_core::control_log::read_from(&path, 0).expect("read");
        let seqs: Vec<u64> = read.envelopes.iter().map(|(_, e)| e.seq).collect();
        for pair in seqs.windows(2) {
            assert!(
                pair[1] == pair[0] + 1,
                "seed {seed_value}: non-monotonic seq {:?}",
                pair
            );
        }
    }
}

/// Claim 2: drive a real task graph through engine ops (dispatch, complete,
/// fail, take_over) with a seeded random policy, mirroring every transition
/// into the log. The folded task view must agree with the graph's final
/// state. This is the "dag sim as op generator" test from the migration
/// sketch: it proves the event vocabulary can express every transition the
/// engine can produce.
#[test]
fn fold_agrees_with_task_graph_driven_through_engine_ops() {
    for seed_value in 0..25u64 {
        let mut rng = Lcg(seed_value.wrapping_mul(0xD1B54A32D192ED03) | 1);
        let dir = tempfile::TempDir::new().expect("tempdir");
        let path = dir.path().join("dag.control.jsonl");
        let mut writer = ControlLogWriter::open(&path, "swarm-dag", LOCAL_ORIGIN).expect("open");

        // Random light-mode DAG: 3-8 nodes, forward-only dependencies.
        let node_count = 3 + rng.pick(6);
        let mut specs = Vec::new();
        for index in 0..node_count {
            let id = format!("n{index}");
            let mut spec = NodeSpec::new(&id, format!("task {id}"), NodeKind::Implement);
            if index > 0 && rng.pick(2) == 0 {
                let dep = format!("n{}", rng.pick(index));
                spec = spec.depends_on([dep.as_str()]);
            }
            specs.push(spec);
        }
        let mut graph = TaskGraph::new(Mode::Light);
        seed(&mut graph, specs).expect("seed dag");

        let workers = ["w-0", "w-1", "w-2"];
        for worker in workers {
            writer
                .append(SwarmControlEvent::MemberJoined {
                    session_id: worker.to_string(),
                    friendly_name: None,
                    role: "agent".to_string(),
                })
                .expect("append join");
        }

        // Drive to quiescence: each step dispatches one ready node to a random
        // worker and randomly completes, fails, or (sometimes) takes over then
        // completes. Every engine transition is mirrored as events.
        let mut guard = 0;
        loop {
            guard += 1;
            assert!(guard < 200, "seed {seed_value}: driver runaway");
            let ready: Vec<String> = ready_nodes(&graph)
                .into_iter()
                .map(|node| node.id.clone())
                .collect();
            let Some(node_id) = ready.first().cloned() else {
                break;
            };
            let worker = workers[rng.pick(workers.len())];
            assert!(dispatch(&mut graph, &node_id, worker));
            writer
                .append(SwarmControlEvent::TaskAssigned {
                    task_id: node_id.clone(),
                    assigned_to: Some(worker.to_string()),
                })
                .expect("append assign");
            writer
                .append(SwarmControlEvent::TaskStatusChanged {
                    task_id: node_id.clone(),
                    status: "running".to_string(),
                })
                .expect("append running");

            match rng.pick(4) {
                // Fail path (fix path not modeled: failed blocks dependents).
                0 => {
                    dag::fail_node(&mut graph, &node_id, worker).expect("fail");
                    writer
                        .append(SwarmControlEvent::TaskStatusChanged {
                            task_id: node_id.clone(),
                            status: "failed".to_string(),
                        })
                        .expect("append failed");
                }
                // Salvage path: owner "dies", coordinator takes over, completes.
                1 => {
                    let salvager = "coordinator";
                    dag::take_over_node(&mut graph, &node_id, salvager).expect("take over");
                    writer
                        .append(SwarmControlEvent::TaskAssigned {
                            task_id: node_id.clone(),
                            assigned_to: Some(salvager.to_string()),
                        })
                        .expect("append reassign");
                    dag::complete_node(
                        &mut graph,
                        &node_id,
                        salvager,
                        jcode_plan::dag::HandoffArtifact::brief("salvaged"),
                    )
                    .expect("complete after takeover");
                    writer
                        .append(SwarmControlEvent::TaskStatusChanged {
                            task_id: node_id.clone(),
                            status: "completed".to_string(),
                        })
                        .expect("append completed");
                }
                // Normal completion.
                _ => {
                    dag::complete_node(
                        &mut graph,
                        &node_id,
                        worker,
                        jcode_plan::dag::HandoffArtifact::brief("done"),
                    )
                    .expect("complete");
                    writer
                        .append(SwarmControlEvent::TaskStatusChanged {
                            task_id: node_id.clone(),
                            status: "completed".to_string(),
                        })
                        .expect("append completed");
                }
            }
        }

        // Agreement: every node the graph knows about must exist in the fold
        // with a matching terminal status and final owner.
        let (state, _offset) = replay(&path).expect("replay");
        for node in graph.nodes() {
            match node.status {
                NodeStatus::Done | NodeStatus::Failed | NodeStatus::Running => {
                    let task = state.tasks.get(node.id.as_str()).unwrap_or_else(|| {
                        panic!(
                            "seed {seed_value}: node {} touched by the engine is \
                             missing from the fold",
                            node.id
                        )
                    });
                    let expected_status = match node.status {
                        NodeStatus::Done => "completed",
                        NodeStatus::Failed => "failed",
                        _ => "running",
                    };
                    assert_eq!(
                        task.status, expected_status,
                        "seed {seed_value}: node {} status diverged",
                        node.id
                    );
                    assert_eq!(
                        task.assigned_to, node.owner,
                        "seed {seed_value}: node {} owner diverged",
                        node.id
                    );
                }
                // Never-dispatched nodes (queued behind failures) are
                // legitimately absent from the control log.
                NodeStatus::Queued => {}
            }
        }

        // And the same fidelity check as claim 1, for free.
        let read = jcode_swarm_core::control_log::read_from(&path, 0).expect("read");
        let refolded = fold(read.envelopes.iter().map(|(_, envelope)| envelope));
        assert_eq!(refolded, state);
    }
}
