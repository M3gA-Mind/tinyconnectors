//! Percent-encoding for values that become part of a request URL.

use percent_encoding::{AsciiSet, NON_ALPHANUMERIC};

/// Everything that must be escaped in a path segment or query value.
///
/// `NON_ALPHANUMERIC` alone is wrong here: it escapes `-`, `.`, `_` and `~`,
/// which RFC 3986 lists as *unreserved* and which appear literally in real
/// Composio identifiers (`conn_9`, `ca-1`). Encoding them produces a different
/// string from the one the backend stored, and the lookup misses.
const UNRESERVED_KEPT: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

/// Encode one value for use in a path segment or query value.
///
/// Every id, slug, and tag here arrived over the bus. An unencoded `/` in a
/// connection id would address a different endpoint entirely, and an unencoded
/// `&` in a toolkit would forge a query parameter.
pub(super) fn encode(value: &str) -> String {
    percent_encoding::utf8_percent_encode(value.trim(), UNRESERVED_KEPT).to_string()
}

/// Encode one value that becomes a whole path segment.
///
/// `.` is unreserved, so `encode` keeps it, and an id of `.` or `..` would
/// become a dot segment that URL parsers resolve: `/connected_accounts/..`
/// addresses the collection, not an account. Escaping it does not help either,
/// since parsers treat `%2e%2e` as `..` too. An empty value would drop the
/// segment the same way. So these are refused before any request is built.
pub(super) fn segment(value: &str, what: &str) -> crate::Result<String> {
    let trimmed = value.trim();
    if trimmed.chars().all(|c| c == '.') {
        return Err(crate::Error::InvalidArguments {
            tool: what.to_string(),
            message: format!("`{trimmed}` is not a valid path segment"),
        });
    }
    Ok(encode(trimmed))
}

/// Join non-empty, trimmed, encoded values for a comma-separated parameter.
///
/// Returns `None` when nothing survives trimming, so the caller omits the
/// parameter rather than sending an empty one — which several endpoints read as
/// "match nothing" instead of "no filter".
pub(super) fn comma_joined(values: &[String]) -> Option<String> {
    let joined = values
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(encode)
        .collect::<Vec<_>>()
        .join(",");
    (!joined.is_empty()).then_some(joined)
}

#[cfg(test)]
#[path = "url_tests.rs"]
mod test;
