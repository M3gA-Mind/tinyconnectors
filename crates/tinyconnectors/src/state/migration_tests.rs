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
async fn preference_migration_resumes_matching_targets_across_pages() {
    use tinystoragedrivers_api::StorageBackend;
    use tinystoragedrivers_testkit::MemoryBackend;
    use tinystoragedrivers_types::{Precondition, Scope};

    let storage = MemoryBackend::new()
        .for_scope(&Scope::new("connector-preference-pagination").unwrap())
        .unwrap();
    for index in 0..(PAGE_SIZE + 3) {
        let id = format!("preference-{index:03}");
        let value = json!({"enabled": index % 2 == 0, "index": index});
        storage
            .documents()
            .put(
                LEGACY_PREFS_NAMESPACE,
                &id,
                json!({"value": value}),
                Precondition::Absent,
            )
            .await
            .unwrap();
    }

    // A previous interrupted opener may have written this exact destination
    // before advancing the query cursor. It is safe to accept and continue.
    let first = encode_preference(&json!({"enabled": true, "index": 0}));
    storage
        .documents()
        .put(
            PREFS_NAMESPACE,
            "preference-000",
            first,
            Precondition::Absent,
        )
        .await
        .unwrap();

    prepare(&storage).await.unwrap();

    for index in 0..(PAGE_SIZE + 3) {
        let id = format!("preference-{index:03}");
        let expected = encode_preference(&json!({"enabled": index % 2 == 0, "index": index}));
        assert_eq!(
            storage
                .documents()
                .get(PREFS_NAMESPACE, &id)
                .await
                .unwrap()
                .unwrap()
                .doc,
            expected
        );
    }
    assert!(
        done(storage.documents().as_ref(), PREFS_STEP)
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn preference_migration_preserves_expiration_and_rejects_expiry_mismatch() {
    use tinystoragedrivers_api::StorageBackend;
    use tinystoragedrivers_testkit::MemoryBackend;
    use tinystoragedrivers_types::{Precondition, Scope, WriteOptions};

    let expires_ms = 4_000_000_000_000;
    let value = json!({"enabled": true});
    let body = encode_preference(&value);

    let collision = MemoryBackend::new()
        .for_scope(&Scope::new("connector-preference-expiry-conflict").unwrap())
        .unwrap();
    collision
        .documents()
        .put_with(
            LEGACY_PREFS_NAMESPACE,
            "gmail",
            json!({"value": value}),
            Precondition::Absent,
            WriteOptions::new().expires_ms(expires_ms),
        )
        .await
        .unwrap();
    // The payload matches, but accepting this destination would silently
    // remove the source's absolute expiration.
    collision
        .documents()
        .put(PREFS_NAMESPACE, "gmail", body.clone(), Precondition::Absent)
        .await
        .unwrap();
    let error = prepare(&collision)
        .await
        .expect_err("matching payload with different expiry is a collision");
    assert_eq!(error.kind(), tinystoragedrivers_types::ErrorKind::Conflict);
    assert_eq!(
        collision
            .documents()
            .get(LEGACY_PREFS_NAMESPACE, "gmail")
            .await
            .unwrap()
            .unwrap()
            .meta
            .expires_ms,
        Some(expires_ms)
    );

    let migrated = MemoryBackend::new()
        .for_scope(&Scope::new("connector-preference-expiry-copy").unwrap())
        .unwrap();
    migrated
        .documents()
        .put_with(
            LEGACY_PREFS_NAMESPACE,
            "gmail",
            json!({"value": value}),
            Precondition::Absent,
            WriteOptions::new().expires_ms(expires_ms),
        )
        .await
        .unwrap();
    prepare(&migrated).await.unwrap();
    let target = migrated
        .documents()
        .get(PREFS_NAMESPACE, "gmail")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(target.doc, body);
    assert_eq!(target.meta.expires_ms, Some(expires_ms));
}

#[tokio::test]
async fn legacy_preferences_without_a_value_migrate_as_null() {
    use tinystoragedrivers_api::StorageBackend;
    use tinystoragedrivers_testkit::MemoryBackend;
    use tinystoragedrivers_types::{Precondition, Scope};

    let storage = MemoryBackend::new()
        .for_scope(&Scope::new("connector-preference-malformed").unwrap())
        .unwrap();
    storage
        .documents()
        .put(
            LEGACY_PREFS_NAMESPACE,
            "missing-value",
            json!({"other": true}),
            Precondition::Absent,
        )
        .await
        .unwrap();
    prepare(&storage).await.unwrap();

    assert_eq!(
        storage
            .documents()
            .get(PREFS_NAMESPACE, "missing-value")
            .await
            .unwrap()
            .unwrap()
            .doc,
        encode_preference(&Value::Null),
        "legacy entries without a `value` retain the old null fallback"
    );
}

#[tokio::test]
async fn preference_query_failure_is_returned_without_marking_the_step_done() {
    use std::sync::Arc;
    use tinystoragedrivers_api::{DocumentStore, Storage, StorageBackend};
    use tinystoragedrivers_testkit::{Failing, FaultPlan, MemoryBackend};
    use tinystoragedrivers_types::{ErrorKind, Scope};

    let backend = MemoryBackend::new();
    let base = backend
        .for_scope(&Scope::new("connector-preference-query-failure").unwrap())
        .unwrap();
    let faults = FaultPlan::new();
    faults.fail_operation_nth("documents.query", 1, ErrorKind::Backend);
    let documents: Arc<dyn DocumentStore> =
        Arc::new(Failing::with_plan(base.documents().clone(), faults));
    let storage = Storage::new(
        documents,
        base.streams().clone(),
        base.kv().clone(),
        base.blobs().clone(),
        base.queues().clone(),
        base.secrets().clone(),
    );

    let error = prepare(&storage)
        .await
        .expect_err("preference query failures are surfaced to the module opener");
    assert_eq!(error.kind(), ErrorKind::Backend);
    assert!(
        base.documents()
            .get(MIGRATIONS, PREFS_STEP)
            .await
            .unwrap()
            .is_none(),
        "failed reads must not persist a completed migration marker"
    );
}

#[tokio::test]
async fn failed_completion_marker_keeps_migration_retryable() {
    use std::sync::Arc;
    use tinystoragedrivers_api::{DocumentStore, Storage, StorageBackend};
    use tinystoragedrivers_testkit::{Failing, FaultPlan, MemoryBackend};
    use tinystoragedrivers_types::{ErrorKind, Precondition, Scope};

    let backend = MemoryBackend::new();
    let base = backend
        .for_scope(&Scope::new("connector-marker-retry").unwrap())
        .unwrap();
    base.documents()
        .put(
            LEGACY_PREFS_NAMESPACE,
            "gmail",
            json!({"value": {"read": true}}),
            Precondition::Absent,
        )
        .await
        .unwrap();

    let faults = FaultPlan::new();
    faults.fail_operation_nth("documents.put", 1, ErrorKind::Backend);
    let documents: Arc<dyn DocumentStore> =
        Arc::new(Failing::with_plan(base.documents().clone(), faults.clone()));
    let storage = Storage::new(
        documents,
        base.streams().clone(),
        base.kv().clone(),
        base.blobs().clone(),
        base.queues().clone(),
        base.secrets().clone(),
    );

    let error = prepare(&storage)
        .await
        .expect_err("an interrupted completion marker is reported to the opener");
    assert_eq!(error.kind(), ErrorKind::Backend);
    assert!(
        base.documents()
            .get(PREFS_NAMESPACE, "gmail")
            .await
            .unwrap()
            .is_some(),
        "the transformed row is left available for an idempotent retry"
    );

    faults.clear();
    prepare(&storage).await.unwrap();
    prepare(&storage).await.unwrap();
    assert!(
        done(storage.documents().as_ref(), PREFS_STEP)
            .await
            .unwrap()
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
async fn stream_migration_resumes_a_matching_partial_target() {
    use tinystoragedrivers_api::StorageBackend;
    use tinystoragedrivers_testkit::MemoryBackend;
    use tinystoragedrivers_types::Scope;

    let storage = MemoryBackend::new()
        .for_scope(&Scope::new("connector-stream-resume").unwrap())
        .unwrap();
    let source = "triggers/2026-10-17";
    let target = "connectors.triggers/2026-10-17";
    for index in 0..3 {
        storage
            .streams()
            .append(source, json!({"event": index}))
            .await
            .unwrap();
    }
    for index in 0..2 {
        storage
            .streams()
            .append(target, json!({"event": index}))
            .await
            .unwrap();
    }

    prepare(&storage).await.unwrap();

    assert_eq!(storage.streams().len(source).await.unwrap(), 0);
    assert_eq!(storage.streams().len(target).await.unwrap(), 3);
    assert_eq!(
        storage
            .streams()
            .read_window(target, 0, 3)
            .await
            .unwrap()
            .into_iter()
            .map(|entry| entry.value)
            .collect::<Vec<_>>(),
        vec![
            json!({"event": 0}),
            json!({"event": 1}),
            json!({"event": 2})
        ]
    );
}

#[tokio::test]
async fn stream_write_failure_preserves_source_for_retry() {
    use std::sync::Arc;
    use tinystoragedrivers_api::{Storage, StorageBackend, StreamStore};
    use tinystoragedrivers_testkit::{Failing, FaultPlan, MemoryBackend};
    use tinystoragedrivers_types::{ErrorKind, Scope};

    let base = MemoryBackend::new()
        .for_scope(&Scope::new("connector-stream-retry").unwrap())
        .unwrap();
    let source = "triggers/2026-10-18";
    let target = "connectors.triggers/2026-10-18";
    base.streams()
        .append(source, json!({"event": "retained"}))
        .await
        .unwrap();

    let faults = FaultPlan::new();
    faults.fail_operation_nth("streams.append_batch_if_offset", 1, ErrorKind::Backend);
    let streams: Arc<dyn StreamStore> =
        Arc::new(Failing::with_plan(base.streams().clone(), faults.clone()));
    let storage = Storage::new(
        base.documents().clone(),
        streams,
        base.kv().clone(),
        base.blobs().clone(),
        base.queues().clone(),
        base.secrets().clone(),
    );

    let error = prepare(&storage)
        .await
        .expect_err("failed target append stops migration before source deletion");
    assert_eq!(error.kind(), ErrorKind::Backend);
    assert_eq!(base.streams().len(source).await.unwrap(), 1);
    assert_eq!(base.streams().len(target).await.unwrap(), 0);

    faults.clear();
    prepare(&storage).await.unwrap();
    assert_eq!(base.streams().len(source).await.unwrap(), 0);
    assert_eq!(base.streams().len(target).await.unwrap(), 1);
}

#[tokio::test]
async fn failed_stream_delete_keeps_both_copies_for_retry() {
    use std::sync::Arc;
    use tinystoragedrivers_api::{Storage, StorageBackend, StreamStore};
    use tinystoragedrivers_testkit::{Failing, FaultPlan, MemoryBackend};
    use tinystoragedrivers_types::{ErrorKind, Scope};

    let base = MemoryBackend::new()
        .for_scope(&Scope::new("connector-stream-delete-failure").unwrap())
        .unwrap();
    let source = "triggers/2026-10-19";
    let target = "connectors.triggers/2026-10-19";
    base.streams()
        .append(source, json!({"event": "retained"}))
        .await
        .unwrap();

    let faults = FaultPlan::new();
    faults.fail_operation_nth("streams.delete_stream", 1, ErrorKind::Backend);
    let streams: Arc<dyn StreamStore> =
        Arc::new(Failing::with_plan(base.streams().clone(), faults.clone()));
    let storage = Storage::new(
        base.documents().clone(),
        streams,
        base.kv().clone(),
        base.blobs().clone(),
        base.queues().clone(),
        base.secrets().clone(),
    );

    let error = prepare(&storage)
        .await
        .expect_err("source deletion failure is returned after copying the target");
    assert_eq!(error.kind(), ErrorKind::Backend);
    assert_eq!(base.streams().len(source).await.unwrap(), 1);
    assert_eq!(base.streams().len(target).await.unwrap(), 1);

    faults.clear();
    prepare(&storage).await.unwrap();
    assert_eq!(base.streams().len(source).await.unwrap(), 0);
    assert_eq!(base.streams().len(target).await.unwrap(), 1);
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
