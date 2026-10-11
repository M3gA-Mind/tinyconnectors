//! Serialized scope categories and display preferences.

use serde::{Deserialize, Serialize};

/// How invasive an action is.
///
/// Ordered by consequence, which is what makes a user preference of "read only"
/// expressible as a threshold rather than a set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolScope {
    /// Pure reads — get, fetch, list, search.
    Read,
    /// Creates or changes the user's data — send, create, update, reply.
    Write,
    /// Destructive or permission-changing — delete, trash, share, revoke.
    Admin,
}

impl ToolScope {
    /// The stable wire name used in preferences and catalogs.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Write => "write",
            Self::Admin => "admin",
        }
    }

    /// Whether this scope is permitted when the user has allowed up to `limit`.
    ///
    /// The ordering is the point: allowing write allows read, because a user
    /// who consented to changes has necessarily consented to looking.
    #[must_use]
    pub fn is_allowed_by(self, limit: Self) -> bool {
        self <= limit
    }
}

/// Per-toolkit display preference: reads and writes on, admin off by default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserScopePref {
    /// Whether the agent may call [`ToolScope::Read`] actions.
    #[serde(default = "default_true")]
    pub read: bool,
    /// Whether the agent may call [`ToolScope::Write`] actions.
    #[serde(default = "default_true")]
    pub write: bool,
    /// Whether the agent may call [`ToolScope::Admin`] actions.
    #[serde(default)]
    pub admin: bool,
}

fn default_true() -> bool {
    true
}

impl Default for UserScopePref {
    fn default() -> Self {
        Self {
            read: true,
            write: true,
            admin: false,
        }
    }
}

impl UserScopePref {
    /// Whether the supplied scope is permitted by these independent flags.
    #[must_use]
    pub fn allows(self, scope: ToolScope) -> bool {
        match scope {
            ToolScope::Read => self.read,
            ToolScope::Write => self.write,
            ToolScope::Admin => self.admin,
        }
    }

    /// Durable preference row key: toolkit trimmed and lowercased.
    #[must_use]
    pub fn key(toolkit: &str) -> String {
        toolkit.trim().to_ascii_lowercase()
    }
}
