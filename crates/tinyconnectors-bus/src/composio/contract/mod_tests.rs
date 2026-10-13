//! Serde representation tests for the contract, probe and overview payloads.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::{
    ComposioConnectedIntegration, ComposioConnectedIntegrationsMode,
    ComposioGatedIntegrationTool, ComposioGetToolContractRequest,
    ComposioListConnectedIntegrationsRequest, ComposioProbeToolOutputRequest, ComposioToolContract,
};
use crate::vocabulary::ToolScope;
use serde_json::json;

#[test]
fn a_contract_keeps_unknown_schemas_distinct_from_empty_ones() {
    let contract = ComposioToolContract {
        slug: "GITHUB_LIST_ISSUES".into(),
        toolkit: "github".into(),
        description: None,
        required_args: vec![],
        input_schema: None,
        output_fields: vec![],
        output_schema: None,
        primary_array_path: None,
        is_curated: true,
    };
    let wire = serde_json::to_value(&contract).unwrap();
    assert_eq!(wire["output_schema"], json!(null));
    assert_eq!(wire["required_args"], json!([]));
    assert_eq!(
        serde_json::from_value::<ComposioToolContract>(wire).unwrap(),
        contract
    );
}

#[test]
fn a_probe_request_without_args_decodes_to_null() {
    let request: ComposioProbeToolOutputRequest =
        serde_json::from_value(json!({"slug": "GMAIL_FETCH_EMAILS"})).unwrap();
    assert!(request.args.is_null());
}

#[test]
fn the_mode_is_spelled_in_lowercase_on_the_wire() {
    let request = ComposioListConnectedIntegrationsRequest {
        mode: ComposioConnectedIntegrationsMode::Direct,
    };
    assert_eq!(serde_json::to_value(request).unwrap(), json!({"mode": "direct"}));
    assert!(
        serde_json::from_value::<ComposioListConnectedIntegrationsRequest>(
            json!({"mode": "Backend"})
        )
        .is_err()
    );
}

#[test]
fn a_gated_tool_carries_a_scope_and_no_parameters() {
    let gated = ComposioGatedIntegrationTool {
        name: "GMAIL_DELETE_MESSAGE".into(),
        description: String::new(),
        required_scope: ToolScope::Admin,
    };
    let wire = serde_json::to_value(&gated).unwrap();
    assert_eq!(wire["required_scope"], "admin");
    assert!(wire.get("parameters").is_none());
}

#[test]
fn an_integration_round_trips() {
    let wire = json!({
        "toolkit": "gmail", "description": "mail", "tools": [], "gated_tools": [],
        "connected": false, "connections": [], "non_active_status": "EXPIRED"
    });
    let integration: ComposioConnectedIntegration = serde_json::from_value(wire.clone()).unwrap();
    assert_eq!(integration.non_active_status.as_deref(), Some("EXPIRED"));
    assert_eq!(serde_json::to_value(integration).unwrap(), wire);
}

#[test]
fn a_contract_request_names_only_the_slug() {
    let request = ComposioGetToolContractRequest {
        slug: "GMAIL_SEND_EMAIL".into(),
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        json!({"slug": "GMAIL_SEND_EMAIL"})
    );
}
