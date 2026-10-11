//! Required routes and tagged read request/reply vocabulary.

use super::super::{
    ComposioActiveTriggersResponse, ComposioAvailableTriggersResponse, ComposioConnectionsResponse,
    ComposioDirectCredential, ComposioGithubReposResponse, ComposioListAvailableTriggersRequest,
    ComposioListGithubReposRequest, ComposioListTriggersRequest, ComposioToolkitsResponse,
    ComposioTransportConfig,
};
use serde::{Deserialize, Serialize};

/// A required immutable route; credentials travel only over confidential calls.
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "route", rename_all = "snake_case", deny_unknown_fields)]
pub enum ComposioCallRoute {
    /// Read through the host's authenticated backend.
    Proxy {
        /// Backend base URL; HTTPS or loopback only.
        base_url: String,
        /// Host-admitted bearer credential, never returned or logged.
        auth_token: String,
        /// Optional IANA timezone.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        timezone: Option<String>,
        /// Host-selected network/TLS policy.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        transport: Option<ComposioTransportConfig>,
    },
    /// Direct read with the host's own Composio key.
    Direct {
        /// Credential and endpoint/network description for this call only.
        credential: ComposioDirectCredential,
    },
}

/// Only reads whose implementation needs no stored policy or provider state.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    tag = "operation",
    content = "request",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum ComposioReadOperation {
    /// Unfiltered connectable toolkit catalog.
    ListToolkits,
    /// Connected accounts visible to this route.
    ListConnections,
    /// GitHub repositories visible to the specified connection.
    ListGithubRepos(ComposioListGithubReposRequest),
    /// Available triggers; direct routes retain their existing refusal.
    ListAvailableTriggers(ComposioListAvailableTriggersRequest),
    /// Active triggers; direct routes retain their existing refusal.
    ListTriggers(ComposioListTriggersRequest),
}

/// One confidential read whose route is neither read from nor written to Configure.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComposioRoutedReadRequest {
    /// Required host-admitted immutable route.
    pub route: ComposioCallRoute,
    /// Stateless read, excluding preferences, execution, profiles and archives.
    pub operation: ComposioReadOperation,
}

/// The matching typed envelope; route credentials never appear in this vocabulary.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "operation", content = "result", rename_all = "snake_case")]
pub enum ComposioRoutedReadReply {
    /// Toolkit catalog reply.
    ListToolkits(ComposioToolkitsResponse),
    /// Connection list reply.
    ListConnections(ComposioConnectionsResponse),
    /// GitHub repository reply.
    ListGithubRepos(ComposioGithubReposResponse),
    /// Available trigger reply.
    ListAvailableTriggers(ComposioAvailableTriggersResponse),
    /// Active trigger reply.
    ListTriggers(ComposioActiveTriggersResponse),
}

impl std::fmt::Debug for ComposioCallRoute {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Proxy { .. } => f.write_str("ComposioCallRoute::Proxy(<redacted>)"),
            Self::Direct { .. } => f.write_str("ComposioCallRoute::Direct(<redacted>)"),
        }
    }
}
