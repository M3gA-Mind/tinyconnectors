//! Portable connector vocabulary payloads, evaluated by the compiled module.
//!
//! Tables and normalization are deliberately absent from this contract crate.
//! Display preferences do not replace the module's persisted execution policy.

pub mod profile;
pub mod scopes;
pub mod tasks;

pub use profile::{ConnectedIdentity, IdentityKind, ProviderUserProfile};
pub use scopes::{ToolScope, UserScopePref};
pub use tasks::{GithubFetchMode, NormalizedTask, TaskContainer, TaskFetchFilter, TaskKind};

use serde::{Deserialize, Serialize};

/// Toolkit names whose compiled vocabulary the caller wants.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct VocabularyRequest {
    /// Requested toolkit names, preserved in response order.
    pub toolkits: Vec<String>,
}

/// One owned curated action entry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VocabularyAction {
    /// Exact provider action slug.
    pub slug: String,
    /// Curated classification.
    pub scope: ToolScope,
}

/// Vocabulary for a requested toolkit, including unknown toolkits.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolkitVocabulary {
    /// Toolkit name supplied by the caller.
    pub toolkit: String,
    /// Existing capability description, including the unknown-name fallback.
    pub description: String,
    /// Whether the extracted vocabulary names a native identity provider.
    pub native_provider: bool,
    /// Curated catalog in declaration order; absent for uncurated toolkits.
    pub actions: Option<Vec<VocabularyAction>>,
}

/// Compiled catalog data; independent of credentials and account state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VocabularyResponse {
    /// Requested toolkit results in request order.
    pub toolkits: Vec<ToolkitVocabulary>,
    /// Complete, sorted catalog-ready list, distinct from provider availability.
    pub agent_ready_toolkits: Vec<String>,
}

/// Explicit fallback profile for an action absent from the curated catalog.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UnknownScopeProfile {
    /// Legacy substring matching; retained for consumers with that behavior.
    SubstringCompatibility,
    /// Classifies the leading action verb, as the provider execution path does.
    LeadingVerb,
}

/// One action metadata query; this cannot authorize execution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActionVocabularyRequest {
    /// Provider action name.
    pub slug: String,
    /// Supplied display preference; persisted execution preferences are separate.
    pub preference: UserScopePref,
    /// Explicit classifier used only when there is no curated classification.
    pub unknown_scope_profile: UnknownScopeProfile,
    /// Requested slug to compare against the first slug as a catalog candidate.
    /// Alias expansion applies to this requested slug, preserving existing matching.
    pub compare_with: Option<String>,
}

/// Classification and naming metadata for one requested action.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActionVocabularyResponse {
    /// Toolkit inferred from the action slug.
    pub toolkit: Option<String>,
    /// Canonical alias replacement, absent when no alias exists.
    pub canonical_slug: Option<String>,
    /// Curated classification, absent for actions not in a catalog.
    pub curated_scope: Option<ToolScope>,
    /// Curated classification or explicitly selected fallback.
    pub scope: ToolScope,
    /// Whether the catalog and supplied display preference expose the action.
    pub visible: bool,
    /// Alias comparison result when a second slug was supplied.
    pub matches: Option<bool>,
}

/// One identifier to canonicalize.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IdentityValueRequest {
    /// Durable identifier dimension.
    pub kind: IdentityKind,
    /// Raw provider value.
    pub value: String,
}

/// Batch identity processing with no persistence or network calls.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct NormalizeIdentitiesRequest {
    /// Values to canonicalize in request order.
    pub values: Vec<IdentityValueRequest>,
    /// Connection identifiers to normalize in request order.
    pub connection_identifiers: Vec<String>,
    /// Existing identities whose exact prompt section should be rendered.
    pub identities: Vec<ConnectedIdentity>,
}

/// Canonical value and matching metadata for an identifier.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IdentityValueResponse {
    /// Canonical form, absent for a blank input.
    pub value: Option<String>,
    /// Confidence associated with the identifier dimension.
    pub confidence: f64,
    /// Whether the dimension participates in identity matching.
    pub matchable: bool,
}

/// Computed identity metadata and exact prompt output.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NormalizeIdentitiesResponse {
    /// Canonical values in request order.
    pub values: Vec<IdentityValueResponse>,
    /// Durable normalized connection identifiers.
    pub connection_identifiers: Vec<String>,
    /// Compiled renderer output, empty when no identity is renderable.
    pub prompt_section: String,
}

/// Prepared query vocabulary; this does not fetch tasks.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PreparedTaskFilter {
    /// Unchanged query envelope, including all provider-native extra fields.
    pub filter: TaskFetchFilter,
    /// Effective item cap: 25 when omitted, otherwise the explicit value.
    pub effective_max: usize,
}

#[cfg(test)]
#[doc = "Wire compatibility tests for portable connector payloads."]
#[path = "mod_tests.rs"]
mod tests;
