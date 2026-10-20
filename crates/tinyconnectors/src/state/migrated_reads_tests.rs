//! Owner migration tests through the same bound-storage preparation path used
//! when the `TinyBus` module first connects to host storage.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;

use serde_json::{Value, json};
use tinyconnectors_bus::storage::{LEGACY_PREFS_NAMESPACE, PREFS_NAMESPACE, STREAM_PREFIX};
use tinyconnectors_bus::vocabulary::UserScopePref;
use tinyconnectors_sync::{PrefsStore, ScopePrefStore};
use tinystoragedrivers_api::{Storage, StorageBackend};
use tinystoragedrivers_testkit::MemoryBackend;
use tinystoragedrivers_types::{Precondition, Scope};

use crate::state::{DocumentPrefsStore, migration};
use crate::triggers::TriggerArchive;

fn module_storage(backend: &MemoryBackend) -> Storage {
    backend
        .for_scope(&Scope::new("connectors").unwrap())
        .unwrap()
}

async fn migrated(storage: &Storage) {
    migration::prepare(storage).await.unwrap();
}

async fn seed_v1(storage: &Storage, id: &str, body: Value) {
    storage
        .documents()
        .put(LEGACY_PREFS_NAMESPACE, id, body, Precondition::None)
        .await
        .unwrap();
}

#[tokio::test]
async fn a_migrated_preference_reads_through_the_store_with_its_choice_intact() {
    let backend = MemoryBackend::new();
    let storage = module_storage(&backend);
    seed_v1(
        &storage,
        "slack",
        json!({"value": {"read": true, "write": false, "admin": false}}),
    )
    .await;
    migrated(&storage).await;

    let store = DocumentPrefsStore::new(Arc::clone(storage.documents()));
    let pref = UserScopePref::load(&store, "slack").await.unwrap();
    assert!(pref.read);
    assert!(!pref.write);
}

#[tokio::test]
async fn non_object_preferences_move_under_the_reserved_field_and_read_back() {
    let backend = MemoryBackend::new();
    let storage = module_storage(&backend);
    seed_v1(&storage, "slack", json!({"value": [1, 2]})).await;
    seed_v1(&storage, "notion", json!({"value": "x"})).await;
    migrated(&storage).await;

    let found = storage
        .documents()
        .get(PREFS_NAMESPACE, "slack")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(found.doc, json!({"non_object_value": [1, 2]}));
    let store = DocumentPrefsStore::new(Arc::clone(storage.documents()));
    assert_eq!(store.get("slack").await.unwrap().unwrap(), json!([1, 2]));
    assert_eq!(store.get("notion").await.unwrap().unwrap(), json!("x"));
}

#[tokio::test]
async fn a_reserved_field_inside_an_object_remains_an_object() {
    let backend = MemoryBackend::new();
    let storage = module_storage(&backend);
    let tricky = json!({"non_object_value": 7});
    seed_v1(&storage, "gmail", json!({"value": tricky})).await;
    migrated(&storage).await;
    let store = DocumentPrefsStore::new(Arc::clone(storage.documents()));
    assert_eq!(store.get("gmail").await.unwrap().unwrap(), tricky);
}

#[tokio::test]
async fn missing_legacy_value_does_not_turn_into_default_permissions() {
    let backend = MemoryBackend::new();
    let storage = module_storage(&backend);
    seed_v1(&storage, "gmail", json!({"other": 1})).await;
    migrated(&storage).await;
    let store = DocumentPrefsStore::new(Arc::clone(storage.documents()));
    assert!(UserScopePref::load(&store, "gmail").await.is_err());
}

#[tokio::test]
async fn migration_can_be_retried_after_the_target_was_written() {
    let backend = MemoryBackend::new();
    let storage = module_storage(&backend);
    seed_v1(&storage, "gmail", json!({"value": {"read": true}})).await;
    migrated(&storage).await;
    migrated(&storage).await;
    let found = storage
        .documents()
        .get(PREFS_NAMESPACE, "gmail")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(found.doc, json!({"read": true}));
}

const LEGACY_STREAM: &str = "triggers/2026-10-15";

#[tokio::test]
async fn migrated_trigger_history_reads_back_through_the_archive() {
    let backend = MemoryBackend::new();
    let storage = module_storage(&backend);
    let entry = |id: &str| {
        json!({
            "received_at_ms": 1_760_000_000_000_u64,
            "toolkit": "gmail",
            "trigger": "GMAIL_NEW_GMAIL_MESSAGE",
            "metadata_id": id,
            "metadata_uuid": "u",
            "payload": {"subject": "hi"},
        })
    };
    for id in ["evt-1", "evt-2"] {
        storage
            .streams()
            .append(LEGACY_STREAM, entry(id))
            .await
            .unwrap();
    }
    migrated(&storage).await;
    let history = TriggerArchive::new(Arc::clone(storage.streams()))
        .list_recent(None)
        .await
        .unwrap();
    let ids: Vec<&str> = history
        .entries
        .iter()
        .map(|entry| entry.metadata_id.as_str())
        .collect();
    assert_eq!(ids, ["evt-2", "evt-1"]);
    assert_eq!(
        storage.streams().streams(STREAM_PREFIX).await.unwrap(),
        ["connectors.triggers/2026-10-15"]
    );
}
