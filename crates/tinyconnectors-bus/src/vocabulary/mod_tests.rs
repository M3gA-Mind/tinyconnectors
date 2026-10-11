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
