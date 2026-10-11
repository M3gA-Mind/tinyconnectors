//! Serialized scope categories and display preferences.

use serde::{Deserialize, Serialize};

/// Classification of how invasive an action is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolScope {
    /// Pure reads — `GET` / `FETCH` / `LIST` / `SEARCH` / `GET_PROFILE`.
    Read,
    /// Side-effectful actions that create or mutate user data —
    /// `SEND` / `CREATE` / `UPDATE` / `REPLY` / `APPEND`.
    Write,
    /// Destructive or permission-changing actions — `DELETE` / `TRASH` /
    /// `REMOVE` / `MODIFY_LABELS` / `SHARE`.
    Admin,
}

impl ToolScope {
    /// Stable lowercase tag, matching the serde representation.
    pub fn as_str(self) -> &'static str {
        match self {
            ToolScope::Read => "read",
            ToolScope::Write => "write",
            ToolScope::Admin => "admin",
        }
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
