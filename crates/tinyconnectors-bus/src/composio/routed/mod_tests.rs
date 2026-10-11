use super::*;
use serde_json::json;

#[test]
fn proxy_route_debug_does_not_reveal_credentials_or_private_endpoint() {
    let request = ComposioRoutedReadRequest {
        route: ComposioCallRoute::Proxy {
            base_url: "https://private.example".into(),
            auth_token: "secret-bearer".into(),
            timezone: None,
            transport: None,
        },
        operation: ComposioReadOperation::ListConnections,
    };
    let debug = format!("{request:?}");
    assert!(!debug.contains("secret-bearer"));
    assert!(!debug.contains("private.example"));
}

#[test]
fn required_routes_and_unknown_state_operations_are_rejected() {
    for value in [
        json!({"operation":{"operation":"list_connections"}}),
        json!({"route":{"route":"none"},"operation":{"operation":"list_connections"}}),
        json!({"route":{"route":"proxy","base_url":"https://example.com"},"operation":{"operation":"list_connections"}}),
        json!({"route":{"route":"proxy","base_url":"https://example.com","auth_token":"token"},"operation":{"operation":"execute","request":{}}}),
    ] {
        assert!(serde_json::from_value::<ComposioRoutedReadRequest>(value).is_err());
    }
}

#[test]
fn legacy_configuration_debug_also_redacts_route_credentials() {
    let request = crate::ComposioConfigureRequest::Proxy {
        base_url: "https://private.example".into(),
        auth_token: "secret-bearer".into(),
        timezone: None,
        transport: None,
    };
    assert!(!format!("{request:?}").contains("secret-bearer"));
    let direct = crate::ComposioConfigureRequest::Direct {
        api_key: "secret-key".into(),
        entity_id: Some("private-entity".into()),
        base_url: None,
        transport: None,
    };
    assert!(!format!("{direct:?}").contains("secret-key"));
    assert_eq!(
        format!("{:?}", crate::ComposioConfigureRequest::None),
        "ComposioConfigureRequest::None"
    );
}

#[test]
fn direct_route_debug_redacts_key_entity_and_proxy_credentials() {
    let request = ComposioCallRoute::Direct {
        credential: crate::ComposioDirectCredential {
            api_key: "secret-key".into(),
            entity_id: Some("private-entity".into()),
            base_url: Some("https://private.example".into()),
            transport: Some(crate::ComposioTransportConfig {
                proxy_url: Some("http://user:password@proxy".into()),
                ..Default::default()
            }),
        },
    };
    let debug = format!("{request:?}");
    for private in [
        "secret-key",
        "private-entity",
        "private.example",
        "password",
    ] {
        assert!(!debug.contains(private));
    }
}

#[test]
fn five_read_requests_and_replies_pin_their_tags_and_trigger_fields() {
    for value in [
        json!({"operation":"list_toolkits"}),
        json!({"operation":"list_connections"}),
        json!({"operation":"list_github_repos","request":{"connection_id":"conn"}}),
        json!({"operation":"list_available_triggers","request":{"toolkit":"github","connection_id":"conn"}}),
        json!({"operation":"list_triggers","request":{"toolkit":"github"}}),
    ] {
        let operation: ComposioReadOperation =
            serde_json::from_value(value.clone()).expect("valid read fixture");
        assert_eq!(
            serde_json::to_value(operation).expect("serializable operation"),
            value
        );
    }
    for value in [
        json!({"operation":"list_toolkits","result":{"toolkits":[]}}),
        json!({"operation":"list_connections","result":{"connections":[]}}),
        json!({"operation":"list_github_repos","result":{"connectionId":"conn","repositories":[]}}),
        json!({"operation":"list_available_triggers","result":{"triggers":[{"slug":"GH_NEW","scope":"github_repo","defaultConfig":{"owner":"org"},"requiredConfigKeys":["repo"],"repo":{"owner":"org","repo":"project"}}]}}),
        json!({"operation":"list_triggers","result":{"triggers":[{"id":"trig","slug":"GH_NEW","toolkit":"github","connectionId":"conn","triggerConfig":{"repo":"project"},"state":"ACTIVE"}]}}),
    ] {
        let reply: ComposioRoutedReadReply =
            serde_json::from_value(value.clone()).expect("valid read fixture");
        assert_eq!(
            serde_json::to_value(reply).expect("serializable reply"),
            value
        );
    }
    assert!(!crate::is_compatible((1, 13)));
    assert!(!crate::is_compatible((1, 14)));
    assert!(!crate::is_compatible((1, 15)));
    assert!(crate::is_compatible(crate::CONTRACT_VERSION));
}
