//! Walks over JSON Schema and JSON values that find where a tool's list lives
//! and which fields it declares.
//!
//! Vendor-neutral: the one piece of provider knowledge a walk needs, the
//! response-envelope keys to skip at the root, is a parameter. These are the
//! same walks the agent harness ships; they are repeated here only because the
//! module must not depend on the harness. They are small and pure.

use serde_json::Value;

/// Dotted path to the shallowest array-typed property declared by `schema`.
///
/// Breadth-first, so the shallowest match wins: every property at one level is
/// checked for `type: "array"` before descending, which matters when a schema
/// declares both a top-level list and a deeper one and a binding should prefer
/// the outer.
///
/// `None` when `schema` is absent, declares no `properties`, or names no array
/// anywhere.
#[must_use]
pub(crate) fn compute_primary_array_path(schema: Option<&Value>) -> Option<String> {
    let root = schema?;
    let mut queue: std::collections::VecDeque<(String, &Value)> = std::collections::VecDeque::new();
    queue.push_back((String::new(), root));

    while let Some((path, node)) = queue.pop_front() {
        let Some(props) = node.get("properties").and_then(Value::as_object) else {
            continue;
        };
        // Check every property at THIS level for an array before descending
        // to the next level — guarantees the shallowest match wins.
        for (key, prop_schema) in props {
            let is_array = prop_schema.get("type").is_some_and(|kind| {
                kind.as_str() == Some("array")
                    || kind.as_array().is_some_and(|types| {
                        types.iter().any(|value| value.as_str() == Some("array"))
                    })
            });
            if is_array {
                let prop_path = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                return Some(prop_path);
            }
        }
        for (key, prop_schema) in props {
            let prop_path = if path.is_empty() {
                key.clone()
            } else {
                format!("{path}.{key}")
            };
            queue.push_back((prop_path, prop_schema));
        }
    }
    None
}

/// [`compute_primary_array_path`]'s counterpart for a REAL runtime value rather
/// than a schema — walks the value breadth-first for the first array-typed
/// property.
///
/// `skip_root_keys` names properties to ignore **at the root only**. A caller
/// whose values arrive wrapped in a response envelope passes the envelope's own
/// metadata keys here, so an envelope field can never masquerade as (or shadow,
/// via the shallowest-wins tie) the tool's real array. Pass `&[]` when the value
/// has no envelope.
///
/// Because the scan starts at whatever root it is given, a hit beneath a wrapper
/// property comes back already prefixed (e.g. `"data.issues"`) with no separate
/// stitching step needed.
///
/// `None` when no array is found anywhere in the value — note a genuinely empty
/// list still serializes as `[]`, which *is* an array, so `None` means the shape
/// truly has no list at any depth.
#[must_use]
pub(crate) fn compute_primary_array_path_from_value(
    value: &Value,
    skip_root_keys: &[&str],
) -> Option<String> {
    let mut queue: std::collections::VecDeque<(String, &Value)> = std::collections::VecDeque::new();
    queue.push_back((String::new(), value));

    while let Some((path, node)) = queue.pop_front() {
        let Some(obj) = node.as_object() else {
            continue;
        };
        for (key, v) in obj {
            if path.is_empty() && skip_root_keys.contains(&key.as_str()) {
                continue;
            }
            if v.is_array() {
                let prop_path = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                return Some(prop_path);
            }
        }
        for (key, v) in obj {
            if path.is_empty() && skip_root_keys.contains(&key.as_str()) {
                continue;
            }
            let prop_path = if path.is_empty() {
                key.clone()
            } else {
                format!("{path}.{key}")
            };
            queue.push_back((prop_path, v));
        }
    }
    None
}

/// Extracts top-level field names from an output-schema value.
///
/// Reads `.properties`' keys for a standard object schema
/// (`{"type": "object", "properties": {…}}`), and falls back to the schema's own
/// top-level keys only when no standard JSON-Schema keywords identify the
/// input as a schema. Empty when the schema is absent or a recognized schema
/// declares no properties — never fails.
#[must_use]
pub(crate) fn response_fields_from_schema(schema: Option<&Value>) -> Vec<String> {
    const SCHEMA_KEYWORDS: &[&str] = &[
        "type",
        "required",
        "additionalProperties",
        "$schema",
        "description",
        "title",
        "examples",
        "$id",
        "$anchor",
        "$ref",
        "$defs",
        "definitions",
        "properties",
        "patternProperties",
        "propertyNames",
        "minProperties",
        "maxProperties",
        "dependencies",
        "dependentRequired",
        "dependentSchemas",
        "unevaluatedProperties",
        "contentEncoding",
        "contentMediaType",
        "contentSchema",
        "default",
        "deprecated",
        "readOnly",
        "writeOnly",
        "const",
        "enum",
        "format",
        "$comment",
        "$vocabulary",
        "$dynamicRef",
        "$dynamicAnchor",
        "allOf",
        "anyOf",
        "oneOf",
        "not",
        "if",
        "then",
        "else",
        "minimum",
        "maximum",
        "exclusiveMinimum",
        "exclusiveMaximum",
        "multipleOf",
        "minLength",
        "maxLength",
        "pattern",
        "minItems",
        "maxItems",
        "uniqueItems",
        "contains",
        "minContains",
        "maxContains",
        "items",
        "prefixItems",
        "unevaluatedItems",
    ];

    let Some(obj) = schema.and_then(Value::as_object) else {
        return Vec::new();
    };
    let mut fields: Vec<String> =
        if let Some(props) = obj.get("properties").and_then(Value::as_object) {
            props.keys().cloned().collect()
        } else {
            obj.keys()
                .filter(|k| !SCHEMA_KEYWORDS.contains(&k.as_str()))
                .cloned()
                .collect()
        };
    fields.sort();
    fields
}

#[cfg(test)]
#[path = "schema_walk_tests.rs"]
mod tests;
