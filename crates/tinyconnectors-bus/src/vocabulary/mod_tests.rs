//! Wire compatibility tests for portable connector payloads.

use super::*;

#[test]
fn old_scope_rows_keep_read_write_defaults_and_admin_opt_in() -> serde_json::Result<()> {
    for (wire, expected) in [
        (
            "{}",
            UserScopePref {
                read: true,
                write: true,
                admin: false,
            },
        ),
        (
            r#"{"admin":true}"#,
            UserScopePref {
                read: true,
                write: true,
                admin: true,
            },
        ),
        (
            r#"{"read":false,"write":false}"#,
            UserScopePref {
                read: false,
                write: false,
                admin: false,
            },
        ),
    ] {
        assert_eq!(serde_json::from_str::<UserScopePref>(wire)?, expected);
    }
    Ok(())
}

#[test]
fn matching_tags_and_unknown_profile_values_fail_closed() -> serde_json::Result<()> {
    assert_eq!(serde_json::to_value(IdentityKind::UserId)?, "user_id");
    assert_eq!(
        serde_json::to_value(UnknownScopeProfile::SubstringCompatibility)?,
        "substring_compatibility"
    );
    assert!(serde_json::from_str::<UnknownScopeProfile>(r#""unknown""#).is_err());
    assert!(serde_json::from_str::<IdentityKind>(r#""username""#).is_err());
    Ok(())
}

#[test]
fn shared_provider_profile_preserves_omitted_fields_and_extras() -> serde_json::Result<()> {
    let profile = ProviderUserProfile {
        toolkit: "gmail".into(),
        email: Some("alice@example.com".into()),
        extras: serde_json::json!({"future": "preserved"}),
        ..Default::default()
    };
    let wire = serde_json::json!({
        "toolkit": "gmail", "email": "alice@example.com", "extras": {"future": "preserved"}
    });
    assert_eq!(serde_json::to_value(&profile)?, wire);
    assert_eq!(
        serde_json::from_value::<ProviderUserProfile>(wire)?,
        profile
    );
    assert_eq!(
        serde_json::to_value(ProviderUserProfile::default())?,
        serde_json::json!({"toolkit": ""})
    );
    Ok(())
}

#[test]
fn provider_profile_serialises_every_field_as_the_host_always_did() -> serde_json::Result<()> {
    // Wire compatibility with the host's former `ProviderUserProfile`: unset
    // options are written as `null`, never omitted.
    assert_eq!(
        serde_json::to_string(&ProviderUserProfile::default())?,
        r#"{"toolkit":"","connection_id":null,"display_name":null,"email":null,"username":null,"avatar_url":null,"profile_url":null,"extras":null}"#
    );
    Ok(())
}

#[test]
fn normalized_task_payload_is_byte_identical_to_the_host_shape() -> serde_json::Result<()> {
    let task = NormalizedTask {
        external_id: "1".into(),
        provider: "github".into(),
        kind: TaskKind::PullRequest,
        title: "t".into(),
        labels: vec!["bug".into()],
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_string(&task)?,
        r#"{"externalId":"1","sourceId":"","provider":"github","kind":"pull_request","title":"t","labels":["bug"],"raw":null}"#
    );
    Ok(())
}

#[test]
fn task_filter_payload_and_cap_match_the_host_shape() -> serde_json::Result<()> {
    assert_eq!(
        serde_json::to_string(&TaskFetchFilter::default())?,
        r#"{"assigneeIsMe":false,"githubFetchMode":"auto","labels":[],"extra":null,"max":0}"#
    );
    assert_eq!(
        TaskFetchFilter::default().effective_max(),
        DEFAULT_TASK_FETCH_MAX
    );
    Ok(())
}
