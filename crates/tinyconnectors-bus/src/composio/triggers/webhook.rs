//! Verify and decode Composio's webhook deliveries.
//!
//! For a host that owns a public webhook endpoint and receives Composio's
//! trigger deliveries there itself, rather than through a backend that fans
//! them out. Two pure functions, no transport and no clock:
//!
//! - [`verify`] checks a delivery's signature and timestamp against the
//!   webhook secret. Run it on the raw body bytes, before parsing anything.
//! - [`decode`] turns a verified body into a [`ComposioTriggerEvent`], the same
//!   shape a backend fan-out delivers, from either payload version.
//!
//! # The signature scheme
//!
//! Each delivery carries three headers: [`HEADER_ID`], [`HEADER_TIMESTAMP`]
//! (unix seconds) and [`HEADER_SIGNATURE`]. The signed content is
//! `"{id}.{timestamp}.{raw body}"`, the MAC is HMAC-SHA256 keyed with the
//! secret string's own bytes (it is not base64-decoded, and a `whsec_` prefix
//! is part of the key), and the digest is base64-encoded. The signature header
//! holds one or more space-separated entries, each `v1,<base64>` or a bare
//! `<base64>`; the delivery is genuine if any `v1` entry matches, which is what
//! lets a secret rotate without dropping deliveries. Entries under another
//! scheme (`v1a,…`) are skipped. Digests are compared in constant time.
//!
//! The timestamp is checked against a caller-supplied `now`, within a
//! tolerance in either direction ([`DEFAULT_TOLERANCE_SECS`] is the usual
//! choice). That bounds how long a captured delivery can be replayed; a host
//! that must refuse every replay also remembers recent `webhook-id`s.
//!
//! # The two payload versions
//!
//! The current (V3) body is `{id, type: "composio.trigger.message", timestamp,
//! metadata: {log_id, trigger_slug, trigger_id, connected_account_id, user_id,
//! …}, data}`. The legacy (V2) body puts the lowercase trigger slug in `type`,
//! the `log_id` at the top level, and mixes the routing ids (`connection_id`,
//! `trigger_nano_id` or `trigger_id`, `user_id`) into `data` beside the event.
//! [`decode`] accepts both, and the older V1 envelope (top-level `trigger_name`,
//! `connection_id`, `payload`); for V2 the payload is `data` as sent, routing keys
//! included, so nothing the provider sent is lost.
//!
//! Neither version names the toolkit reliably. An explicit `toolkit_slug` (or
//! `toolkit`) wins; otherwise it is the lowercase first word of the trigger
//! slug (`GMAIL_NEW_GMAIL_MESSAGE` → `gmail`), which is wrong for a toolkit
//! whose own slug holds an underscore.

use base64::Engine as _;
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;

use super::{ComposioTriggerEvent, ComposioTriggerMetadata};

/// Name of the header carrying the delivery's unique message id.
pub const HEADER_ID: &str = "webhook-id";
/// Name of the header carrying the delivery's send time, in unix seconds.
pub const HEADER_TIMESTAMP: &str = "webhook-timestamp";
/// Name of the header carrying the delivery's signatures.
pub const HEADER_SIGNATURE: &str = "webhook-signature";
/// How far, in seconds, a delivery's timestamp may be from `now` either way.
pub const DEFAULT_TOLERANCE_SECS: u64 = 300;

/// The V3 `type` of a trigger delivery.
const V3_TRIGGER_TYPE: &str = "composio.trigger.message";

/// The signature headers of one delivery, as the host's HTTP server read them.
///
/// A header that was absent is passed as an empty string, and [`verify`]
/// reports it as missing.
#[derive(Debug, Clone, Copy)]
pub struct WebhookHeaders<'a> {
    /// The [`HEADER_ID`] value.
    pub id: &'a str,
    /// The [`HEADER_TIMESTAMP`] value.
    pub timestamp: &'a str,
    /// The [`HEADER_SIGNATURE`] value.
    pub signature: &'a str,
}

/// Why a delivery was refused.
///
/// Every variant means "do not act on this body". A host answers a delivery
/// that fails [`verify`] with a 4xx, so the sender does not count it as
/// delivered.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum WebhookError {
    /// A signature header was absent or blank.
    #[error("webhook header `{header}` is missing")]
    MissingHeader {
        /// Name of the missing header.
        header: &'static str,
    },
    /// A signature header was present but could not be read: a timestamp that
    /// is not unix seconds, or a signature header with no decodable entry.
    #[error("webhook header `{header}` is malformed")]
    MalformedHeader {
        /// Name of the malformed header.
        header: &'static str,
    },
    /// The webhook secret was empty. Anyone can compute a MAC under an empty
    /// key, so verifying with one would accept forgeries.
    #[error("webhook secret is empty")]
    EmptySecret,
    /// The delivery's timestamp is further from `now` than the tolerance, in
    /// either direction: a stale delivery may be a replay, and one from the
    /// future was not sent when it claims.
    #[error("webhook timestamp is {skew_secs}s from now, outside the {tolerance_secs}s tolerance")]
    StaleTimestamp {
        /// Distance between the timestamp and `now`, in seconds.
        skew_secs: u64,
        /// The tolerance it exceeded, in seconds.
        tolerance_secs: u64,
    },
    /// No `v1` signature in the header matched the body under this secret.
    #[error("webhook signature does not match")]
    SignatureMismatch,
    /// The body is not a trigger delivery in either payload version.
    #[error("webhook body is not a trigger delivery: {reason}")]
    UndecodableBody {
        /// What was wrong with it.
        reason: String,
    },
}

/// Check that a delivery was signed with `secret` and sent recently.
///
/// `body` must be the raw request body, byte for byte: re-serialized JSON does
/// not hash the same. `now_unix_secs` is the host's current time, passed in so
/// verification needs no clock; `tolerance_secs` bounds the distance between
/// it and the delivery's timestamp ([`DEFAULT_TOLERANCE_SECS`] is typical).
///
/// # Errors
///
/// - [`WebhookError::MissingHeader`] when a header is blank;
/// - [`WebhookError::EmptySecret`] when `secret` is blank;
/// - [`WebhookError::MalformedHeader`] when the timestamp is not unix seconds,
///   or no signature entry is decodable;
/// - [`WebhookError::StaleTimestamp`] when the timestamp is out of tolerance;
/// - [`WebhookError::SignatureMismatch`] when no `v1` signature matches.
///
/// # Examples
///
/// ```
/// use tinyconnectors_bus::composio::triggers::webhook::{
///     verify, WebhookError, WebhookHeaders, DEFAULT_TOLERANCE_SECS,
/// };
///
/// let headers = WebhookHeaders {
///     id: "msg_2Lh9",
///     timestamp: "1760000000",
///     signature: "v1,u3rhivKxA1i5wJshDB8nGZDxVEAR9ymjDSxmeO8/8I0=",
/// };
/// let body = br#"{"type":"composio.trigger.message"}"#;
///
/// assert_eq!(verify("whsec_dGVzdA==", &headers, body, 1_760_000_042, DEFAULT_TOLERANCE_SECS), Ok(()));
/// assert_eq!(
///     verify("another-secret", &headers, body, 1_760_000_042, DEFAULT_TOLERANCE_SECS),
///     Err(WebhookError::SignatureMismatch),
/// );
/// ```
pub fn verify(
    secret: &str,
    headers: &WebhookHeaders<'_>,
    body: &[u8],
    now_unix_secs: u64,
    tolerance_secs: u64,
) -> Result<(), WebhookError> {
    let id = present(headers.id, HEADER_ID)?;
    let timestamp = present(headers.timestamp, HEADER_TIMESTAMP)?;
    let signature = present(headers.signature, HEADER_SIGNATURE)?;
    if secret.trim().is_empty() {
        return Err(WebhookError::EmptySecret);
    }

    let sent_at: u64 = timestamp
        .parse()
        .map_err(|_| WebhookError::MalformedHeader {
            header: HEADER_TIMESTAMP,
        })?;
    let skew_secs = now_unix_secs.abs_diff(sent_at);
    if skew_secs > tolerance_secs {
        return Err(WebhookError::StaleTimestamp {
            skew_secs,
            tolerance_secs,
        });
    }

    let mut candidates = Vec::new();
    let mut undecodable = false;
    for entry in signature.split_whitespace() {
        let encoded = match entry.split_once(',') {
            Some(("v1", encoded)) => encoded,
            Some(_) => continue,
            None => entry,
        };
        match base64::engine::general_purpose::STANDARD.decode(encoded) {
            Ok(digest) => candidates.push(digest),
            Err(_) => undecodable = true,
        }
    }
    if candidates.is_empty() && undecodable {
        return Err(WebhookError::MalformedHeader {
            header: HEADER_SIGNATURE,
        });
    }

    let mut mac =
        Hmac::<Sha256>::new_from_slice(secret.as_bytes()).map_err(|_| WebhookError::EmptySecret)?;
    mac.update(id.as_bytes());
    mac.update(b".");
    mac.update(timestamp.as_bytes());
    mac.update(b".");
    mac.update(body);
    // `verify_slice` consumes the MAC, so each candidate checks a clone.
    if candidates
        .iter()
        .any(|digest| mac.clone().verify_slice(digest).is_ok())
    {
        Ok(())
    } else {
        Err(WebhookError::SignatureMismatch)
    }
}

/// Decode a delivery body, in either payload version, into a
/// [`ComposioTriggerEvent`].
///
/// Run [`verify`] first: this function trusts what it reads.
///
/// # Errors
///
/// Returns [`WebhookError::UndecodableBody`] when the body is not a JSON
/// object, or names no trigger slug in either version's place for it.
pub fn decode(body: &[u8]) -> Result<ComposioTriggerEvent, WebhookError> {
    decode_with_toolkits(body, &[])
}

/// [`decode`], resolving the toolkit against `known` toolkit slugs.
///
/// A delivery rarely names its toolkit, and a toolkit slug can hold an
/// underscore (`microsoft_teams`), so the slug's first word alone mislabels
/// it. When the payload names no toolkit, the longest slug in `known` that
/// prefixes the trigger slug wins; with no match, the first word is used.
///
/// # Errors
///
/// As [`decode`].
pub fn decode_with_toolkits(
    body: &[u8],
    known: &[&str],
) -> Result<ComposioTriggerEvent, WebhookError> {
    let value: serde_json::Value =
        serde_json::from_slice(body).map_err(|error| WebhookError::UndecodableBody {
            reason: error.to_string(),
        })?;
    let Some(object) = value.as_object() else {
        return Err(undecodable("the body is not a JSON object"));
    };
    let kind = text(object.get("type"));

    match object
        .get("metadata")
        .and_then(serde_json::Value::as_object)
    {
        Some(metadata) if kind == V3_TRIGGER_TYPE => decode_v3(&value, metadata, known),
        Some(_) => Err(undecodable("not a V3 trigger delivery")),
        None if !kind.is_empty() && kind != V3_TRIGGER_TYPE => decode_v2(&value, &kind, known)
            .ok_or_else(|| {
                undecodable(
                    "not a V2 trigger delivery: needs `timestamp` and `data` naming a connection",
                )
            }),
        None => decode_v1(&value, known)
            .ok_or_else(|| undecodable("no trigger slug in `metadata`, `type` or `trigger_name`")),
    }
}

/// Decode the current payload, whose routing ids live in `metadata`.
fn decode_v3(
    value: &serde_json::Value,
    metadata: &serde_json::Map<String, serde_json::Value>,
    known: &[&str],
) -> Result<ComposioTriggerEvent, WebhookError> {
    let trigger = text(metadata.get("trigger_slug"));
    if trigger.is_empty() {
        return Err(undecodable("`metadata` names no `trigger_slug`"));
    }
    Ok(ComposioTriggerEvent {
        toolkit: explicit_toolkit(metadata)
            .or_else(|| value.as_object().and_then(explicit_toolkit))
            .unwrap_or_else(|| toolkit_of(&trigger, known)),
        payload: value.get("data").cloned().unwrap_or_default(),
        metadata: ComposioTriggerMetadata {
            id: text(value.get("id")),
            uuid: text(metadata.get("log_id")),
            user_id: text(metadata.get("user_id")),
            connected_account_id: text(metadata.get("connected_account_id")),
            trigger_id: text(metadata.get("trigger_id")),
        },
        trigger,
    })
}

/// Decode the legacy payload, whose routing ids are mixed into `data`.
///
/// `None` unless the shape is a V2 trigger's: a `timestamp`, and `data` as an
/// object that names the connection it came from.
fn decode_v2(
    value: &serde_json::Value,
    kind: &str,
    known: &[&str],
) -> Option<ComposioTriggerEvent> {
    let trigger = kind.to_ascii_uppercase();
    let data = value.get("data").cloned().unwrap_or_default();
    let fields = data.as_object()?;
    value.get("timestamp").filter(|at| !at.is_null())?;
    let names_connection = [
        "connection_nano_id",
        "connection_id",
        "connected_account_id",
    ]
    .iter()
    .any(|key| !text(fields.get(*key)).is_empty());
    if !names_connection {
        return None;
    }
    let fields = Some(fields);
    let first = |keys: &[&str]| {
        keys.iter()
            .map(|key| text(fields.and_then(|fields| fields.get(*key))))
            .find(|value| !value.is_empty())
            .unwrap_or_default()
    };
    Some(ComposioTriggerEvent {
        toolkit: fields
            .and_then(explicit_toolkit)
            .or_else(|| value.as_object().and_then(explicit_toolkit))
            .unwrap_or_else(|| toolkit_of(&trigger, known)),
        metadata: ComposioTriggerMetadata {
            id: text(value.get("id")),
            uuid: text(value.get("log_id")),
            user_id: first(&["user_id"]),
            connected_account_id: first(&[
                "connection_nano_id",
                "connected_account_id",
                "connection_id",
            ]),
            trigger_id: first(&["trigger_nano_id", "trigger_id"]),
        },
        payload: data,
        trigger,
    })
}

/// Decode the oldest payload, whose routing ids sit at the top level beside
/// the provider's `payload`. `None` unless it names a `trigger_name`, its
/// `connection_id` and `trigger_id`, and carries a `payload`.
fn decode_v1(value: &serde_json::Value, known: &[&str]) -> Option<ComposioTriggerEvent> {
    let trigger = text(value.get("trigger_name"));
    let complete = !trigger.is_empty()
        && !text(value.get("connection_id")).is_empty()
        && !text(value.get("trigger_id")).is_empty()
        && value
            .get("payload")
            .is_some_and(|payload| !payload.is_null());
    if !complete {
        return None;
    }
    Some(ComposioTriggerEvent {
        toolkit: value
            .as_object()
            .and_then(explicit_toolkit)
            .unwrap_or_else(|| toolkit_of(&trigger, known)),
        metadata: ComposioTriggerMetadata {
            id: text(value.get("id")),
            uuid: text(value.get("log_id")),
            user_id: text(value.get("user_id")),
            connected_account_id: text(value.get("connection_id")),
            trigger_id: text(value.get("trigger_id")),
        },
        payload: value.get("payload").cloned().unwrap_or_default(),
        trigger: trigger.to_ascii_uppercase(),
    })
}

/// A header's trimmed value, or the error naming it when blank.
fn present<'a>(value: &'a str, header: &'static str) -> Result<&'a str, WebhookError> {
    let value = value.trim();
    if value.is_empty() {
        Err(WebhookError::MissingHeader { header })
    } else {
        Ok(value)
    }
}

/// A string value, trimmed; anything else is empty.
fn text(value: Option<&serde_json::Value>) -> String {
    value
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .unwrap_or_default()
        .to_string()
}

/// A toolkit the payload names outright, when it does.
fn explicit_toolkit(fields: &serde_json::Map<String, serde_json::Value>) -> Option<String> {
    ["toolkit_slug", "toolkit"]
        .iter()
        .map(|key| text(fields.get(*key)).to_ascii_lowercase())
        .find(|toolkit| !toolkit.is_empty())
}

/// The toolkit a trigger slug belongs to: the longest `known` slug that
/// prefixes it, else its first word.
fn toolkit_of(trigger: &str, known: &[&str]) -> String {
    let upper = trigger.to_ascii_uppercase();
    let best = known
        .iter()
        .filter(|toolkit| {
            !toolkit.is_empty() && upper.starts_with(&format!("{}_", toolkit.to_ascii_uppercase()))
        })
        .max_by_key(|toolkit| toolkit.len());
    if let Some(toolkit) = best {
        return toolkit.to_ascii_lowercase();
    }
    trigger
        .split('_')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase()
}

fn undecodable(reason: &str) -> WebhookError {
    WebhookError::UndecodableBody {
        reason: reason.to_string(),
    }
}

#[cfg(test)]
#[path = "webhook_tests.rs"]
mod test;
