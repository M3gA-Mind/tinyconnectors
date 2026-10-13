//! A preference store backed by one document per toolkit.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{Value, json};
use tinyconnectors_sync::{Error as SyncError, PREFS_NAMESPACE, PrefsStore, Result as SyncResult};
use tinystoragedrivers_core::{DocumentStore, Precondition, StorageError};

/// Key inside the stored document holding the preference value.
///
/// Documents must be JSON objects, a preference is any JSON value, so the
/// value is wrapped rather than stored bare.
const VALUE_FIELD: &str = "value";

/// Scope preferences persisted through a [`DocumentStore`] port.
///
/// The collection is [`PREFS_NAMESPACE`] and the document id is the toolkit
/// key. The port is already bound to the caller's scope, so one user's
/// preferences are never visible to another.
#[derive(Debug, Clone)]
pub struct DocumentPrefsStore {
    documents: Arc<dyn DocumentStore>,
}

impl DocumentPrefsStore {
    /// Store preferences in `documents`.
    #[must_use]
    pub fn new(documents: Arc<dyn DocumentStore>) -> Self {
        Self { documents }
    }
}

fn failed(key: &str, action: &str, error: &StorageError) -> SyncError {
    tracing::debug!(key, action, %error, "[connectors][prefs] storage call failed");
    SyncError::Store {
        key: key.to_string(),
        message: format!("could not {action}: {error}"),
    }
}

#[async_trait]
impl PrefsStore for DocumentPrefsStore {
    async fn get(&self, key: &str) -> SyncResult<Option<Value>> {
        let found = self
            .documents
            .get(PREFS_NAMESPACE, key)
            .await
            .map_err(|error| failed(key, "read the preference", &error))?;
        // A toolkit the user never configured is the normal case, not a failure.
        // A document that exists without its value is corrupt, not "unset":
        // defaulting it would silently re-enable a write the user denied.
        match found {
            None => Ok(None),
            Some(versioned) => match versioned.doc.get(VALUE_FIELD) {
                Some(value) => Ok(Some(value.clone())),
                None => Err(SyncError::Store {
                    key: key.to_string(),
                    message: format!("the stored preference has no `{VALUE_FIELD}` field"),
                }),
            },
        }
    }

    async fn set(&self, key: &str, value: &Value) -> SyncResult<()> {
        self.documents
            .put(
                PREFS_NAMESPACE,
                key,
                json!({ VALUE_FIELD: value }),
                Precondition::None,
            )
            .await
            .map(|_| ())
            .map_err(|error| failed(key, "write the preference", &error))
    }
}
