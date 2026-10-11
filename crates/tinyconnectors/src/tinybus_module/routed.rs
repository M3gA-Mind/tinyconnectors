//! Stateless dispatch bound to one immutable local client for the entire call.
use super::{ComposioClient, RouteConfig, TinyBusResult, to_bus_error};
use tinyconnectors_bus::{
    ComposioCallRoute, ComposioReadOperation, ComposioRoutedReadReply, ComposioRoutedReadRequest,
};

pub(super) async fn read(
    request: ComposioRoutedReadRequest,
) -> TinyBusResult<ComposioRoutedReadReply> {
    use ComposioReadOperation as Op;
    use ComposioRoutedReadReply as Reply;

    let config = match request.route {
        ComposioCallRoute::Proxy {
            base_url,
            auth_token,
            timezone,
            transport,
        } => {
            if auth_token.trim().is_empty() || base_url.trim().is_empty() {
                return Err(tinybus::Error::failed(
                    "routed read requires a nonempty proxy endpoint and bearer credential",
                ));
            }
            RouteConfig::Proxy {
                base_url,
                auth_token,
                timezone,
                transport,
                state_dir: None,
            }
        }
        ComposioCallRoute::Direct { credential } => {
            if credential.api_key.trim().is_empty()
                || credential
                    .base_url
                    .as_ref()
                    .is_some_and(|url| url.trim().is_empty())
            {
                return Err(tinybus::Error::failed(
                    "routed read requires a nonempty direct key and endpoint",
                ));
            }
            RouteConfig::Direct {
                api_key: credential.api_key,
                entity_id: credential.entity_id,
                base_url: credential.base_url,
                transport: credential.transport,
                state_dir: None,
            }
        }
    };
    let client = ComposioClient::new(config.into_route().map_err(|error| match error {
        // The legacy error includes the rejected URL, which can itself contain
        // credentials. This confidential route must not copy it into a reply.
        crate::Error::InsecureBaseUrl { reason, .. } => {
            tinybus::Error::failed(format!("refusing credential-bearing endpoint: {reason}"))
        }
        other => to_bus_error(&other),
    })?);
    match request.operation {
        Op::ListToolkits => client.list_toolkits().await.map(Reply::ListToolkits),
        Op::ListConnections => client.list_connections().await.map(Reply::ListConnections),
        Op::ListGithubRepos(request) => client
            .list_github_repos(request.connection_id.as_deref())
            .await
            .map(Reply::ListGithubRepos),
        Op::ListAvailableTriggers(request) => client
            .list_available_triggers(&request.toolkit, request.connection_id.as_deref())
            .await
            .map(Reply::ListAvailableTriggers),
        Op::ListTriggers(request) => client
            .list_triggers(request.toolkit.as_deref())
            .await
            .map(Reply::ListTriggers),
    }
    .map_err(|error| to_bus_error(&error))
}
