fn minimal_swarm_member_json(status: &str) -> serde_json::Value {
    serde_json::json!({
        "session_id": "worker-1",
        "friendly_name": null,
        "status": status,
        "detail": null,
        "runtime": {}
    })
}

#[test]
fn old_swarm_member_payload_falls_back_to_compatibility_status() {
    let member: SwarmMemberStatus =
        serde_json::from_value(minimal_swarm_member_json("completed")).unwrap();

    assert_eq!(member.lifecycle, None);
    assert_eq!(
        member.lifecycle_state(),
        jcode_swarm_core::MemberLifecycleState::Succeeded
    );
}

#[test]
fn typed_swarm_member_lifecycle_wins_over_stale_compatibility_status() {
    let mut payload = minimal_swarm_member_json("ready");
    payload["lifecycle"] = serde_json::json!("failed");
    let mut member: SwarmMemberStatus = serde_json::from_value(payload).unwrap();

    assert_eq!(
        member.lifecycle_state(),
        jcode_swarm_core::MemberLifecycleState::Failed
    );
    member.normalize_lifecycle();
    assert_eq!(member.lifecycle_status(), "failed");
    assert_eq!(member.status, "ready", "compatibility output remains intact");
}

#[test]
fn fleet_member_lifecycle_is_backward_compatible_and_prefers_typed_state() {
    let mut member: SwarmFleetMember = serde_json::from_value(serde_json::json!({
        "session_id": "worker-1",
        "friendly_name": null,
        "status": "completed"
    }))
    .unwrap();
    assert_eq!(
        member.lifecycle_state(),
        jcode_swarm_core::MemberLifecycleState::Succeeded
    );

    member.status = "ready".to_string();
    member.lifecycle = Some(jcode_swarm_core::MemberLifecycleState::Failed);
    assert_eq!(member.lifecycle_status(), "failed");
}

#[test]
fn swarm_status_age_uses_latest_epoch_millisecond_evidence() {
    let lifecycle = jcode_swarm_core::SwarmLifecycleStatus::starting(10_000);

    assert_eq!(latest_swarm_evidence_unix_ms(&lifecycle, None), Some(10_000));
    assert_eq!(swarm_status_age_secs(15_999, &lifecycle, None), 5);
    assert_eq!(
        latest_swarm_evidence_unix_ms(&lifecycle, Some(15_000)),
        Some(15_000)
    );
    assert_eq!(swarm_status_age_secs(15_999, &lifecycle, Some(15_000)), 0);
    assert_eq!(
        swarm_status_age_secs(14_000, &lifecycle, Some(15_000)),
        0,
        "wall-clock skew must saturate instead of underflowing"
    );
}

#[test]
fn swarm_status_age_treats_zero_lifecycle_timestamp_as_unknown() {
    let lifecycle = jcode_swarm_core::SwarmLifecycleStatus::default();

    assert_eq!(latest_swarm_evidence_unix_ms(&lifecycle, None), None);
    assert_eq!(swarm_status_age_secs(1_800_000_000_000, &lifecycle, None), 0);
}
