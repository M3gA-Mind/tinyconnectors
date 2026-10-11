//! The live module mode, independent of any caller's route.
use serde::{Deserialize, Serialize};

/// Immutable module-instance initialization mode.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComposioRuntimeMode {
    /// Existing module-wide configuration and optional state behavior.
    #[default]
    Classic,
    /// Confidential routed reads and static capabilities; stateful calls refused.
    RoutedReadOnly,
}

/// Runtime admission handshake from the admitted module instance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComposioRuntimeModeResponse {
    /// The actual immutable live mode, rather than requested configuration.
    pub mode: ComposioRuntimeMode,
    /// The implementation's wire contract major and minor.
    pub contract_version: (u32, u32),
}
