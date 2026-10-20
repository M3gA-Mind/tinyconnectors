#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::*;

fn storage() -> tinystoragedrivers_api::Storage {
    use tinystoragedrivers_api::StorageBackend;
    use tinystoragedrivers_testkit::MemoryBackend;
    use tinystoragedrivers_types::Scope;

    MemoryBackend::new()
        .for_scope(&Scope::new("connector-migration-tests").unwrap())
        .unwrap()
}

#[test]
fn legacy_composite_key_bytes_keep_the_pre_split_encoding() {
    assert_eq!(legacy_key(["a/b"]).unwrap().as_str(), "a%2Fb");
    assert_eq!(legacy_key(["%00"]).unwrap().as_str(), "%2500");
    assert_eq!(legacy_key(["~reserved"]).unwrap().as_str(), "%7Ereserved");
    assert_eq!(legacy_key(["雪/猫"]).unwrap().as_str(), "雪%2F猫");
    assert_eq!(
        legacy_key(["owner", "part"]).unwrap().as_str(),
        "owner/part"
    );
}

#[test]
fn legacy_key_rejects_empty_input_and_hashes_values_over_the_storage_limit() {
    assert!(legacy_key(Vec::<String>::new()).is_err());

    let key = legacy_key(["x".repeat(600)]).unwrap();
    assert!(key.as_str().starts_with('~'));
    assert!(key.as_str().len() <= 512);
}

#[tokio::test]
async fn migration_refuses_an_invalid_owner_marker() {
    use tinystoragedrivers_types::Precondition;

    let storage = storage();
    storage
        .documents()
        .put(
            MIGRATIONS,
            PREFS_STEP,
            json!({"owner": "another-owner", "step": PREFS_STEP, "done": true}),
            Precondition::Absent,
        )
        .await
        .unwrap();

    let error = prepare(&storage)
        .await
        .expect_err("foreign marker is refused");
    assert_eq!(error.kind(), tinystoragedrivers_types::ErrorKind::Conflict);
}

#[tokio::test]
async fn preference_collision_preserves_the_legacy_source_and_refuses_open() {
    use tinystoragedrivers_api::StorageBackend;
    use tinystoragedrivers_testkit::MemoryBackend;
    use tinystoragedrivers_types::{Precondition, Scope};

    let backend = MemoryBackend::new();
    let storage = backend
        .for_scope(&Scope::new("connector-preference-collision").unwrap())
        .unwrap();
    storage
        .documents()
        .put(
            LEGACY_PREFS_NAMESPACE,
            "gmail",
            json!({"value": {"read": true}}),
            Precondition::Absent,
        )
        .await
        .unwrap();
    storage
        .documents()
        .put(
            PREFS_NAMESPACE,
            "gmail",
            json!({"read": false}),
            Precondition::Absent,
        )
        .await
        .unwrap();

    let error = prepare(&storage)
        .await
        .expect_err("different target content is not overwritten");
    assert_eq!(error.kind(), tinystoragedrivers_types::ErrorKind::Conflict);
    assert_eq!(
        storage
            .documents()
            .get(LEGACY_PREFS_NAMESPACE, "gmail")
            .await
            .unwrap()
            .unwrap()
            .doc,
        json!({"value": {"read": true}})
    );
}

#[tokio::test]
async fn stream_migration_refuses_a_target_longer_than_its_source() {
    use tinystoragedrivers_api::StorageBackend;
    use tinystoragedrivers_testkit::MemoryBackend;
    use tinystoragedrivers_types::Scope;

    let backend = MemoryBackend::new();
    let storage = backend
        .for_scope(&Scope::new("connector-stream-collision").unwrap())
        .unwrap();
    storage
        .streams()
        .append("triggers/2026-10-15", json!({"event": 1}))
        .await
        .unwrap();
    storage
        .streams()
        .append("connectors.triggers/2026-10-15", json!({"event": 1}))
        .await
        .unwrap();
    storage
        .streams()
        .append("connectors.triggers/2026-10-15", json!({"event": 2}))
        .await
        .unwrap();

    let error = prepare(&storage)
        .await
        .expect_err("a longer target cannot be reconciled safely");
    assert_eq!(error.kind(), tinystoragedrivers_types::ErrorKind::Conflict);
    assert_eq!(
        storage.streams().len("triggers/2026-10-15").await.unwrap(),
        1,
        "source events survive a refused migration"
    );
}

#[tokio::test]
async fn stream_migration_refuses_same_length_target_with_different_events() {
    use tinystoragedrivers_api::StorageBackend;
    use tinystoragedrivers_testkit::MemoryBackend;
    use tinystoragedrivers_types::Scope;

    let backend = MemoryBackend::new();
    let storage = backend
        .for_scope(&Scope::new("connector-stream-content-collision").unwrap())
        .unwrap();
    storage
        .streams()
        .append("triggers/2026-10-16", json!({"event": "source"}))
        .await
        .unwrap();
    storage
        .streams()
        .append(
            "connectors.triggers/2026-10-16",
            json!({"event": "different"}),
        )
        .await
        .unwrap();

    let error = prepare(&storage)
        .await
        .expect_err("same-length but different target data must not be accepted");
    assert_eq!(error.kind(), tinystoragedrivers_types::ErrorKind::Conflict);
    assert_eq!(
        storage.streams().len("triggers/2026-10-16").await.unwrap(),
        1,
        "the source is preserved when target verification fails"
    );
}

#[tokio::test]
async fn concurrent_opens_resume_owner_migrations_without_duplicate_stream_entries() {
    use tinystoragedrivers_api::StorageBackend;
    use tinystoragedrivers_testkit::MemoryBackend;
    use tinystoragedrivers_types::{Precondition, Scope};

    let backend = MemoryBackend::new();
    let storage = backend
        .for_scope(&Scope::new("connectors").unwrap())
        .unwrap();
    storage
        .documents()
        .put(
            LEGACY_PREFS_NAMESPACE,
            "gmail",
            json!({"value": {"read": true}}),
            Precondition::None,
        )
        .await
        .unwrap();
    storage
        .streams()
        .append("triggers/2026-10-15", json!({"event": 1}))
        .await
        .unwrap();

    let (left, right) = tokio::join!(prepare(&storage), prepare(&storage));
    left.unwrap();
    right.unwrap();
    assert_eq!(
        storage
            .streams()
            .len("connectors.triggers/2026-10-15")
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        storage
            .documents()
            .get(PREFS_NAMESPACE, "gmail")
            .await
            .unwrap()
            .unwrap()
            .doc,
        json!({"read": true})
    );
}
