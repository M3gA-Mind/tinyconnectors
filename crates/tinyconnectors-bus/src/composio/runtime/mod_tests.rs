use super::*;

#[test]
fn runtime_handshake_reports_actual_mode_with_major_and_minor() {
    for mode in [
        ComposioRuntimeMode::Classic,
        ComposioRuntimeMode::RoutedReadOnly,
    ] {
        let reply = ComposioRuntimeModeResponse {
            mode,
            contract_version: (1, 15),
        };
        let value = serde_json::to_value(&reply).expect("serializable mode");
        assert_eq!(value["contract_version"], serde_json::json!([1, 15]));
        assert_eq!(
            serde_json::from_value::<ComposioRuntimeModeResponse>(value).expect("valid mode"),
            reply
        );
    }
    assert_eq!(ComposioRuntimeMode::default(), ComposioRuntimeMode::Classic);
    assert!(serde_json::from_str::<ComposioRuntimeMode>(r#""routd_read_only""#).is_err());
}
