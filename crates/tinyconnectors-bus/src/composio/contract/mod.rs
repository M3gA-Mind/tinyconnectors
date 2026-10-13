//! Live tool contracts, output probes, and the connected-integration overview.
//!
//! The payloads of three members that turn the connector backend's listings
//! into something a host can ground an agent on: [`ComposioToolContract`]
//! (what an action accepts and returns), [`ComposioProbedOutputSample`] (what
//! it really returned once), and [`ComposioConnectedIntegration`] (which
//! toolkits the user can use right now). The module owns the fetching, the
//! caching and the shaping; the host owns policy on top of the answers.

mod types;

pub use types::{
    ComposioConnectedIntegration, ComposioConnectedIntegrationTool,
    ComposioConnectedIntegrationsMode, ComposioConnectedIntegrationsResponse,
    ComposioGatedIntegrationTool, ComposioGetToolContractRequest, ComposioGetToolContractResponse,
    ComposioIntegrationConnection, ComposioListConnectedIntegrationsRequest,
    ComposioListToolContractsRequest, ComposioListToolContractsResponse,
    ComposioProbeToolOutputRequest, ComposioProbedOutputSample, ComposioToolContract,
};

#[cfg(test)]
#[path = "mod_tests.rs"]
mod test;
