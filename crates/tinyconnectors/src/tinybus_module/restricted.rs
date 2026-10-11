//! Immutable read-only module instance; no client slot, preferences or archive.
use super::{Connection, TinyBusResult, names, routed};
use tinyconnectors_bus::{
    ComposioAgentReadyToolkitsResponse, ComposioCapabilitiesResponse, ComposioRoutedReadReply,
    ComposioRoutedReadRequest, ComposioRuntimeMode, ComposioRuntimeModeResponse,
};

struct RestrictedService;

#[tinybus::interface(name = "ai.tinyhumans.connectors.Composio")]
impl RestrictedService {
    #[tinybus(confidential)]
    async fn routed_read(
        &self,
        request: ComposioRoutedReadRequest,
    ) -> TinyBusResult<ComposioRoutedReadReply> {
        routed::read(request).await
    }

    #[allow(
        clippy::unused_async,
        reason = "TinyBus signatures are async; mode is immutable"
    )]
    async fn runtime_mode(&self) -> TinyBusResult<ComposioRuntimeModeResponse> {
        Ok(ComposioRuntimeModeResponse {
            mode: ComposioRuntimeMode::RoutedReadOnly,
            contract_version: tinyconnectors_bus::CONTRACT_VERSION,
        })
    }

    #[allow(
        clippy::unused_async,
        reason = "TinyBus signatures are async; static registry only"
    )]
    async fn list_capabilities(&self) -> TinyBusResult<ComposioCapabilitiesResponse> {
        // No native provider methods are available in this instance.
        // Routed reads are admitted separately through the mode handshake.
        Ok(ComposioCapabilitiesResponse::default())
    }

    #[allow(
        clippy::unused_async,
        reason = "TinyBus signatures are async; static registry only"
    )]
    async fn list_agent_ready_toolkits(&self) -> TinyBusResult<ComposioAgentReadyToolkitsResponse> {
        Ok(ComposioAgentReadyToolkitsResponse {
            toolkits: crate::providers::default_registry().agent_ready_toolkits(),
        })
    }
}

pub(super) async fn setup(connection: Connection) -> TinyBusResult<()> {
    connection
        .serve_at(names::OBJECT_PATH.try_into()?, RestrictedService)
        .await?;
    connection.request_name(names::INTERFACE).await?;
    Ok(())
}

#[cfg(test)]
#[path = "restricted_tests.rs"]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests;
