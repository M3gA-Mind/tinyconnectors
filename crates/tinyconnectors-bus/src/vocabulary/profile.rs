//! Connected account wire shapes and durable identifier tags.

use serde::{Deserialize, Serialize};

/// Normalized profile returned by a provider.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProviderUserProfile {
    /// Composio toolkit slug the profile was fetched from, e.g. `"gmail"`.
    pub toolkit: String,
    /// The connection the profile belongs to; `None` on toolkit-wide fetches.
    pub connection_id: Option<String>,
    /// Human display label, when the provider exposes one.
    pub display_name: Option<String>,
    /// Primary email address on the connected account.
    pub email: Option<String>,
    /// Platform username or screen name, without any leading `@`.
    pub username: Option<String>,
    /// URL of the account's avatar image.
    pub avatar_url: Option<String>,
    /// URL of the account's public profile page.
    pub profile_url: Option<String>,
    /// Provider-specific extras (raw JSON object).
    #[serde(default)]
    pub extras: serde_json::Value,
}

/// Durable identifier dimension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentityKind {
    /// Platform-canonical immutable id — a Slack `U123ABC`, a Notion UUID.
    UserId,
    /// Email address.
    Email,
    /// An `@`-style screen name, canonicalised without the leading `@`.
    Handle,
    /// E.164 phone number.
    Phone,
    /// Human display label. A weak signal — never auto-promotes to "is self".
    DisplayName,
    /// Not for matching; kept for UI and prompt rendering.
    AvatarUrl,
    /// Not for matching; kept for UI and prompt rendering.
    ProfileUrl,
}

impl IdentityKind {
    /// The stored key segment for this kind.
    ///
    /// Durable: it is the last segment of a persisted facet key, so renaming a
    /// variant's string orphans every row filed under the old one.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::UserId => "user_id",
            Self::Email => "email",
            Self::Handle => "handle",
            Self::Phone => "phone",
            Self::DisplayName => "display_name",
            Self::AvatarUrl => "avatar_url",
            Self::ProfileUrl => "profile_url",
        }
    }

    /// Parse a stored key segment back into a kind.
    ///
    /// Returns `None` for anything unrecognised — including the legacy
    /// `username` segment written before the identifier rewrite. Callers skip
    /// those rows rather than failing the load, so one stale row cannot make a
    /// user's whole identity set unreadable.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "user_id" => Self::UserId,
            "email" => Self::Email,
            "handle" => Self::Handle,
            "phone" => Self::Phone,
            "display_name" => Self::DisplayName,
            "avatar_url" => Self::AvatarUrl,
            "profile_url" => Self::ProfileUrl,
            _ => return None,
        })
    }
}

/// Identifiers for one source and connection.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ConnectedIdentity {
    /// Toolkit slug the identity came from, normalised.
    pub source: String,
    /// Connection identifier, normalised — see
    /// the module normalization operation.
    pub identifier: String,
    /// Human display label, when one was stored.
    pub display_name: Option<String>,
    /// Canonicalised email address.
    pub email: Option<String>,
    /// Canonicalised screen name, without the leading `@`.
    pub handle: Option<String>,
    /// Canonicalised phone number.
    pub phone: Option<String>,
    /// Platform-canonical immutable id.
    pub user_id: Option<String>,
    /// Avatar image URL.
    pub avatar_url: Option<String>,
    /// Public profile URL.
    pub profile_url: Option<String>,
}
