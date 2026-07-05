// W3a (orchestration-hardening): run_plan's end-of-plan cleanup only runs on
// the success path. Every driver failure (stall, await timeout, max loops)
// exits through Err and leaks the spawned workers: they sit "ready" forever,
// counting against the member cap and cluttering the swarm. The fix must
// collect FINISHED owned workers on error paths too, while leaving running
// workers alive so the plan can be resumed.

#[tokio::test]
async fn communicate_run_plan_stall_still_collects_finished_workers() {
    let _env_lock = crate::storage::lock_test_env();
    let runtime_dir = tempfile::TempDir::new().expect("runtime tempdir");
    let repo_dir = std::env::current_dir().expect("repo cwd");
    let socket_path = runtime_dir.path().join("jcode.sock");
    let _runtime = EnvGuard::set("JCODE_RUNTIME_DIR", runtime_dir.path());
    let _socket = EnvGuard::set("JCODE_SOCKET", &socket_path);
    let _debug = EnvGuard::set("JCODE_DEBUG_CONTROL", "1");

    let provider: Arc<dyn Provider> = Arc::new(DelayedTestProvider {
        delay: Duration::from_millis(50),
    });
    let server = Arc::new(Server::new(provider));
    let mut server_task = {
        let server = Arc::clone(&server);
        tokio::spawn(async move { server.run().await })
    };

    wait_for_server_socket(&socket_path, &mut server_task)
        .await
        .expect("server socket should be ready");

    // Coordinator + a foreign live client session (peer). The peer is a
    // non-drivable member: tasks assigned to it wedge the driver.
    let mut watcher = RawClient::connect(&socket_path)
        .await
        .expect("watcher should connect");
    let mut peer = RawClient::connect(&socket_path)
        .await
        .expect("peer should connect");
    watcher
        .subscribe(&repo_dir)
        .await
        .expect("watcher subscribe");
    peer.subscribe(&repo_dir).await.expect("peer subscribe");

    let watcher_session = watcher.session_id().await.expect("watcher session id");
    let peer_session = peer.session_id().await.expect("peer session id");

    let tool = CommunicateTool::new();
    let ctx = test_ctx(&watcher_session, &repo_dir);

    tool.execute(
        json!({
            "action": "assign_role",
            "target_session": watcher_session,
            "role": "coordinator"
        }),
        ctx.clone(),
    )
    .await
    .expect("self-promotion to coordinator should succeed");

    // A finished owned worker: this is the resource the error path leaks.
    // Headless mode so the worker registers "ready" without a terminal app.
    let spawn_output = tool
        .execute(
            json!({"action": "spawn", "spawn_mode": "headless"}),
            ctx.clone(),
        )
        .await
        .expect("worker spawn should succeed");
    let worker_session = spawn_output
        .output
        .strip_prefix("Spawned new agent: ")
        .expect("spawn output should include session id")
        .trim()
        .to_string();
    wait_for_member_presence(&mut watcher, &watcher_session, &worker_session)
        .await
        .expect("spawned worker should appear in swarm");

    // Wedged plan: the only runnable task is assigned to the live foreign
    // peer, which run_plan cannot drive. Assignment finds nothing unassigned
    // (the assignee is live, so stale-reclaim correctly leaves it), nothing is
    // in flight, so the driver stalls and errors out.
    tool.execute(
        json!({
            "action": "propose_plan",
            "plan_items": [{
                "id": "wedged",
                "content": "task run_plan cannot drive",
                "status": "queued",
                "priority": "high",
                "assigned_to": peer_session
            }]
        }),
        ctx.clone(),
    )
    .await
    .expect("plan proposal should succeed");

    let error = tokio::time::timeout(
        Duration::from_secs(30),
        tool.execute(
            json!({
                "action": "run_plan",
                "background": false,
                "timeout_minutes": 1
            }),
            ctx.clone(),
        ),
    )
    .await
    .expect("run_plan should return promptly")
    .expect_err("run_plan should fail on the wedged plan");
    let message = error.to_string();
    assert!(
        message.contains("stalled"),
        "expected a stall error, got: {message}"
    );

    // The point: the finished worker must have been collected despite the
    // failure. Success-only cleanup leaves it 'ready' forever (the leak).
    let members = watcher
        .comm_list(&watcher_session)
        .await
        .expect("comm_list should succeed");
    let leaked = members
        .iter()
        .find(|member| member.session_id == worker_session);
    assert!(
        leaked.is_none()
            || leaked.is_some_and(|member| member.status.as_deref() == Some("stopped")),
        "W3a reproduced: run_plan error path leaked finished worker {} (status {:?}); \
         end-of-plan cleanup must also run on failure exits",
        worker_session,
        leaked.and_then(|member| member.status.as_deref())
    );
    // And the error message must tell the operator what was collected vs
    // retained, not claim blanket retention.
    assert!(
        !message.contains("Spawned workers were retained"),
        "error hint should reflect that finished workers were collected, got: {message}"
    );

    server_task.abort();
}
