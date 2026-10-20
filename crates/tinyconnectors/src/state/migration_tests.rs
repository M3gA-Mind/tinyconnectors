#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::*;

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
