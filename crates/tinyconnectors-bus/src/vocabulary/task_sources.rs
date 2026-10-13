//! The portable half of the host's task-source configuration.
//!
//! A task source is a user-configured pull of work items from an external tool.
//! Which provider it pulls from, how it filters, why a fetch ran and where the
//! results go are plain data the connector vocabulary shares with every host.
//! Scheduling, persistence and the todo board stay with the host.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{GithubFetchMode, TaskFetchFilter};

/// External tool a task source pulls from. The string form matches the Composio
/// toolkit slug, so it keys directly into the provider registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderSlug {
    /// `GitHub` issues and pull requests.
    Github,
    /// Notion database rows.
    Notion,
    /// Linear issues.
    Linear,
    /// `ClickUp` tasks.
    Clickup,
}

impl ProviderSlug {
    /// The toolkit slug.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Github => "github",
            Self::Notion => "notion",
            Self::Linear => "linear",
            Self::Clickup => "clickup",
        }
    }

    /// Parse a toolkit slug. Case-insensitive.
    ///
    /// # Errors
    ///
    /// A message naming the accepted providers when `raw` is none of them.
    pub fn parse(raw: &str) -> Result<Self, String> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "github" => Ok(Self::Github),
            "notion" => Ok(Self::Notion),
            "linear" => Ok(Self::Linear),
            "clickup" => Ok(Self::Clickup),
            other => Err(format!(
                "unknown task source provider '{other}' (expected github|notion|linear|clickup)"
            )),
        }
    }
}

/// Per-provider, user-configured filter. Tagged by `provider` on the wire so a
/// frontend can render typed pickers; each variant carries a free-form `extra`
/// object as an advanced escape hatch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "provider", rename_all = "snake_case")]
pub enum FilterSpec {
    /// `GitHub` issue and pull-request filter.
    Github {
        /// `owner/name` repository scope.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        repo: Option<String>,
        /// Label filter.
        #[serde(default)]
        labels: Vec<String>,
        /// Only items assigned to the authenticated user.
        #[serde(default)]
        assignee_is_me: bool,
        /// Issue state.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        state: Option<String>,
        /// How to fetch: Composio connection, local `gh`/REST, or `auto`.
        #[serde(default)]
        fetch_mode: GithubFetchMode,
        /// Provider-native filter fragment.
        #[serde(default)]
        extra: Value,
    },
    /// Notion database filter.
    Notion {
        /// Database (board) id.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        database_id: Option<String>,
        /// Only rows assigned to the authenticated user.
        #[serde(default)]
        assigned_to_me: bool,
        /// Status property value.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        status: Option<String>,
        /// Provider-native filter fragment.
        #[serde(default)]
        extra: Value,
    },
    /// Linear filter.
    Linear {
        /// Team id.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        team_id: Option<String>,
        /// Only issues assigned to the authenticated user.
        #[serde(default)]
        assignee_is_me: bool,
        /// Workflow state.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        state: Option<String>,
        /// Provider-native filter fragment.
        #[serde(default)]
        extra: Value,
    },
    /// `ClickUp` filter.
    Clickup {
        /// Workspace (team) id.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        team_id: Option<String>,
        /// List id.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        list_id: Option<String>,
        /// Only tasks assigned to the authenticated user.
        #[serde(default)]
        assignee_is_me: bool,
        /// Provider-native filter fragment.
        #[serde(default)]
        extra: Value,
    },
}

impl FilterSpec {
    /// The provider this filter targets; it must match the owning source's.
    #[must_use]
    pub fn provider(&self) -> ProviderSlug {
        match self {
            Self::Github { .. } => ProviderSlug::Github,
            Self::Notion { .. } => ProviderSlug::Notion,
            Self::Linear { .. } => ProviderSlug::Linear,
            Self::Clickup { .. } => ProviderSlug::Clickup,
        }
    }

    /// Flatten the per-provider variant into the shared [`TaskFetchFilter`]
    /// envelope and stamp the per-fetch cap `max`.
    ///
    /// Each provider reads only the fields relevant to its toolkit, so fields
    /// that do not apply to this variant stay unset.
    #[must_use]
    pub fn to_fetch_filter(&self, max: u32) -> TaskFetchFilter {
        match self {
            Self::Github {
                repo,
                labels,
                assignee_is_me,
                state,
                fetch_mode,
                extra,
            } => TaskFetchFilter {
                assignee_is_me: *assignee_is_me,
                github_fetch_mode: *fetch_mode,
                repo: repo.clone(),
                labels: labels.clone(),
                state: state.clone(),
                extra: extra.clone(),
                max,
                ..Default::default()
            },
            Self::Notion {
                database_id,
                assigned_to_me,
                status,
                extra,
            } => TaskFetchFilter {
                assignee_is_me: *assigned_to_me,
                database_id: database_id.clone(),
                status: status.clone(),
                extra: extra.clone(),
                max,
                ..Default::default()
            },
            Self::Linear {
                team_id,
                assignee_is_me,
                state,
                extra,
            } => TaskFetchFilter {
                assignee_is_me: *assignee_is_me,
                team_id: team_id.clone(),
                state: state.clone(),
                extra: extra.clone(),
                max,
                ..Default::default()
            },
            Self::Clickup {
                team_id,
                list_id,
                assignee_is_me,
                extra,
            } => TaskFetchFilter {
                assignee_is_me: *assignee_is_me,
                team_id: team_id.clone(),
                list_id: list_id.clone(),
                extra: extra.clone(),
                max,
                ..Default::default()
            },
        }
    }
}

/// How enriched tasks are routed once fetched.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceTarget {
    /// Append a todo card and dispatch a triage turn so an agent may start
    /// working immediately (triage still gates noise).
    #[default]
    AgentTodoProactive,
    /// Collect into the ingestion ledger only; never auto-start an agent turn.
    TodoOnly,
}

/// Why a fetch ran; mirrors the provider sync-reason semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FetchReason {
    /// First fetch right after an `OAuth` connection is created.
    ConnectionCreated,
    /// Periodic background poll.
    Periodic,
    /// Explicit user or RPC trigger.
    Manual,
}

impl FetchReason {
    /// The stable wire tag.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ConnectionCreated => "connection_created",
            Self::Periodic => "periodic",
            Self::Manual => "manual",
        }
    }
}

#[cfg(test)]
#[doc = "Wire tests for the portable task-source configuration."]
#[path = "task_sources_tests.rs"]
mod tests;
