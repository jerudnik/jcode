use super::*;
use futures::{SinkExt, StreamExt};
use serde_json::Value;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;
use tokio::time::timeout;
use tokio_tungstenite::MaybeTlsStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::handshake::server::Request;
use tokio_tungstenite::{WebSocketStream, connect_async};

#[test]
fn test_device_registry_pairing() {
    let _lock = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("temp home");
    let _home = EnvVarGuard::set_path("JCODE_HOME", temp.path());
    let mut registry = DeviceRegistry::default();

    // Generate pairing code
    let code = registry.generate_pairing_code();
    assert_eq!(code.len(), 6);
    assert_eq!(registry.pending_codes.len(), 1);

    // Validate correct code
    assert!(registry.validate_code(&code));
    assert_eq!(registry.pending_codes.len(), 0); // consumed

    // Validate again should fail (consumed)
    assert!(!registry.validate_code(&code));
}

#[test]
fn test_device_registry_token_auth() {
    let _lock = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("temp home");
    let _home = EnvVarGuard::set_path("JCODE_HOME", temp.path());
    let mut registry = DeviceRegistry::default();

    // Pair a device
    let token = registry.pair_device("test-device-1".to_string(), "Test iPhone".to_string(), None);

    // Validate correct token
    assert!(registry.validate_token(&token).is_some());
    let device = registry.validate_token(&token).unwrap();
    assert_eq!(device.name, "Test iPhone");
    assert_eq!(device.id, "test-device-1");

    // Validate wrong token
    assert!(registry.validate_token("wrong-token").is_none());

    // Token hash should be stored, not raw token
    assert!(registry.devices[0].token_hash.starts_with("sha256:"));
}

#[test]
fn test_device_re_pairing() {
    let _lock = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("temp home");
    let _home = EnvVarGuard::set_path("JCODE_HOME", temp.path());
    let mut registry = DeviceRegistry::default();

    // Pair same device twice
    let token1 = registry.pair_device("device-1".to_string(), "iPhone v1".to_string(), None);
    let token2 = registry.pair_device("device-1".to_string(), "iPhone v2".to_string(), None);

    // Only one device entry (old one replaced)
    assert_eq!(registry.devices.len(), 1);
    assert_eq!(registry.devices[0].name, "iPhone v2");

    // Old token should be invalid
    assert!(registry.validate_token(&token1).is_none());
    // New token should be valid
    assert!(registry.validate_token(&token2).is_some());
}

#[test]
fn test_parse_bearer_token() {
    assert_eq!(parse_bearer_token("Bearer abc"), Some("abc"));
    assert_eq!(parse_bearer_token("bearer abc"), Some("abc"));
    assert_eq!(parse_bearer_token("BEARER abc"), Some("abc"));
    assert_eq!(parse_bearer_token("Bearer"), None);
    assert_eq!(parse_bearer_token("Basic abc"), None);
    assert_eq!(parse_bearer_token("Bearer abc def"), None);
}

#[test]
fn test_parse_query_token() {
    assert_eq!(parse_query_token("token=abc"), Some("abc"));
    assert_eq!(parse_query_token("foo=bar&token=abc123"), Some("abc123"));
    assert_eq!(parse_query_token("token="), None);
    assert_eq!(parse_query_token("foo=bar"), None);
}

#[test]
fn test_hex_token_validation() {
    assert!(is_valid_hex_token(
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
    ));
    assert!(!is_valid_hex_token("abc"));
    assert!(!is_valid_hex_token(
        "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz"
    ));
}

#[test]
fn test_extract_ws_auth_prefers_header_and_falls_back_to_query() {
    let token_a = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let token_b = "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";

    let header_request = Request::builder()
        .uri("ws://example.com/ws")
        .header("authorization", format!("Bearer {token_a}"))
        .body(())
        .expect("request");
    let header_auth = extract_ws_auth(&header_request).expect("header auth");
    assert_eq!(header_auth.token, token_a);
    assert_eq!(header_auth.source, WsAuthSource::Header);

    let query_request = Request::builder()
        .uri(format!("ws://example.com/ws?token={token_b}"))
        .body(())
        .expect("request");
    let query_auth = extract_ws_auth(&query_request).expect("query auth");
    assert_eq!(query_auth.token, token_b);
    assert_eq!(query_auth.source, WsAuthSource::Query);
}

#[test]
fn test_extract_ws_auth_rejects_conflicting_sources() {
    let token_a = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let token_b = "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";

    let request = Request::builder()
        .uri(format!("ws://example.com/ws?token={token_b}"))
        .header("authorization", format!("Bearer {token_a}"))
        .body(())
        .expect("request");
    assert!(extract_ws_auth(&request).is_err());
}

#[test]
fn test_find_header_end() {
    assert_eq!(
        super::find_header_end(b"POST /pair HTTP/1.1\r\nContent-Length: 2\r\n\r\n{}"),
        Some(38)
    );
    assert_eq!(
        super::find_header_end(b"POST /pair HTTP/1.1\r\nContent-"),
        None
    );
    assert_eq!(super::find_header_end(b""), None);
}

struct EnvVarGuard {
    key: &'static str,
    previous: Option<std::ffi::OsString>,
}

impl EnvVarGuard {
    fn set_path(key: &'static str, value: &Path) -> Self {
        let previous = std::env::var_os(key);
        crate::env::set_var(key, value);
        Self { key, previous }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        if let Some(previous) = self.previous.as_ref() {
            crate::env::set_var(self.key, previous);
        } else {
            crate::env::remove_var(self.key);
        }
    }
}

struct GatewayFixture {
    addr: SocketAddr,
    client_rx: tokio::sync::mpsc::UnboundedReceiver<GatewayClient>,
    task: JoinHandle<()>,
}

impl GatewayFixture {
    async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind gateway fixture");
        let addr = listener.local_addr().expect("fixture addr");
        let registry = Arc::new(tokio::sync::RwLock::new(DeviceRegistry::load()));
        let (client_tx, client_rx) = tokio::sync::mpsc::unbounded_channel();
        let task = tokio::spawn(async move {
            loop {
                let Ok((stream, peer_addr)) = listener.accept().await else {
                    break;
                };
                let registry = Arc::clone(&registry);
                let client_tx = client_tx.clone();
                tokio::spawn(async move {
                    let _ = super::handle_connection(stream, peer_addr, registry, client_tx).await;
                });
            }
        });
        Self {
            addr,
            client_rx,
            task,
        }
    }

    async fn next_runtime_client(&mut self) -> GatewayClient {
        timeout(Duration::from_secs(2), self.client_rx.recv())
            .await
            .expect("runtime client delivered")
            .expect("gateway client")
    }
}

impl Drop for GatewayFixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn post_pair(
    addr: SocketAddr,
    code: &str,
    device_id: &str,
    device_name: &str,
) -> (u16, Value) {
    let body = serde_json::json!({
        "code": code,
        "device_id": device_id,
        "device_name": device_name,
    })
    .to_string();
    let request = format!(
        "POST /pair HTTP/1.1\r\nHost: {addr}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let mut stream = TcpStream::connect(addr).await.expect("connect /pair");
    stream
        .write_all(request.as_bytes())
        .await
        .expect("write /pair");
    let mut response = Vec::new();
    stream.read_to_end(&mut response).await.expect("read /pair");
    let text = String::from_utf8(response).expect("utf8 response");
    let status = text
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse::<u16>().ok())
        .expect("http status");
    let body = text
        .split("\r\n\r\n")
        .nth(1)
        .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
        .expect("json body");
    (status, body)
}

async fn connect_gateway_ws(
    addr: SocketAddr,
    token: &str,
) -> WebSocketStream<MaybeTlsStream<TcpStream>> {
    let url = format!("ws://{addr}/ws?token={token}");
    let (ws, _) = connect_async(url).await.expect("connect websocket");
    ws
}

async fn send_ws(ws: &mut WebSocketStream<MaybeTlsStream<TcpStream>>, payload: Value) {
    ws.send(Message::Text(payload.to_string()))
        .await
        .expect("send websocket payload");
}

async fn next_ws_json(ws: &mut WebSocketStream<MaybeTlsStream<TcpStream>>) -> Value {
    loop {
        let message = timeout(Duration::from_secs(2), ws.next())
            .await
            .expect("websocket message")
            .expect("websocket open")
            .expect("websocket item");
        match message {
            Message::Text(text) => {
                return serde_json::from_str(&text).expect("json websocket frame");
            }
            Message::Ping(data) => ws.send(Message::Pong(data)).await.expect("pong keepalive"),
            Message::Pong(_) | Message::Binary(_) | Message::Frame(_) => {}
            Message::Close(frame) => panic!("websocket closed before text frame: {frame:?}"),
        }
    }
}

async fn next_runtime_json(reader: &mut BufReader<crate::transport::ReadHalf>) -> Value {
    let mut line = String::new();
    timeout(Duration::from_secs(2), reader.read_line(&mut line))
        .await
        .expect("runtime request")
        .expect("read runtime request");
    assert!(
        !line.trim().is_empty(),
        "runtime request line should not be empty"
    );
    serde_json::from_str(line.trim()).expect("json runtime request")
}

async fn write_runtime_event(writer: &mut crate::transport::WriteHalf, payload: Value) {
    writer
        .write_all(payload.to_string().as_bytes())
        .await
        .expect("write runtime event");
    writer.write_all(b"\n").await.expect("write newline");
    writer.flush().await.expect("flush runtime event");
}

#[tokio::test(flavor = "current_thread")]
async fn gateway_e2e_pair_ws_history_send_cancel_reconnect_and_stale_ack_isolation() {
    let _lock = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("temp home");
    std::fs::create_dir_all(temp.path()).expect("jcode home dir");
    let _home = EnvVarGuard::set_path("JCODE_HOME", temp.path());

    let mut registry = DeviceRegistry::default();
    let code = registry.generate_pairing_code();
    assert!(
        DeviceRegistry::load()
            .pending_codes
            .iter()
            .any(|pending| pending.code == code),
        "generated pairing code should be persisted under isolated JCODE_HOME"
    );
    let mut fixture = GatewayFixture::start().await;

    let (status, body) = post_pair(fixture.addr, &code, "surface-y700", "Y700 test").await;
    assert_eq!(
        status,
        200,
        "pair response body={body}; registry={}",
        std::fs::read_to_string(temp.path().join("devices.json")).unwrap_or_default()
    );
    let token = body["token"].as_str().expect("pair token");
    assert_eq!(token.len(), 64);

    let bad_code = if code == "000000" { "111111" } else { "000000" };
    let (bad_status, bad_body) =
        post_pair(fixture.addr, bad_code, "bad-device", "Bad Device").await;
    assert_eq!(bad_status, 401);
    assert_eq!(bad_body["error"], "Invalid or expired pairing code");

    let mut ws = connect_gateway_ws(fixture.addr, token).await;
    let runtime_client = fixture.next_runtime_client().await;
    assert_eq!(runtime_client.device_name, "Y700 test");
    assert_eq!(runtime_client.device_id, "surface-y700");
    let (runtime_read, mut runtime_write) = runtime_client.stream.into_split();
    let mut runtime_reader = BufReader::new(runtime_read);

    send_ws(&mut ws, serde_json::json!({"type":"subscribe","id":1,"target_session_id":"sess-a","client_instance_id":"surface-1","client_has_local_history":true,"allow_session_takeover":true})).await;
    let subscribe = next_runtime_json(&mut runtime_reader).await;
    assert_eq!(subscribe["type"], "subscribe");
    assert_eq!(subscribe["target_session_id"], "sess-a");
    assert_eq!(subscribe["client_has_local_history"], true);

    write_runtime_event(&mut runtime_write, serde_json::json!({"type":"ack","id":1})).await;
    assert_eq!(next_ws_json(&mut ws).await["type"], "ack");

    send_ws(&mut ws, serde_json::json!({"type":"get_history","id":2})).await;
    let get_history = next_runtime_json(&mut runtime_reader).await;
    assert_eq!(get_history["type"], "get_history");
    write_runtime_event(&mut runtime_write, serde_json::json!({"type":"history","id":2,"session_id":"sess-a","messages":[{"role":"assistant","content":"ready"}],"available_models":["haiku"],"all_sessions":["sess-a"]})).await;
    let history = next_ws_json(&mut ws).await;
    assert_eq!(history["type"], "history");
    assert_eq!(history["messages"][0]["content"], "ready");

    send_ws(
        &mut ws,
        serde_json::json!({"type":"message","id":3,"content":"ship the gateway fixture"}),
    )
    .await;
    let message = next_runtime_json(&mut runtime_reader).await;
    assert_eq!(message["type"], "message");
    assert_eq!(message["content"], "ship the gateway fixture");
    write_runtime_event(&mut runtime_write, serde_json::json!({"type":"ack","id":3})).await;
    assert_eq!(next_ws_json(&mut ws).await["id"], 3);

    send_ws(&mut ws, serde_json::json!({"type":"cancel","id":4})).await;
    let cancel = next_runtime_json(&mut runtime_reader).await;
    assert_eq!(cancel["type"], "cancel");
    write_runtime_event(
        &mut runtime_write,
        serde_json::json!({"type":"interrupted","id":4}),
    )
    .await;
    assert_eq!(next_ws_json(&mut ws).await["type"], "interrupted");

    send_ws(
        &mut ws,
        serde_json::json!({"type":"message","id":10,"content":"maybe delivered before disconnect"}),
    )
    .await;
    assert_eq!(next_runtime_json(&mut runtime_reader).await["id"], 10);
    ws.close(None).await.expect("close old websocket");
    // Late events from an old bridge should not leak into the next browser connection.
    let _ = write_runtime_event(
        &mut runtime_write,
        serde_json::json!({"type":"ack","id":10}),
    )
    .await;

    let mut reconnected_ws = connect_gateway_ws(fixture.addr, token).await;
    let reconnected_client = fixture.next_runtime_client().await;
    let (reconnected_read, mut reconnected_write) = reconnected_client.stream.into_split();
    let mut reconnected_reader = BufReader::new(reconnected_read);
    let stale_text = timeout(Duration::from_millis(150), async {
        loop {
            match reconnected_ws.next().await {
                Some(Ok(Message::Text(text))) => break text,
                Some(Ok(Message::Ping(data))) => {
                    reconnected_ws
                        .send(Message::Pong(data))
                        .await
                        .expect("pong keepalive");
                }
                Some(Ok(_)) => {}
                Some(Err(error)) => panic!("websocket error while checking stale ack: {error}"),
                None => panic!("websocket closed while checking stale ack"),
            }
        }
    })
    .await;
    assert!(
        stale_text.is_err(),
        "stale text frame from old bridge must not arrive after reconnect: {stale_text:?}"
    );

    send_ws(&mut reconnected_ws, serde_json::json!({"type":"subscribe","id":11,"target_session_id":"sess-a","client_instance_id":"surface-1","client_has_local_history":true,"allow_session_takeover":true})).await;
    assert_eq!(
        next_runtime_json(&mut reconnected_reader).await["type"],
        "subscribe"
    );
    send_ws(
        &mut reconnected_ws,
        serde_json::json!({"type":"get_history","id":12}),
    )
    .await;
    assert_eq!(
        next_runtime_json(&mut reconnected_reader).await["type"],
        "get_history"
    );
    write_runtime_event(&mut reconnected_write, serde_json::json!({"type":"history","id":12,"session_id":"sess-a","messages":[{"role":"assistant","content":"resynced"}],"available_models":["haiku"],"all_sessions":["sess-a"]})).await;
    assert_eq!(
        next_ws_json(&mut reconnected_ws).await["messages"][0]["content"],
        "resynced"
    );
}
