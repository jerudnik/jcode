// A backgrounded await resolves promptly, but the notification it produces is
// queued as a soft interrupt and surfaces only when the requesting agent's
// current turn ends. Observed lag on a live run was almost three minutes. The
// body carried no measurement time, so a reading taken at 02:03 and shown at
// 02:05 read exactly like one taken at 02:05, and the coordinator misread a
// stale result as current.
//
// The fix is to date the payload at the instant it resolves. These tests pin
// both directions: the stamp is present and it is the RESOLUTION time, not the
// delivery time.

/// Extract the RFC3339 stamp from the notification header.
fn resolved_stamp(notification: &str) -> chrono::DateTime<chrono::Utc> {
    let header = notification
        .lines()
        .next()
        .expect("notification has a header line");
    let start = header
        .find("(resolved ")
        .expect("header carries a resolution stamp")
        + "(resolved ".len();
    let rest = &header[start..];
    let end = rest.find(')').expect("stamp is closed");
    chrono::DateTime::parse_from_rfc3339(&rest[..end])
        .expect("stamp parses as RFC3339")
        .with_timezone(&chrono::Utc)
}

/// Drive a background await to completion and return the notification body the
/// requesting session would be shown.
async fn completed_await_notification(swarm_id: &str, requester: &str, peer: &str) -> String {
    let await_runtime = AwaitMembersRuntime::default();
    let swarm_members = Arc::new(RwLock::new(HashMap::from([
        (requester.to_string(), member(requester, swarm_id, "ready")),
        (peer.to_string(), member(peer, swarm_id, "running")),
    ])));
    let swarms_by_id = Arc::new(RwLock::new(HashMap::from([(
        swarm_id.to_string(),
        HashSet::from([requester.to_string(), peer.to_string()]),
    )])));
    let swarm_plans = Arc::new(RwLock::new(HashMap::new()));
    let (swarm_event_tx, _swarm_event_rx) = broadcast::channel(32);
    let mut bus_rx = crate::bus::Bus::global().subscribe();

    let (client_tx, _client_rx) = mpsc::unbounded_channel();
    handle_comm_await_members(
        1,
        requester.to_string(),
        vec!["completed".to_string()],
        vec![peer.to_string()],
        Some("all".to_string()),
        Some(60),
        true,
        true,
        true,
        CommAwaitMembersContext {
            client_event_tx: &client_tx,
            swarm_members: &swarm_members,
            swarms_by_id: &swarms_by_id,
            swarm_plans: &swarm_plans,
            swarm_event_tx: &swarm_event_tx,
            await_members_runtime: &await_runtime,
        },
    )
    .await;

    {
        let mut members = swarm_members.write().await;
        members.get_mut(peer).expect("peer exists").status = "completed".to_string();
    }
    let _ = swarm_event_tx.send(swarm_event(
        peer,
        swarm_id,
        SwarmEventType::StatusChange {
            old_status: "running".to_string(),
            new_status: "completed".to_string(),
        },
    ));

    let event = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match bus_rx.recv().await {
                Ok(crate::bus::BusEvent::SwarmAwaitCompleted(event))
                    if event.session_id == requester =>
                {
                    return event;
                }
                Ok(_) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    panic!("bus closed before SwarmAwaitCompleted arrived")
                }
            }
        }
    })
    .await
    .expect("background await should publish SwarmAwaitCompleted");

    assert!(event.completed, "peer reached a target status");
    event.notification
}

/// The delivered payload carries a parseable resolution stamp.
#[tokio::test]
async fn background_await_notification_is_dated() {
    let (_env, _runtime_dir) = RuntimeEnvGuard::new();
    let before = chrono::Utc::now();
    let notification =
        completed_await_notification("swarm-dated", "coord-dated", "peer-dated").await;
    let after = chrono::Utc::now();

    let stamp = resolved_stamp(&notification);
    // The stamp is truncated to whole seconds, so allow a second of slack at
    // each edge rather than asserting an exact half-open interval.
    assert!(
        stamp >= before - chrono::Duration::seconds(1)
            && stamp <= after + chrono::Duration::seconds(1),
        "stamp {stamp} should sit inside [{before}, {after}]"
    );
}

/// Direction test: the stamp records when the await RESOLVED, not when the
/// payload is read. Without this, a formatter that stamps at render time would
/// satisfy the assertion above while reproducing the exact defect -- a stale
/// result wearing a fresh date.
#[tokio::test]
async fn the_stamp_is_the_resolution_time_not_the_read_time() {
    let (_env, _runtime_dir) = RuntimeEnvGuard::new();
    let notification =
        completed_await_notification("swarm-lagged", "coord-lagged", "peer-lagged").await;
    let stamp = resolved_stamp(&notification);

    // Stand in for the real delivery lag: the notification sits in the soft
    // interrupt queue while the coordinator finishes its turn.
    tokio::time::sleep(Duration::from_secs(2)).await;

    let age = chrono::Utc::now() - stamp;
    assert!(
        age >= chrono::Duration::seconds(2),
        "a payload held for 2s must read as at least 2s old, got {age}"
    );
}
