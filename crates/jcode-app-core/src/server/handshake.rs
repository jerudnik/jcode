//! NS1 server-side compatibility handshake.
//!
//! When a client advertises its build/protocol identity on `Subscribe`
//! (`protocol_version` + `build_hash`), the server compares it against its own
//! compiled identity and returns a typed [`HandshakeCompatibility`] verdict so
//! the client can decide whether to attach or re-exec into the matching
//! launcher. Legacy clients (no advertised identity) are never sent a verdict
//! event, so the seam is fully additive. See
//! `docs/architecture/SELFDEV_NIX_DAEMON_DIVERGENCE.md` (NS1, gaps G1/G3).

use jcode_protocol::{HandshakeCompatibility, PROTOCOL_VERSION, ServerEvent};
use jcode_selfdev_types::RuntimeIdentityProjection;
use tokio::sync::mpsc;

pub(super) struct SubscribeHandshakeEvaluation {
    pub compatibility: HandshakeCompatibility,
    pub verdict_event: Option<ServerEvent>,
    detail: String,
    server_hash: String,
    server_runtime_identity: RuntimeIdentityProjection,
}

pub(super) fn evaluate_subscribe_handshake(
    id: u64,
    client_protocol_version: Option<u32>,
    client_build_hash: Option<&str>,
) -> SubscribeHandshakeEvaluation {
    let server_hash = server_build_hash();
    let server_runtime_identity = server_runtime_identity_projection();
    let (compatibility, detail) = HandshakeCompatibility::evaluate(
        client_protocol_version,
        client_build_hash,
        PROTOCOL_VERSION,
        Some(server_hash),
    );
    let verdict_event = client_protocol_version.map(|_| ServerEvent::HandshakeVerdict {
        id,
        compatibility,
        server_protocol_version: PROTOCOL_VERSION,
        server_build_hash: Some(server_hash.to_string()),
        server_runtime_identity: Some(server_runtime_identity.clone()),
        detail: detail.clone(),
    });
    SubscribeHandshakeEvaluation {
        compatibility,
        verdict_event,
        detail,
        server_hash: server_hash.to_string(),
        server_runtime_identity,
    }
}

pub(super) fn log_subscribe_handshake_evaluation(
    id: u64,
    client_protocol_version: Option<u32>,
    client_build_hash: Option<&str>,
    client_runtime_identity: Option<&RuntimeIdentityProjection>,
    evaluation: &SubscribeHandshakeEvaluation,
) {
    crate::logging::event_info(
        "HANDSHAKE_VERDICT",
        vec![
            ("request_id", id.to_string()),
            (
                "client_protocol",
                client_protocol_version
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "none".to_string()),
            ),
            (
                "client_hash",
                client_build_hash.unwrap_or("none").to_string(),
            ),
            ("server_protocol", PROTOCOL_VERSION.to_string()),
            ("server_hash", evaluation.server_hash.clone()),
            (
                "compatibility",
                format!("{:?}", evaluation.compatibility),
            ),
            ("detail", evaluation.detail.clone()),
            (
                "client_runtime_identity",
                client_runtime_identity
                    .map(|identity| identity.version_label.clone())
                    .unwrap_or_else(|| "none".to_string()),
            ),
            (
                "server_runtime_identity",
                evaluation.server_runtime_identity.version_label.clone(),
            ),
        ],
    );
}

/// Evaluate the client's advertised handshake identity against this server's
/// own and, when the client advertised a protocol version, send the verdict
/// event on `client_event_tx`. Returns the verdict (for logging/tests).
///
/// The pure comparison lives in [`HandshakeCompatibility::evaluate`]; this is
/// the thin daemon-side shell that supplies the server's own identity and
/// emits the wire event. The verdict event is intentionally *not* sent to
/// legacy clients (`client_protocol_version == None`) so they keep parsing the
/// stream they already understand.
pub(super) fn evaluate_and_notify(
    id: u64,
    client_protocol_version: Option<u32>,
    client_build_hash: Option<&str>,
    client_runtime_identity: Option<&RuntimeIdentityProjection>,
    client_event_tx: &mpsc::UnboundedSender<ServerEvent>,
) -> HandshakeCompatibility {
    let evaluation = evaluate_subscribe_handshake(
        id,
        client_protocol_version,
        client_build_hash,
    );
    log_subscribe_handshake_evaluation(
        id,
        client_protocol_version,
        client_build_hash,
        client_runtime_identity,
        &evaluation,
    );

    // Only clients that advertised identity understand the verdict event.
    if let Some(event) = evaluation.verdict_event.clone() {
        let _ = client_event_tx.send(event);
    }

    evaluation.compatibility
}

/// The server's own short git hash, as stamped into this binary at build time.
fn server_build_hash() -> &'static str {
    jcode_build_meta::GIT_HASH
}

fn server_runtime_identity_projection() -> RuntimeIdentityProjection {
    crate::build::current_runtime_identity_projection("shared-server")
}

#[cfg(test)]
mod tests {
    use super::*;
    use jcode_protocol::ServerEvent;

    #[test]
    fn legacy_client_gets_no_verdict_event() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let verdict = evaluate_and_notify(7, None, None, None, &tx);
        assert_eq!(verdict, HandshakeCompatibility::Compatible);
        // No event is sent to a legacy client.
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn advertised_client_gets_a_verdict_event() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        // The server hash is whatever this test binary was stamped with; a
        // deliberately-mismatched client hash forces IncompatibleReconnect so
        // the assertion does not depend on the ambient build hash.
        let verdict = evaluate_and_notify(
            9,
            Some(PROTOCOL_VERSION),
            Some("0000000-not-a-real-hash"),
            None,
            &tx,
        );
        assert_eq!(verdict, HandshakeCompatibility::IncompatibleReconnect);
        match rx.try_recv() {
            Ok(ServerEvent::HandshakeVerdict {
                id,
                compatibility,
                server_protocol_version,
                server_runtime_identity,
                ..
            }) => {
                assert_eq!(id, 9);
                assert_eq!(compatibility, HandshakeCompatibility::IncompatibleReconnect);
                assert_eq!(server_protocol_version, PROTOCOL_VERSION);
                assert!(server_runtime_identity.is_some());
            }
            other => panic!("expected HandshakeVerdict, got {other:?}"),
        }
    }

    #[test]
    fn matching_protocol_and_hash_is_compatible_with_event() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        // Advertise the server's own hash -> compatible, but still get an event.
        let verdict = evaluate_and_notify(
            3,
            Some(PROTOCOL_VERSION),
            Some(server_build_hash()),
            None,
            &tx,
        );
        assert_eq!(verdict, HandshakeCompatibility::Compatible);
        assert!(matches!(
            rx.try_recv(),
            Ok(ServerEvent::HandshakeVerdict {
                compatibility: HandshakeCompatibility::Compatible,
                ..
            })
        ));
    }
}
