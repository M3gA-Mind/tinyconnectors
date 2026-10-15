//! Translation of Composio's v3 trigger rows into the contract's envelopes.
//!
//! Pure functions over JSON: the direct route makes the calls and this file
//! reshapes what comes back. Kept apart from the route so the request flow in
//! `direct.rs` reads without the field-by-field drift handling.

use super::{field, slug_field};
use crate::{ComposioActiveTrigger, ComposioAvailableTrigger};

/// The rows of a v3 listing, which has come as `{items}`, `{data}` or a bare array.
pub(super) fn rows_of(value: &serde_json::Value) -> Vec<serde_json::Value> {
    value
        .get("items")
        .or_else(|| value.get("data"))
        .unwrap_or(value)
        .as_array()
        .cloned()
        .unwrap_or_default()
}

/// The cursor for the next page, when the listing says there is one.
pub(super) fn next_cursor(value: &serde_json::Value) -> Option<String> {
    slug_field(value, &["next_cursor", "nextCursor"])
}

/// Translate one `triggers_types` row into a [`ComposioAvailableTrigger`].
///
/// Every row is `scope: "static"`: v3 describes a trigger type once per
/// toolkit, and a repository-bound trigger takes its repository as ordinary
/// required configuration. Defaults come from the config schema's
/// `properties[*].default`, and the required keys from its `required` list.
pub(super) fn available_trigger_from_v3(
    row: &serde_json::Value,
) -> Option<ComposioAvailableTrigger> {
    let slug = slug_field(row, &["slug", "name"])?;
    let config = row.get("config");

    let (defaults, required) = config_shape(config);

    Some(ComposioAvailableTrigger {
        slug,
        scope: "static".to_string(),
        default_config: (!defaults.is_empty()).then_some(serde_json::Value::Object(defaults)),
        required_config_keys: required,
        repo: None,
    })
}

/// The defaults and required keys of a trigger type's `config`.
///
/// Composio has shipped two shapes: a JSON Schema object (`properties` plus a
/// `required` list) and a bare map of field name to a descriptor carrying
/// `required: true` and an optional `default`. Both are read.
fn config_shape(
    config: Option<&serde_json::Value>,
) -> (
    serde_json::Map<String, serde_json::Value>,
    Option<Vec<String>>,
) {
    let Some(config) = config.and_then(serde_json::Value::as_object) else {
        return (serde_json::Map::new(), None);
    };
    let schema_fields = config
        .get("properties")
        .and_then(serde_json::Value::as_object);
    let required_list = config.get("required").and_then(serde_json::Value::as_array);

    if schema_fields.is_some() || required_list.is_some() {
        let defaults = schema_fields
            .into_iter()
            .flatten()
            .filter_map(|(key, schema)| schema.get("default").map(|d| (key.clone(), d.clone())))
            .collect();
        let required = required_list.map(|keys| {
            keys.iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::to_string)
                .collect()
        });
        return (defaults, required);
    }

    let fields = || config.iter().filter(|(_, field)| field.is_object());
    let defaults = fields()
        .filter_map(|(key, field)| field.get("default").map(|d| (key.clone(), d.clone())))
        .collect();
    let required: Vec<String> = fields()
        .filter(|(_, field)| {
            field.get("required").and_then(serde_json::Value::as_bool) == Some(true)
        })
        .map(|(key, _)| key.clone())
        .collect();
    (defaults, (!required.is_empty()).then_some(required))
}

/// The keys a row may carry its connected account's id under.
const CONNECTION_KEYS: &[&str] = &[
    "connected_account_id",
    "connectedAccountId",
    "connection_id",
];

/// Whether an active-instance row names no toolkit of its own.
pub(super) fn names_no_toolkit(row: &serde_json::Value) -> bool {
    slug_field(row, &["toolkit", "toolkit_slug", "app_name", "appName"]).is_none()
}

/// Whether an active-instance row belongs to `entity_id`.
///
/// A row with no `user_id` is kept: the listing was already filtered by user,
/// and dropping an owner-less row would make a live subscription look deleted.
/// A row naming a *different* user is never kept, whatever the filter did.
pub(super) fn owned_by(row: &serde_json::Value, entity_id: &str) -> bool {
    slug_field(row, &["user_id", "userId"]).is_none_or(|owner| owner == entity_id)
}

/// Whether a row positively names `entity_id` as its owner.
///
/// Stricter than [`owned_by`], which keeps an owner-less row for a listing:
/// deleting a subscription requires the owner to be on the row.
pub(super) fn names_owner(row: &serde_json::Value, entity_id: &str) -> bool {
    slug_field(row, &["user_id", "userId"]).is_some_and(|owner| owner == entity_id)
}

/// The toolkit a trigger slug belongs to, by its first word: `GMAIL_…` → `gmail`.
///
/// Only a fallback. A toolkit slug can itself hold an underscore, so an
/// explicit toolkit field, or the caller's own filter, is always preferred.
pub(super) fn toolkit_of_slug(slug: &str) -> String {
    slug.split('_')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase()
}

/// Whether a trigger slug names a trigger of `toolkit`: `MICROSOFT_TEAMS_X`
/// belongs to `microsoft_teams`, and `GMAIL_X` does not belong to `gmai`.
fn slug_in_toolkit(slug: &str, toolkit: &str) -> bool {
    let prefix = format!("{}_", toolkit.to_ascii_uppercase());
    slug.to_ascii_uppercase().starts_with(&prefix)
}

/// Translate one active-instance row into a [`ComposioActiveTrigger`].
///
/// With a `toolkit` filter, a row outside it yields `None`, and a row inside
/// it is labelled with the filter rather than with the slug's first word.
/// Without one, a row that names no toolkit takes its connection's toolkit from
/// `connection_toolkits`, and only then the slug's first word.
pub(super) fn active_trigger_from_v3(
    row: &serde_json::Value,
    toolkit: Option<&str>,
    connection_toolkits: &std::collections::HashMap<String, String>,
) -> Option<ComposioActiveTrigger> {
    let id = slug_field(row, &["id", "trigger_id", "triggerId"])?;
    let slug = slug_field(row, &["trigger_name", "triggerName", "slug"]).unwrap_or_default();
    let explicit = slug_field(row, &["toolkit", "toolkit_slug", "app_name", "appName"]);

    let toolkit = match toolkit.map(str::trim).filter(|t| !t.is_empty()) {
        Some(wanted) => {
            let matches = match &explicit {
                Some(explicit) => explicit.eq_ignore_ascii_case(wanted),
                None => slug_in_toolkit(&slug, wanted),
            };
            if !matches {
                return None;
            }
            explicit.unwrap_or_else(|| wanted.to_string())
        }
        None => explicit
            .or_else(|| {
                field(row, CONNECTION_KEYS)
                    .and_then(|id| connection_toolkits.get(&id).cloned())
                    .filter(|toolkit| !toolkit.is_empty())
            })
            .unwrap_or_else(|| toolkit_of_slug(&slug)),
    };

    let disabled = row
        .get("disabled_at")
        .or_else(|| row.get("disabledAt"))
        .is_some_and(|at| !at.is_null());

    Some(ComposioActiveTrigger {
        id,
        slug,
        toolkit,
        connection_id: field(row, CONNECTION_KEYS).unwrap_or_default(),
        trigger_config: row
            .get("trigger_config")
            .or_else(|| row.get("triggerConfig"))
            .filter(|config| !config.is_null())
            .cloned(),
        state: Some(if disabled { "disabled" } else { "active" }.to_string()),
    })
}
