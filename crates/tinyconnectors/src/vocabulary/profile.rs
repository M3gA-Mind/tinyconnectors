//! Compiled identifier normalization and prompt rendering.

use std::fmt::Write;

use tinyconnectors_bus::vocabulary::{ConnectedIdentity, IdentityKind};

/// Matching metadata evaluated inside the module.
pub(crate) trait IdentityMetadata {
    fn confidence(self) -> f64;
    fn is_matchable(&self) -> bool;
}
impl IdentityMetadata for IdentityKind {
    /// Confidence the matcher records on a row of this kind.
    ///
    /// Hard kinds auto-promote a chunk to "is self"; weak kinds require
    /// corroboration. A display name is deliberately low — two people share a
    /// name far more often than they share a user id.
    fn confidence(self) -> f64 {
        match self {
            Self::UserId | Self::Phone => 1.00,
            Self::Email => 0.95,
            Self::Handle => 0.70,
            Self::DisplayName => 0.40,
            Self::AvatarUrl | Self::ProfileUrl => 0.50,
        }
    }

    /// Whether this kind is a real identity signal worth running through the
    /// matcher, as opposed to a UI-only field.
    fn is_matchable(&self) -> bool {
        matches!(
            self,
            Self::UserId | Self::Email | Self::Handle | Self::Phone | Self::DisplayName
        )
    }
}

/// Canonicalize a raw identifier for storage and lookup.
///
/// The same routine runs on the entity side at match time, so equality of
/// canonical forms is the matcher's only test. Returns `None` for an empty or
/// whitespace-only value — storing one would match every chunk that happened to
/// carry a blank sender.
pub(crate) fn canonicalize(kind: IdentityKind, raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(match kind {
        IdentityKind::Email => trimmed.to_lowercase(),
        IdentityKind::Handle => trimmed.trim_start_matches('@').to_lowercase(),
        IdentityKind::Phone => trimmed
            .chars()
            .filter(|c| c.is_ascii_digit() || *c == '+')
            .collect(),
        IdentityKind::DisplayName => trimmed.split_whitespace().collect::<Vec<_>>().join(" "),
        IdentityKind::UserId | IdentityKind::AvatarUrl | IdentityKind::ProfileUrl => {
            trimmed.to_string()
        }
    })
}

/// Render a compact prompt section for a set of identities.
///
/// Skips `user_id` (not human-readable) and prefixes a handle with `@`. Returns
/// an empty string — rather than a bare heading — when there is nothing worth
/// showing, so a caller can concatenate the result unconditionally.
///
/// Every value is flattened onto one line and has `|` replaced before it is
/// joined with `|` separators: an identifier is user-controlled text arriving
/// from a third-party provider, and a display name containing a newline would
/// otherwise let it forge additional prompt lines.
pub(crate) fn render_connected_identities_section(identities: &[ConnectedIdentity]) -> String {
    if identities.is_empty() {
        return String::new();
    }
    let mut out = String::from("## Connected Identities\n\n");
    for id in identities {
        let mut fields = Vec::<String>::new();
        if let Some(v) = id.display_name.as_deref() {
            let v = sanitize_prompt_value(v);
            if !v.is_empty() {
                fields.push(v);
            }
        }
        if let Some(v) = id.email.as_deref() {
            let v = sanitize_prompt_value(v);
            if !v.is_empty() {
                fields.push(v);
            }
        }
        if let Some(v) = id.handle.as_deref() {
            let v = sanitize_prompt_value(v);
            if !v.is_empty() {
                fields.push(format!("@{v}"));
            }
        }
        if let Some(v) = id.profile_url.as_deref() {
            let v = sanitize_prompt_value(v);
            if !v.is_empty() {
                fields.push(v);
            }
        }
        if fields.is_empty() {
            continue;
        }
        let identifier = sanitize_prompt_value(&id.identifier);
        let _ = writeln!(
            out,
            "- {} ({}): {}",
            title_case(&id.source),
            identifier,
            fields.join(" | ")
        );
    }
    if out.trim() == "## Connected Identities" {
        return String::new();
    }
    out
}

/// Normalize a raw toolkit slug or connection id into the form facet keys are
/// stored under.
///
/// Lowercases, replaces every character outside `[a-z0-9_-]` with `_`, and
/// trims leading and trailing underscores. Writer and reader must both call
/// this: a caller passing a raw connection id to a delete would otherwise match
/// no rows, and the disconnected account would keep being treated as the user.
pub(crate) fn normalize_connection_identifier(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for ch in raw.chars() {
        let lower = ch.to_ascii_lowercase();
        if lower.is_ascii_alphanumeric() || lower == '-' || lower == '_' {
            out.push(lower);
        } else {
            out.push('_');
        }
    }
    out.trim_matches('_').to_string()
}

fn title_case(raw: &str) -> String {
    let mut chars = raw.chars();
    match chars.next() {
        Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
        None => String::new(),
    }
}

fn sanitize_prompt_value(raw: &str) -> String {
    let replaced = raw.replace(['\n', '\r', '\t'], " ").replace('|', "/");
    replaced.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
#[path = "profile_tests.rs"]
mod tests;

#[cfg(test)]
use tinyconnectors_bus::vocabulary::ProviderUserProfile;
