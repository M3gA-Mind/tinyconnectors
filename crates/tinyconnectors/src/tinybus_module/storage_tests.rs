//! A link with no host behind it reports storage unavailable and debug-prints
//! without leaking anything.
#![allow(clippy::expect_used)]
use super::*;

#[test]
fn unlinked_debug_reports_disconnected() {
    let text = format!("{:?}", StorageLink::none());
    assert!(text.contains("StorageLink"));
    assert!(text.contains("connected: false"));
}

#[tokio::test]
async fn unlinked_stores_are_unavailable() {
    let link = StorageLink::none();
    assert!(link.documents().await.is_err());
    assert!(link.streams().await.is_err());
}

#[tokio::test]
async fn unlinked_preferences_report_read_and_write_failures() {
    use serde_json::json;
    use tinyconnectors_sync::PrefsStore;

    let store = LinkedPrefsStore::new(StorageLink::none());
    assert!(store.get("gmail").await.is_err());
    assert!(store.set("gmail", &json!({"read": true})).await.is_err());
}
