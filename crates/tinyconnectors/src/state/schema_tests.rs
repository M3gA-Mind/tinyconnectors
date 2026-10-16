//! Unit tests for the module's storage schema and its v1 to v2 steps.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;

use serde_json::{Value, json};
use tinyconnectors_bus::vocabulary::UserScopePref;
use tinyconnectors_sync::{LEGACY_PREFS_NAMESPACE, PREFS_NAMESPACE, PrefsStore, ScopePrefStore};
use tinystoragedrivers_core::schema::{Item, MigrateOptions, Registry, ScopeClass};
use tinystoragedrivers_core::{
    DocumentStore, MemoryStorage, Precondition, Scope, ScopedStorage, StorageBackend, StreamStore,
};

use super::{DocumentPrefsStore, storage_needs, storage_schema};
use crate::triggers::TriggerArchive;

const LEGACY_STREAM: &str = "triggers/2026-10-15";
const STREAM: &str = "connectors.triggers/2026-10-15";

fn module_storage(backend: &MemoryStorage) -> ScopedStorage {
    backend
        .for_scope(&Scope::local().module("connectors").unwrap())
        .unwrap()
}

async fn migrated(storage: &ScopedStorage, backend: &MemoryStorage) {
    let registry = Registry::compose([storage_schema()]).unwrap();
    registry.prepare(backend).await.unwrap();
    registry
        .migrate(storage, &MigrateOptions::new())
        .await
        .unwrap();
}

async fn seed_v1(storage: &ScopedStorage, id: &str, body: Value) {
    storage
        .documents()
        .put(LEGACY_PREFS_NAMESPACE, id, body, Precondition::None)
        .await
        .unwrap();
}

#[test]
fn the_schema_composes_and_everything_in_it_is_module_scoped() {
    let schema = storage_schema();
    assert_eq!(schema.owner.as_str(), "connectors");
    assert!(Registry::compose([schema.clone()]).is_ok());
    assert!(
        schema
            .items
            .iter()
            .all(|item| item.scope() == ScopeClass::Module)
    );
    let names: Vec<&str> = schema.items.iter().map(Item::name).collect();
    assert_eq!(names, ["connectors.user_scopes", "connectors.triggers"]);
}

#[test]
fn the_grant_is_generated_from_the_schema() {
    let needs = storage_needs();
    assert_eq!(needs.collections, [PREFS_NAMESPACE]);
    assert_eq!(needs.stream_prefixes, ["connectors.triggers/"]);
}

#[tokio::test]
async fn an_object_preference_loses_its_value_wrapper() {
    let backend = MemoryStorage::new();
    let storage = module_storage(&backend);
    seed_v1(
        &storage,
        "gmail",
        json!({ "value": { "read": true, "write": false, "admin": false } }),
    )
    .await;

    migrated(&storage, &backend).await;

    let found = storage
        .documents()
        .get(PREFS_NAMESPACE, "gmail")
        .await
        .unwrap()
        .expect("the preference moved to the v2 collection");
    assert_eq!(
        found.doc,
        json!({ "read": true, "write": false, "admin": false })
    );
    assert_eq!(found.meta.schema, 2);
    // The v1 document is gone once the step completes.
    assert!(
        storage
            .documents()
            .get(LEGACY_PREFS_NAMESPACE, "gmail")
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn a_migrated_preference_reads_through_the_store_with_its_choice_intact() {
    let backend = MemoryStorage::new();
    let storage = module_storage(&backend);
    // A denied write must stay denied: it is the case a lossy step would flip.
    seed_v1(
        &storage,
        "slack",
        json!({ "value": { "read": true, "write": false, "admin": false } }),
    )
    .await;
    migrated(&storage, &backend).await;

    let store = DocumentPrefsStore::new(Arc::clone(storage.documents()));
    let pref = UserScopePref::load(&store, "slack").await.unwrap();
    assert!(pref.read);
    assert!(!pref.write);
}

#[tokio::test]
async fn a_non_object_preference_moves_under_a_named_field_and_reads_back_whole() {
    let backend = MemoryStorage::new();
    let storage = module_storage(&backend);
    seed_v1(&storage, "slack", json!({ "value": [1, 2] })).await;
    seed_v1(&storage, "notion", json!({ "value": "x" })).await;
    migrated(&storage, &backend).await;

    let found = storage
        .documents()
        .get(PREFS_NAMESPACE, "slack")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(found.doc, json!({ "non_object_value": [1, 2] }));

    let store = DocumentPrefsStore::new(Arc::clone(storage.documents()));
    assert_eq!(store.get("slack").await.unwrap().unwrap(), json!([1, 2]));
    assert_eq!(store.get("notion").await.unwrap().unwrap(), json!("x"));
}

#[tokio::test]
async fn an_object_preference_that_uses_the_reserved_field_is_not_confused_with_a_wrapper() {
    let backend = MemoryStorage::new();
    let storage = module_storage(&backend);
    let tricky = json!({ "non_object_value": 7 });
    seed_v1(&storage, "gmail", json!({ "value": tricky })).await;
    migrated(&storage, &backend).await;

    let store = DocumentPrefsStore::new(Arc::clone(storage.documents()));
    assert_eq!(store.get("gmail").await.unwrap().unwrap(), tricky);
}

#[tokio::test]
async fn a_v1_document_without_a_value_stays_an_unreadable_preference_not_a_default() {
    // Defaulting it would silently re-enable a write the user denied.
    let backend = MemoryStorage::new();
    let storage = module_storage(&backend);
    seed_v1(&storage, "gmail", json!({ "other": 1 })).await;
    migrated(&storage, &backend).await;

    let store = DocumentPrefsStore::new(Arc::clone(storage.documents()));
    let loaded = UserScopePref::load(&store, "gmail").await;
    assert!(loaded.is_err(), "a corrupt row must not read as defaults");
}

#[tokio::test]
async fn the_step_keeps_the_creation_time_and_is_idempotent() {
    let backend = MemoryStorage::new();
    let storage = module_storage(&backend);
    seed_v1(&storage, "gmail", json!({ "value": { "read": false } })).await;
    let created = storage
        .documents()
        .get(LEGACY_PREFS_NAMESPACE, "gmail")
        .await
        .unwrap()
        .unwrap()
        .meta
        .created_ms;

    migrated(&storage, &backend).await;
    migrated(&storage, &backend).await;

    let found = storage
        .documents()
        .get(PREFS_NAMESPACE, "gmail")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(found.doc, json!({ "read": false }));
    assert_eq!(found.meta.created_ms, created);
}

#[tokio::test]
async fn trigger_day_streams_move_to_the_v2_name_with_their_entries_untouched() {
    let backend = MemoryStorage::new();
    let storage = module_storage(&backend);
    let entry = |id: &str| {
        json!({
            "received_at_ms": 1_760_000_000_000_u64,
            "toolkit": "gmail",
            "trigger": "GMAIL_NEW_GMAIL_MESSAGE",
            "metadata_id": id,
            "metadata_uuid": "u",
            "payload": { "subject": "hi" },
        })
    };
    storage
        .streams()
        .append(LEGACY_STREAM, entry("evt-1"))
        .await
        .unwrap();
    storage
        .streams()
        .append(LEGACY_STREAM, entry("evt-2"))
        .await
        .unwrap();

    migrated(&storage, &backend).await;

    let moved = storage.streams().read_window(STREAM, 0, 10).await.unwrap();
    let values: Vec<&Value> = moved.iter().map(|record| &record.value).collect();
    assert_eq!(values, [&entry("evt-1"), &entry("evt-2")]);
    assert!(
        storage
            .streams()
            .streams("triggers/")
            .await
            .unwrap()
            .is_empty()
    );

    // The archive reads the migrated history back through its public API.
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
}
