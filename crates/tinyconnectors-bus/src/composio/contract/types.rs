//! Payloads for live tool contracts, output probes, and the connected
//! integration overview.

use serde::{Deserialize, Serialize};

use crate::vocabulary::ToolScope;

/// One action's live, ground-truth contract, as the connector backend
/// publishes it.
///
/// `input_schema` and `output_schema` are verbatim from the backend listing.
/// `output_fields` and `primary_array_path` are derived from `output_schema`
/// unless a probe of the real response has since replaced them. `is_curated`
/// is a ranking signal only: an uncurated real action still has a contract.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComposioToolContract {
    /// The action slug, e.g. `GMAIL_SEND_EMAIL`.
    pub slug: String,
    /// The lowercase toolkit slug the action belongs to.
    pub toolkit: String,
    /// The description the backend publishes, when it publishes one.
    pub description: Option<String>,
    /// Required top-level input argument names. Empty means no required
    /// arguments, not an unknown schema.
    pub required_args: Vec<String>,
    /// The full input JSON Schema, verbatim.
    pub input_schema: Option<serde_json::Value>,
    /// Top-level field names of the tool's payload (the value under the
    /// execute response's `data`, not the response envelope).
    pub output_fields: Vec<String>,
    /// The full output JSON Schema. `None` means unknown, not empty.
    pub output_schema: Option<serde_json::Value>,
    /// Dotted path, relative to the engine envelope's `json` field, to the
    /// first array in the real output. Already includes the leading `data.`
    /// segment of the execute envelope.
    pub primary_array_path: Option<String>,
    /// Whether the action is also in the hand-curated catalog for its toolkit.
    pub is_curated: bool,
}

/// Asks for one action's contract.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComposioGetToolContractRequest {
    /// The action slug. Matched case-insensitively.
    pub slug: String,
}

/// The answer to [`ComposioGetToolContractRequest`].
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ComposioGetToolContractResponse {
    /// The contract, or `None` when the toolkit is real but publishes no such
    /// action, or the slug names no toolkit. A fetch failure is a member
    /// error, never `None`.
    pub contract: Option<ComposioToolContract>,
}

/// Asks for every contract of one toolkit.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComposioListToolContractsRequest {
    /// The toolkit slug. Trimmed and lowercased by the module.
    pub toolkit: String,
}

/// The answer to [`ComposioListToolContractsRequest`].
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ComposioListToolContractsResponse {
    /// The normalized toolkit slug.
    pub toolkit: String,
    /// Every action the backend publishes for the toolkit, connected or not.
    /// Empty means the toolkit publishes none.
    pub contracts: Vec<ComposioToolContract>,
}

/// Asks the module to run one read-only action and describe the real shape of
/// what comes back.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ComposioProbeToolOutputRequest {
    /// The action slug. It must be a confirmed read-scope action.
    pub slug: String,
    /// Arguments forwarded verbatim to the real call. `null` means `{}`.
    #[serde(default)]
    pub args: serde_json::Value,
}

/// The observed output shape of one real action call.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ComposioProbedOutputSample {
    /// Dotted path to the first array in the real response, relative to the
    /// engine envelope's `json` field. `None` means the response has no array.
    pub primary_array_path: Option<String>,
    /// Top-level field names of the real response's `data` payload.
    pub output_fields: Vec<String>,
    /// The full envelope-shaped sample. Returned to this one caller and never
    /// cached by the module, because it can hold the user's private data.
    pub sample: serde_json::Value,
}

/// Which side of the backend an overview is assembled from.
///
/// The module reaches one route at a time. A request whose mode disagrees with
/// the live route is refused rather than answered from the wrong tenant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ComposioConnectedIntegrationsMode {
    /// The hosted backend: its allowlist defines the toolkits, and a tool
    /// listing failure fails the overview.
    Backend,
    /// The user's own tenant: the active connections define the toolkits, and
    /// a tool listing failure leaves tools empty.
    Direct,
}

/// Asks for the connected-integration overview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComposioListConnectedIntegrationsRequest {
    /// The route the overview is built from.
    pub mode: ComposioConnectedIntegrationsMode,
}

/// One action a user can call on a connected toolkit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComposioConnectedIntegrationTool {
    /// The action slug.
    pub name: String,
    /// One-line description, empty when the backend gives none.
    pub description: String,
    /// The action's parameter schema, when the backend supplies one.
    pub parameters: Option<serde_json::Value>,
}

/// A curated action the user's scope preference hides.
///
/// There is deliberately no parameter schema: a gated action can be described
/// but never called.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComposioGatedIntegrationTool {
    /// The action slug.
    pub name: String,
    /// One-line description, empty when the backend gives none.
    pub description: String,
    /// The scope the user must enable for the action to become callable.
    pub required_scope: ToolScope,
}

/// One active connection of a toolkit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComposioIntegrationConnection {
    /// The backend connection id.
    pub connection_id: String,
    /// The first non-empty of account email, workspace and username.
    pub label: Option<String>,
    /// Whether this is the oldest active connection of the toolkit.
    pub is_default: bool,
}

/// One toolkit in the overview.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComposioConnectedIntegration {
    /// The lowercase toolkit slug.
    pub toolkit: String,
    /// One-line capability description. The backend catalog's wins over the
    /// compiled table.
    pub description: String,
    /// Callable actions. Empty unless `connected`.
    pub tools: Vec<ComposioConnectedIntegrationTool>,
    /// Curated actions hidden by the user's scope preference. Empty unless
    /// `connected`.
    pub gated_tools: Vec<ComposioGatedIntegrationTool>,
    /// Whether the user has an active connection for the toolkit.
    pub connected: bool,
    /// Active connections, oldest first. Empty unless `connected`.
    pub connections: Vec<ComposioIntegrationConnection>,
    /// The most actionable non-active connection status
    /// (`EXPIRED`, then `FAILED`/`ERROR`, then in-flight ones), when the
    /// toolkit has no active connection but has rows.
    pub non_active_status: Option<String>,
}

/// The answer to [`ComposioListConnectedIntegrationsRequest`], sorted by
/// toolkit slug.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ComposioConnectedIntegrationsResponse {
    /// One entry per toolkit.
    pub integrations: Vec<ComposioConnectedIntegration>,
}
