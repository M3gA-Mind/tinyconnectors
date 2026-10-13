//! Which toolkits a host offers its agent.
//!
//! A host policy the connector module enforces when it lists: an operator can
//! narrow the integrations an agent is shown without touching the user's
//! connections or scope preferences. The host supplies it in the module's load
//! configuration (`offered_toolkits`).

use serde::{Deserialize, Serialize};

/// Allow and deny lists of toolkit slugs.
///
/// An empty `allow` offers every toolkit; a non-empty one offers only those
/// named. `deny` always wins. Names compare case-insensitively and ignore `_`
/// and `-`, so `google_calendar` and `googlecalendar` are the same toolkit.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct OfferedToolkits {
    /// Only these toolkits are offered. Empty means no restriction.
    #[serde(default)]
    pub allow: Vec<String>,
    /// These toolkits are never offered, even when `allow` names them.
    #[serde(default)]
    pub deny: Vec<String>,
}

fn key(name: &str) -> String {
    name.chars()
        .filter(|c| *c != '_' && *c != '-' && !c.is_whitespace())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

impl OfferedToolkits {
    /// Whether neither list narrows anything.
    #[must_use]
    pub fn is_unrestricted(&self) -> bool {
        self.allow.is_empty() && self.deny.is_empty()
    }

    /// Whether `toolkit` is offered.
    #[must_use]
    pub fn permits(&self, toolkit: &str) -> bool {
        let wanted = key(toolkit);
        if self.deny.iter().any(|name| key(name) == wanted) {
            return false;
        }
        self.allow.is_empty() || self.allow.iter().any(|name| key(name) == wanted)
    }
}

#[cfg(test)]
#[doc = "Tests for the offered-toolkit policy."]
#[path = "offered_tests.rs"]
mod tests;
