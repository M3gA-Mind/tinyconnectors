//! Connected account wire shapes and durable identifier tags.

use serde::{Deserialize, Serialize};

/// The connected account's identity, as far as a toolkit reports it.
///
/// Every field is optional because the toolkits disagree about which they have:
/// Gmail knows an email, Slack knows a workspace and a display name, GitHub
/// knows a login. A UI picking a label falls back through them in that order.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderUserProfile {
    /// Toolkit the profile is for.
    pub toolkit: String,
    /// Connection the profile was read through.
    #[serde(default)]
    pub connection_id: Option<String>,
    /// Human name, when the provider reports one.
    #[serde(default)]
    pub display_name: Option<String>,
    /// Account email.
    #[serde(default)]
    pub email: Option<String>,
    /// Login or handle.
    #[serde(default)]
    pub username: Option<String>,
    /// Avatar image URL.
    #[serde(default)]
    pub avatar_url: Option<String>,
    /// Link to the account on the provider.
    #[serde(default)]
    pub profile_url: Option<String>,
    /// Anything toolkit-specific.
    ///
    /// Here so a new toolkit with an interesting field does not require
    /// widening this shape — and every consumer of it — to carry something one
    /// provider reports.
    #[serde(default)]
    pub extras: serde_json::Value,
}

impl From<crate::composio::ComposioUserProfile> for ProviderUserProfile {
    /// The member reply's profile as the host's identity shape: the same fields,
    /// moved. Replaces a serde round-trip between the two shapes, which could
    /// only fail once they drifted apart.
    fn from(profile: crate::composio::ComposioUserProfile) -> Self {
        Self {
            toolkit: profile.toolkit,
            connection_id: profile.connection_id,
            display_name: profile.display_name,
            email: profile.email,
            username: profile.username,
            avatar_url: profile.avatar_url,
            profile_url: profile.profile_url,
            extras: profile.extras,
        }
    }
}

impl From<ProviderUserProfile> for crate::composio::ComposioUserProfile {
    fn from(profile: ProviderUserProfile) -> Self {
        Self {
            toolkit: profile.toolkit,
            connection_id: profile.connection_id,
            display_name: profile.display_name,
            email: profile.email,
            username: profile.username,
            avatar_url: profile.avatar_url,
            profile_url: profile.profile_url,
            extras: profile.extras,
        }
    }
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
