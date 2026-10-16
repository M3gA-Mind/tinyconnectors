//! The module's storage schema: what it stores and how v1 data reaches v2.
//!
//! The host composes this with every other owner's schema
//! (`Registry::compose`), declares the items, and runs the steps over the
//! module's scope. Everything here is [`ScopeClass::Module`], so the host's
//! grant for the module is generated from [`storage_schema`] rather than kept
//! by hand (see [`storage_needs`](super::storage_needs)).
//!
//! # v1 to v2
//!
//! | v1 | v2 | change |
//! |---|---|---|
//! | collection `composio-user-scopes`, id = toolkit, body `{ "value": … }` | collection `connectors.user_scopes`, id = `Key::of([toolkit])` | the `value` wrapper is removed |
//! | streams `triggers/<YYYY-MM-DD>` | streams `connectors.triggers/<YYYY-MM-DD>` | renamed, entries unchanged |
//!
//! A document body must be a JSON object, but a preference is any JSON value.
//! An object value becomes the body as it is. Any other value is stored under
//! the reserved field [`NON_OBJECT_FIELD`]: `{ "non_object_value": <value> }`.
//! An object value that itself uses that field is wrapped the same way, so a
//! body with exactly that one field always means "wrapped" and the encoding
//! loses nothing.

use serde_json::{Value, json};
use tinyconnectors_sync::{LEGACY_PREFS_NAMESPACE, PREFS_NAMESPACE};
use tinystoragedrivers_core::schema::{Owner, Schema, ScopeClass, SourceRecord, Step, TargetRecord};
use tinystoragedrivers_core::{Key, Result, SealPolicy, SealWhen};

use crate::triggers::{LEGACY_STREAM_PREFIX, STREAM_NAME, STREAM_PREFIX};

/// The owner prefix every v2 name of this module starts with.
const OWNER: &str = "connectors";

/// The body version [`PREFS_NAMESPACE`] documents are written at.
pub(super) const PREFS_SCHEMA_VERSION: u32 = 2;

/// Reserved field holding a preference that is not a plain object.
pub(super) const NON_OBJECT_FIELD: &str = "non_object_value";

/// A finished day's stream is sealed once it has been idle this long. Nothing
/// appends to a day after its UTC midnight, so two days is a wide margin.
const DAY_STREAM_IDLE_MS: u64 = 2 * 24 * 60 * 60 * 1000;

/// Everything this module stores, plus the steps that migrate v1 data.
///
/// # Panics
///
/// Never in practice: the owner name is a constant that satisfies the owner
/// rules, which a test pins.
#[must_use]
#[allow(clippy::expect_used)]
pub fn storage_schema() -> Schema {
    let owner = Owner::new(OWNER).expect("`connectors` is a valid owner name");
    Schema::new(owner)
        .collection(PREFS_NAMESPACE, PREFS_SCHEMA_VERSION, ScopeClass::Module)
        .stream_with(
            STREAM_NAME,
            ScopeClass::Module,
            SealPolicy::new(SealWhen::Idle(DAY_STREAM_IDLE_MS)),
        )
        .step(
            Step::new(
                "user_scopes_unwrap_value",
                PREFS_NAMESPACE,
                1,
                PREFS_SCHEMA_VERSION,
                unwrap_user_scopes,
            )
            .source(LEGACY_PREFS_NAMESPACE),
        )
        .step(
            Step::stream(
                "triggers_rename",
                STREAM_NAME,
                1,
                2,
                |source, entries| Ok(rename_trigger_stream(&source, entries)),
            )
            .source_prefix(LEGACY_STREAM_PREFIX),
        )
}

/// The v2 body for a preference value.
pub(super) fn encode_preference(value: &Value) -> Value {
    match value {
        Value::Object(fields) if !fields.contains_key(NON_OBJECT_FIELD) => value.clone(),
        other => json!({ NON_OBJECT_FIELD: other }),
    }
}

/// The preference value a v2 body holds: the inverse of [`encode_preference`].
pub(super) fn decode_preference(body: Value) -> Value {
    match body {
        Value::Object(mut fields) if fields.len() == 1 && fields.contains_key(NON_OBJECT_FIELD) => {
            fields.remove(NON_OBJECT_FIELD).unwrap_or(Value::Null)
        }
        other => other,
    }
}

/// Step `user_scopes_unwrap_value`: `{ "value": v }` becomes the encoding of `v`.
///
/// A v1 document with no `value` field was unreadable (the store reported it
/// corrupt rather than unset). It is carried as a wrapped `null`, which no
/// preference type decodes, so it stays an error instead of becoming the
/// default and silently re-enabling a write the user denied.
fn unwrap_user_scopes(batch: Vec<SourceRecord>) -> Result<Vec<TargetRecord>> {
    batch
        .into_iter()
        .map(|record| {
            let value = match record.body {
                Value::Object(mut fields) => fields.remove("value").unwrap_or(Value::Null),
                _ => Value::Null,
            };
            let id = Key::of([record.id.as_str()])?;
            let mut target =
                TargetRecord::new(PREFS_NAMESPACE, id.as_str(), encode_preference(&value));
            if record.meta.created_ms > 0 {
                target = target.created_ms(record.meta.created_ms);
            }
            Ok(target)
        })
        .collect()
}

/// Step `triggers_rename`: `triggers/<day>` becomes `connectors.triggers/<day>`.
/// A stream that is not under the v1 prefix is dropped, which the runner
/// never asks for. Entries already have the v2 shape.
fn rename_trigger_stream(source: &str, entries: Vec<Value>) -> Option<(String, Vec<Value>)> {
    let day = source.strip_prefix(LEGACY_STREAM_PREFIX)?;
    Some((format!("{STREAM_PREFIX}{day}"), entries))
}

#[cfg(test)]
#[path = "schema_tests.rs"]
mod tests;
