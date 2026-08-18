fn awaited_member(session_id: &str, done: bool) -> AwaitedMemberStatus {
    AwaitedMemberStatus {
        session_id: session_id.to_string(),
        friendly_name: Some(session_id.to_string()),
        status: if done { "completed" } else { "running" }.to_string(),
        done,
        completion_report: None,
        last_activity_age_secs: None,
    }
}

#[test]
fn awaited_members_header_all_done() {
    let members = vec![awaited_member("fox", true), awaited_member("wolf", true)];
    let output = format_comm_awaited_members_with_reports(
        true,
        "All 2 members are done: fox, wolf",
        &members,
        &std::collections::HashMap::new(),
    );
    assert!(
        output.starts_with("All members done."),
        "expected all-done header, got: {output}"
    );
}

#[test]
fn awaited_members_header_any_mode_partial_match() {
    let members = vec![awaited_member("fox", true), awaited_member("wolf", false)];
    let output = format_comm_awaited_members_with_reports(
        true,
        "Matched 1 member: fox",
        &members,
        &std::collections::HashMap::new(),
    );
    assert!(
        output.starts_with("Await satisfied."),
        "any-mode partial match must not claim all members are done, got: {output}"
    );
    assert!(!output.starts_with("All members done."));
}

#[test]
fn awaited_members_header_incomplete() {
    let members = vec![awaited_member("fox", false)];
    let output = format_comm_awaited_members_with_reports(
        false,
        "Timed out. Still waiting on: fox (running)",
        &members,
        &std::collections::HashMap::new(),
    );
    assert!(
        output.starts_with("Await incomplete."),
        "expected incomplete header, got: {output}"
    );
}

/// A wedged worker and a working worker both report lifecycle status
/// "running". Without an activity age they render identically, so a wait
/// cannot tell the caller which one it is looking at. This is the test that
/// the pre-change formatter fails: it emitted `wolf (running)` for both.
#[test]
fn pending_member_activity_distinguishes_stuck_from_working() {
    let render = |age: Option<u64>| {
        let mut member = awaited_member("wolf", false);
        member.last_activity_age_secs = age;
        format_comm_awaited_members_with_reports(
            false,
            "Timed out. Still waiting on: wolf",
            &[member],
            &std::collections::HashMap::new(),
        )
    };

    let working = render(Some(5));
    let wedged = render(Some(1800));

    assert_ne!(
        working, wedged,
        "a worker active 5s ago must not render the same as one silent for 30m"
    );
    assert!(
        working.contains("active 5s ago"),
        "expected recent activity, got: {working}"
    );
    assert!(
        wedged.contains("no activity for 30m"),
        "expected a stall notice, got: {wedged}"
    );
    // A member that finished needs no liveness evidence.
    let mut done = awaited_member("fox", true);
    done.last_activity_age_secs = Some(1800);
    let done_output = format_comm_awaited_members_with_reports(
        true,
        "All 1 members are done: fox",
        &[done],
        &std::collections::HashMap::new(),
    );
    assert!(
        !done_output.contains("no activity"),
        "a completed member must not be flagged as stalled, got: {done_output}"
    );
}
