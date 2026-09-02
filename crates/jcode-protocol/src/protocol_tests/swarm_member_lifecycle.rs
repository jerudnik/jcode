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
