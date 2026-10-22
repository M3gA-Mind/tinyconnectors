use serde_json::json;
use tinyconnectors::state::DocumentPrefsStore;
use tinyconnectors_sync::PrefsStore;
use tinystoragedrivers_core::{Key, MemoryStorage, Scope, StorageBackend};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let backend = MemoryStorage::new();
    let storage = backend
        .for_scope(&Scope::local().module("connectors")?)?;
    let preferences = DocumentPrefsStore::new(storage.documents().clone());
    preferences
        .set("gmail", &json!({"read": true, "write": false, "admin": false}))
        .await?;

    let id = Key::of(["gmail"])?;
    let record = storage
        .documents()
        .get(tinyconnectors_sync::PREFS_NAMESPACE, id.as_str())
        .await?
        .expect("the public writer persisted the preference");
    println!("{}", serde_json::to_string_pretty(&record)?);
    Ok(())
}
