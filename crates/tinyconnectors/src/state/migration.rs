//! Owner-local migration of the connector module's persisted records.

use serde_json::{Value, json};
use tinystoragedrivers_api::{DocumentStore, Storage, StreamStore};
use tinystoragedrivers_types::{
    CollectionSpec, Key, Precondition, Query, StorageError, WriteOptions,
};

use tinyconnectors_bus::storage::{
    LEGACY_PREFS_NAMESPACE, LEGACY_STREAM_PREFIX, MIGRATIONS_NAMESPACE, PREFS_NAMESPACE,
    PREFS_SCHEMA_VERSION, STREAM_PREFIX, encode_preference,
};

const MIGRATIONS: &str = MIGRATIONS_NAMESPACE;
const PREFS_STEP: &str = "prefs-v1-unwrap";
const STREAMS_STEP: &str = "triggers-v1-rename";
const PAGE_SIZE: usize = 128;

/// Collections the host should ensure before opening this module.
#[must_use]
pub fn collections() -> Vec<CollectionSpec> {
    vec![
        CollectionSpec::new(PREFS_NAMESPACE),
        CollectionSpec::new(MIGRATIONS),
    ]
}

/// Run `TinyConnectors`' migrations against its already-bound storage ports.
///
/// # Errors
/// Returns the first storage or migration error. A later open can safely retry
/// because each migrated record is conditionally written and each step is
/// recorded only after its transformation completes.
pub(crate) async fn prepare(storage: &Storage) -> tinystoragedrivers_types::Result<()> {
    let documents = storage.documents();
    let streams = storage.streams();
    if !done(documents.as_ref(), PREFS_STEP).await? {
        migrate_preferences(documents.as_ref()).await?;
        mark_done(documents.as_ref(), PREFS_STEP).await?;
    }
    if !done(documents.as_ref(), STREAMS_STEP).await? {
        migrate_streams(streams.as_ref()).await?;
        mark_done(documents.as_ref(), STREAMS_STEP).await?;
    }
    Ok(())
}

async fn done(documents: &dyn DocumentStore, step: &str) -> tinystoragedrivers_types::Result<bool> {
    let Some(record) = documents.get(MIGRATIONS, step).await? else {
        return Ok(false);
    };
    if record.doc["owner"] == "connectors"
        && record.doc["step"] == step
        && record.doc["done"] == true
    {
        Ok(true)
    } else {
        Err(StorageError::conflict(format!(
            "invalid connector migration marker for `{step}`"
        )))
    }
}

async fn mark_done(
    documents: &dyn DocumentStore,
    step: &str,
) -> tinystoragedrivers_types::Result<()> {
    match documents
        .put(
            MIGRATIONS,
            step,
            json!({"owner": "connectors", "step": step, "done": true}),
            Precondition::Absent,
        )
        .await
    {
        Ok(_) => Ok(()),
        Err(error) if error.kind() == tinystoragedrivers_types::ErrorKind::Conflict => {
            if done(documents, step).await? {
                Ok(())
            } else {
                Err(error)
            }
        }
        Err(error) => Err(error),
    }
}

async fn migrate_preferences(
    documents: &dyn DocumentStore,
) -> tinystoragedrivers_types::Result<()> {
    let mut query = Query::all().limit(PAGE_SIZE);
    loop {
        let page = documents.query(LEGACY_PREFS_NAMESPACE, &query).await?;
        if page.items.is_empty() {
            break;
        }
        let next = page.next.clone();
        for record in page.items {
            let id = legacy_key([record.id.as_str()])?;
            let value = match record.doc {
                Value::Object(mut fields) => fields.remove("value").unwrap_or(Value::Null),
                _ => Value::Null,
            };
            let body = encode_preference(&value);
            // Expiration is application-significant and is carried over the
            // module storage bus. `created_ms` is host-owned, so the target
            // receives the host's creation time for this new cross-collection
            // record instead of impersonating the legacy record's timestamp.
            let mut options = WriteOptions::new().schema(PREFS_SCHEMA_VERSION);
            if let Some(expires_ms) = record.meta.expires_ms {
                options = options.expires_ms(expires_ms);
            }
            match documents
                .put_with(
                    PREFS_NAMESPACE,
                    id.as_str(),
                    body.clone(),
                    Precondition::Absent,
                    options,
                )
                .await
            {
                Ok(_) => {}
                Err(error) if error.kind() == tinystoragedrivers_types::ErrorKind::Conflict => {
                    let current = documents.get(PREFS_NAMESPACE, id.as_str()).await?;
                    if current.is_none_or(|current| current.doc != body) {
                        return Err(error);
                    }
                }
                Err(error) => return Err(error),
            }
        }
        let Some(cursor) = next else { break };
        query.cursor = Some(cursor);
    }
    Ok(())
}

async fn migrate_streams(streams: &dyn StreamStore) -> tinystoragedrivers_types::Result<()> {
    let names = streams.streams(LEGACY_STREAM_PREFIX).await?;
    for source in names {
        let Some(day) = source.strip_prefix(LEGACY_STREAM_PREFIX) else {
            continue;
        };
        let target = format!("{STREAM_PREFIX}{day}");
        let source_len = streams.len(&source).await?;
        let target_len = streams.len(&target).await?;
        if target_len > source_len {
            return Err(StorageError::conflict(format!(
                "migration target `{target}` is longer than source `{source}`"
            )));
        }
        let mut offset = target_len;
        while offset < source_len {
            let entries = streams.read_window(&source, offset, PAGE_SIZE).await?;
            if entries.is_empty() {
                return Err(StorageError::conflict(format!(
                    "migration source `{source}` changed while it was copied"
                )));
            }
            let values = entries
                .into_iter()
                .map(|entry| entry.value)
                .collect::<Vec<_>>();
            match streams
                .append_batch_if_offset(&target, offset, values.clone())
                .await
            {
                Ok(copied) if copied == offset => {}
                Ok(_) => {
                    return Err(StorageError::conflict(format!(
                        "migration target `{target}` changed while it was copied"
                    )));
                }
                Err(error) if error.kind() == tinystoragedrivers_types::ErrorKind::Conflict => {
                    // Another opener may have copied this same page. Accept
                    // only an exact match at the expected offsets.
                    let raced = streams.read_window(&target, offset, values.len()).await?;
                    if raced.len() != values.len()
                        || raced
                            .iter()
                            .zip(&values)
                            .any(|(entry, value)| entry.value != *value)
                    {
                        return Err(error);
                    }
                }
                Err(error) => return Err(error),
            }
            offset = streams.len(&target).await?;
        }
        if streams.len(&source).await? != source_len {
            return Err(StorageError::conflict(format!(
                "migration source `{source}` changed while it was copied"
            )));
        }
        streams.delete_stream(&source).await?;
    }
    Ok(())
}

/// Preserve the retired core `Key::of` encoding for on-disk ids.
pub(super) fn legacy_key(
    parts: impl IntoIterator<Item = impl AsRef<str>>,
) -> tinystoragedrivers_types::Result<Key> {
    const MAX_ID_LEN: usize = 512;
    let mut escaped = String::new();
    let mut count = 0;
    for part in parts {
        if count > 0 {
            escaped.push('/');
        }
        for (index, ch) in part.as_ref().chars().enumerate() {
            match ch {
                '%' => escaped.push_str("%25"),
                '/' => escaped.push_str("%2F"),
                '\0' => escaped.push_str("%00"),
                '~' if count == 0 && index == 0 => escaped.push_str("%7E"),
                other => escaped.push(other),
            }
        }
        count += 1;
    }
    if count == 0 {
        return Err(StorageError::invalid_input("a key needs at least one part"));
    }
    if escaped.len() > MAX_ID_LEN {
        use sha2::{Digest, Sha256};
        escaped = format!(
            "~{}",
            data_encoding::BASE32_NOPAD
                .encode(&Sha256::digest(escaped.as_bytes()))
                .to_ascii_lowercase()
        );
    }
    Key::new(escaped)
}

#[cfg(test)]
#[path = "migration_tests.rs"]
mod tests;
