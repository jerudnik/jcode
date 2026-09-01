use std::collections::{BTreeMap, BTreeSet};

use jcode_storage::{
    ARTIFACTS, ArtifactId, ArtifactKey, RuntimePaths, SessionInboxId, TemporaryKind,
    canonical_tier_for_path, durable_path, tag, temporary_path,
};

#[test]
fn registry_is_complete_unique_and_ordered_like_the_closed_enum() {
    assert_eq!(ARTIFACTS.len(), ArtifactId::ALL.len());

    let enum_ids: BTreeSet<_> = ArtifactId::ALL.iter().copied().collect();
    let registry_ids: BTreeSet<_> = ARTIFACTS.iter().map(|spec| spec.id).collect();
    assert_eq!(registry_ids, enum_ids);
    assert_eq!(registry_ids.len(), ARTIFACTS.len());

    for (index, spec) in ARTIFACTS.iter().enumerate() {
        assert_eq!(
            spec.id as usize, index,
            "{} is out of registry order",
            spec.name
        );
    }
}

#[test]
fn permanent_pin_set_is_exactly_the_four_declared_exceptions() {
    let actual: BTreeMap<_, _> = ARTIFACTS
        .iter()
        .filter_map(|spec| spec.pinned.map(|pin| (spec.id, pin.reason)))
        .collect();
    let expected = BTreeSet::from([
        ArtifactId::ConfigToml,
        ArtifactId::SessionLock,
        ArtifactId::DaemonSocket,
        ArtifactId::LegacySessionRoot,
    ]);

    assert_eq!(actual.len(), 4);
    assert_eq!(actual.keys().copied().collect::<BTreeSet<_>>(), expected);
    assert!(actual.values().all(|reason| !reason.trim().is_empty()));
    let config = ARTIFACTS
        .iter()
        .find(|spec| spec.id == ArtifactId::ConfigToml)
        .expect("config spec");
    assert_eq!(config.legacy_companions, &["config.toml.hm-backup"]);
    let config_path = durable_path::<tag::ConfigToml>(()).expect("resolve config path");
    let jcode_home = config_path
        .as_path()
        .parent()
        .expect("config path has a parent");

    let session_lock = temporary_path::<tag::SessionLock>("session-123".to_owned())
        .expect("resolve pinned session lock");
    assert_eq!(
        session_lock.as_path(),
        jcode_home.join("active_pids/session-123")
    );

    let legacy_sessions =
        durable_path::<tag::LegacySessionRoot>(()).expect("resolve pinned legacy session root");
    assert_eq!(legacy_sessions.as_path(), jcode_home.join("sessions"));
}

#[test]
fn every_existing_artifact_keeps_a_legacy_location_and_w2_is_born_canonical() {
    for spec in ARTIFACTS {
        if spec.id == ArtifactId::SessionInboxItem {
            assert!(spec.legacy.is_none());
        } else {
            assert!(
                spec.legacy.is_some(),
                "pre-existing artifact {} lost its legacy location",
                spec.name
            );
        }
    }

    let path = durable_path::<tag::SessionInboxItem>(
        SessionInboxId::new("session-123").expect("valid inbox ID"),
    )
    .expect("resolve W2 session inbox path");
    let config_path = durable_path::<tag::ConfigToml>(()).expect("resolve config path");
    let jcode_home = config_path
        .as_path()
        .parent()
        .expect("config path has a parent");
    assert_eq!(path.as_path(), jcode_home.join("session-123"));
}

#[test]
fn w2_session_inbox_id_is_one_direct_child_component() {
    for invalid in ["", ".", "..", "nested/session", "../session", "/session"] {
        assert!(
            SessionInboxId::new(invalid).is_err(),
            "accepted invalid W2 ID {invalid:?}"
        );
    }
    assert!(SessionInboxId::new("session-123").is_ok());
}

#[test]
fn every_canonical_location_recovers_its_declared_tier() {
    let paths = RuntimePaths::current();
    let mut resolved = BTreeMap::new();
    for spec in ARTIFACTS {
        assert_eq!(spec.canonical_root.tier(), spec.tier, "{} root", spec.name);
        let dynamic_key = spec.example_key.to_owned();
        let key: &dyn ArtifactKey = if dynamic_key.is_empty() {
            &()
        } else {
            &dynamic_key
        };
        let canonical = spec
            .canonical_path(&paths, key)
            .unwrap_or_else(|error| panic!("resolve {}: {error}", spec.name));
        assert_eq!(
            canonical_tier_for_path(&paths, &canonical).expect("classify canonical path"),
            Some(spec.tier),
            "{} at {}",
            spec.name,
            canonical.display()
        );
        if let Some(previous) = resolved.insert(canonical.clone(), spec.name) {
            panic!(
                "canonical path collision between {previous} and {} at {}",
                spec.name,
                canonical.display()
            );
        }
    }
}

#[test]
fn temporary_constructor_preserves_the_declared_sub_kind() {
    let path = temporary_path::<tag::ConfigLock>(()).expect("resolve config lock");
    assert_eq!(path.kind(), TemporaryKind::Lock);
}
