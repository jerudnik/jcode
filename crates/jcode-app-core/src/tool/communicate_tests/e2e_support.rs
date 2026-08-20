struct EnvGuard {
    key: &'static str,
    original: Option<std::ffi::OsString>,
}

impl EnvGuard {
    fn set(key: &'static str, value: impl AsRef<std::ffi::OsStr>) -> Self {
        let original = std::env::var_os(key);
        crate::env::set_var(key, value);
        Self { key, original }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        if let Some(value) = self.original.take() {
            crate::env::set_var(self.key, value);
        } else {
            crate::env::remove_var(self.key);
        }
    }
}

struct DelayedTestProvider {
    delay: Duration,
}

#[async_trait]
impl Provider for DelayedTestProvider {
    async fn complete(
        &self,
        _messages: &[Message],
        _tools: &[ToolDefinition],
        _system: &str,
        _resume_session_id: Option<&str>,
    ) -> Result<EventStream> {
        let delay = self.delay;
        let stream = futures::stream::once(async move {
            tokio::time::sleep(delay).await;
            Ok(StreamEvent::TextDelta("ok".to_string()))
        })
        .chain(futures::stream::once(async {
            Ok(StreamEvent::MessageEnd { stop_reason: None })
        }));
        Ok(Box::pin(stream))
    }

    fn name(&self) -> &str {
        "test"
    }

    fn fork(&self) -> Arc<dyn Provider> {
        Arc::new(Self { delay: self.delay })
    }
}

#[derive(Clone)]
struct FailingTestProvider {
    calls: Arc<std::sync::atomic::AtomicUsize>,
    message: &'static str,
}

#[async_trait]
impl Provider for FailingTestProvider {
    async fn complete(
        &self,
        _messages: &[Message],
        _tools: &[ToolDefinition],
        _system: &str,
        _resume_session_id: Option<&str>,
    ) -> Result<EventStream> {
        self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Err(anyhow::anyhow!(self.message))
    }

    fn name(&self) -> &str {
        "test-failing"
    }

    fn fork(&self) -> Arc<dyn Provider> {
        Arc::new(self.clone())
    }
}

struct RawClient {
    reader: BufReader<ReadHalf>,
    writer: WriteHalf,
    next_id: u64,
}

impl RawClient {
    async fn connect(path: &Path) -> Result<Self> {
        let stream = Stream::connect(path).await?;
        let (reader, writer) = stream.into_split();
        Ok(Self {
            reader: BufReader::new(reader),
            writer,
            next_id: 1,
        })
    }

    async fn send_request(&mut self, request: Request) -> Result<u64> {
        let id = request.id();
        let json = serde_json::to_string(&request)? + "\n";
        self.writer.write_all(json.as_bytes()).await?;
        Ok(id)
    }

    async fn read_event(&mut self) -> Result<ServerEvent> {
        let mut line = String::new();
        let n = self.reader.read_line(&mut line).await?;
        if n == 0 {
            anyhow::bail!("server disconnected")
        }
        Ok(serde_json::from_str(&line)?)
    }

    async fn read_until<F>(&mut self, timeout: Duration, mut predicate: F) -> Result<ServerEvent>
    where
        F: FnMut(&ServerEvent) -> bool,
    {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            let event = tokio::time::timeout(remaining, self.read_event()).await??;
            if predicate(&event) {
                return Ok(event);
            }
        }
    }

    async fn subscribe(&mut self, working_dir: &Path) -> Result<()> {
        let id = self.next_id;
        self.next_id += 1;
        self.send_request(Request::Subscribe {
            id,
            working_dir: Some(working_dir.display().to_string()),
            selfdev: None,
            target_session_id: None,
            client_instance_id: None,
            client_has_local_history: false,
            allow_session_takeover: false,
            terminal_env: Vec::new(),
            protocol_version: None,
            build_hash: None,
            runtime_identity: None,
            spawn_swarm_id: None,
            spawn_session_id: None,
            client_pid: None,
        })
        .await?;
        self.read_until(
            E2E_SETUP_BUDGET,
            |event| matches!(event, ServerEvent::Done { id: done_id } if *done_id == id),
        )
        .await?;
        Ok(())
    }

    /// Subscribe while advertising an explicit NS1 protocol/build identity, and
    /// return the request id so the caller can match the server's
    /// `HandshakeVerdict`/`Done` events. Unlike [`Self::subscribe`], this does
    /// not drain to `Done`, so the test can observe the verdict event ordering
    /// on the wire.
    async fn subscribe_with_identity(
        &mut self,
        working_dir: &Path,
        protocol_version: Option<u32>,
        build_hash: Option<String>,
    ) -> Result<u64> {
        self.subscribe_with_identity_and_pid(working_dir, protocol_version, build_hash, None)
            .await
    }

    async fn subscribe_with_identity_and_pid(
        &mut self,
        working_dir: &Path,
        protocol_version: Option<u32>,
        build_hash: Option<String>,
        client_pid: Option<u32>,
    ) -> Result<u64> {
        let id = self.next_id;
        self.next_id += 1;
        self.send_request(Request::Subscribe {
            id,
            working_dir: Some(working_dir.display().to_string()),
            selfdev: None,
            target_session_id: None,
            client_instance_id: None,
            client_has_local_history: false,
            allow_session_takeover: false,
            terminal_env: Vec::new(),
            protocol_version,
            build_hash,
            runtime_identity: None,
            spawn_swarm_id: None,
            spawn_session_id: None,
            client_pid,
        })
        .await?;
        Ok(id)
    }

    async fn session_id(&mut self) -> Result<String> {
        let id = self.next_id;
        self.next_id += 1;
        self.send_request(Request::GetState { id }).await?;
        match self
            .read_until(
                E2E_FLOW_BUDGET,
                |event| matches!(event, ServerEvent::State { id: event_id, .. } if *event_id == id),
            )
            .await?
        {
            ServerEvent::State { session_id, .. } => Ok(session_id),
            other => anyhow::bail!("unexpected state response: {other:?}"),
        }
    }

    async fn send_message(&mut self, content: &str) -> Result<u64> {
        let id = self.next_id;
        self.next_id += 1;
        self.send_request(Request::Message {
            id,
            content: content.to_string(),
            images: vec![],
            system_reminder: None,
        })
        .await
    }

    async fn wait_for_done(&mut self, request_id: u64) -> Result<()> {
        self.read_until(
            Duration::from_secs(10),
            |event| matches!(event, ServerEvent::Done { id } if *id == request_id),
        )
        .await?;
        Ok(())
    }

    async fn comm_list(&mut self, session_id: &str) -> Result<Vec<AgentInfo>> {
        let id = self.next_id;
        self.next_id += 1;
        self.send_request(Request::CommList {
            id,
            session_id: session_id.to_string(),
        })
        .await?;
        match self
                .read_until(E2E_FLOW_BUDGET, |event| {
                    matches!(event, ServerEvent::CommMembers { id: event_id, .. } if *event_id == id)
                })
                .await?
            {
                ServerEvent::CommMembers { members, .. } => Ok(members),
                other => anyhow::bail!("unexpected comm_list response: {other:?}"),
            }
    }

    async fn comm_status(
        &mut self,
        session_id: &str,
        target_session: &str,
    ) -> Result<AgentStatusSnapshot> {
        let id = self.next_id;
        self.next_id += 1;
        self.send_request(Request::CommStatus {
            id,
            session_id: session_id.to_string(),
            target_session: target_session.to_string(),
        })
        .await?;
        match self
                .read_until(E2E_FLOW_BUDGET, |event| {
                    matches!(event, ServerEvent::CommStatusResponse { id: event_id, .. } if *event_id == id)
                })
                .await?
            {
                ServerEvent::CommStatusResponse { snapshot, .. } => Ok(snapshot),
                other => anyhow::bail!("unexpected comm_status response: {other:?}"),
            }
    }

    /// Wait for the next `Message` notification and return its scope
    /// ("dm" or "broadcast"). Other events are skipped.
    async fn next_message_notification(&mut self, timeout: Duration) -> Result<Option<String>> {
        match self
            .read_until(timeout, |event| {
                matches!(
                    event,
                    ServerEvent::Notification {
                        notification_type: NotificationType::Message { .. },
                        ..
                    }
                )
            })
            .await?
        {
            ServerEvent::Notification {
                notification_type: NotificationType::Message { scope, .. },
                ..
            } => Ok(scope),
            other => anyhow::bail!("unexpected notification response: {other:?}"),
        }
    }
}

const E2E_FLOW_BUDGET: Duration = Duration::from_secs(30);
/// Setup-only headroom. See `docs/issues/comm-e2e-tests-flake-under-saturation.md`.
const E2E_SETUP_BUDGET: Duration = Duration::from_secs(60);

async fn wait_for_server_socket(
    path: &Path,
    server_task: &mut tokio::task::JoinHandle<Result<crate::server::ServerExit>>,
) -> Result<()> {
    let deadline = tokio::time::Instant::now() + E2E_SETUP_BUDGET;
    loop {
        if server_task.is_finished() {
            let result = server_task.await?;
            return Err(anyhow::anyhow!(
                "server exited before socket became ready: {:?}",
                result
            ));
        }
        match Stream::connect(path).await {
            Ok(stream) => {
                drop(stream);
                return Ok(());
            }
            Err(err) => {
                if tokio::time::Instant::now() >= deadline {
                    return Err(err.into());
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        }
    }
}

fn test_ctx(session_id: &str, working_dir: &Path) -> ToolContext {
    ToolContext {
        session_id: session_id.to_string(),
        message_id: "msg-1".to_string(),
        tool_call_id: "call-1".to_string(),
        working_dir: Some(working_dir.to_path_buf()),
        stdin_request_tx: None,
        graceful_shutdown_signal: None,
        execution_mode: ToolExecutionMode::Direct,
    }
}

async fn wait_for_member_status(
    client: &mut RawClient,
    requester_session: &str,
    target_session: &str,
    expected_status: &str,
) -> Result<Vec<AgentInfo>> {
    let deadline = tokio::time::Instant::now() + E2E_FLOW_BUDGET;
    loop {
        let members = client.comm_list(requester_session).await?;
        if members
            .iter()
            .find(|member| member.session_id == target_session)
            .and_then(|member| member.status.as_deref())
            == Some(expected_status)
        {
            return Ok(members);
        }
        if tokio::time::Instant::now() >= deadline {
            anyhow::bail!(
                "timed out waiting for member {} to reach status {}",
                target_session,
                expected_status
            );
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

async fn wait_for_member_presence(
    client: &mut RawClient,
    requester_session: &str,
    target_session: &str,
) -> Result<Vec<AgentInfo>> {
    let deadline = tokio::time::Instant::now() + E2E_FLOW_BUDGET;
    loop {
        let members = client.comm_list(requester_session).await?;
        if members
            .iter()
            .any(|member| member.session_id == target_session)
        {
            return Ok(members);
        }
        if tokio::time::Instant::now() >= deadline {
            anyhow::bail!("timed out waiting for member {} to appear", target_session);
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}
