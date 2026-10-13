//! Pins the text-only slug helpers: verb precedence in
//! [`super::classify_unknown`], the multi-segment toolkit prefixes and the
//! alias rewrite.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use super::*;

#[test]
fn destructive_verbs_classify_as_admin() {
    assert_eq!(classify_unknown("GMAIL_DELETE_EMAIL"), ToolScope::Admin);
    assert_eq!(classify_unknown("GMAIL_TRASH_EMAIL"), ToolScope::Admin);
    assert_eq!(classify_unknown("GMAIL_MODIFY_LABELS"), ToolScope::Admin);
    assert_eq!(classify_unknown("DRIVE_SHARE_FILE"), ToolScope::Admin);
}

#[test]
fn mutating_verbs_classify_as_write() {
    assert_eq!(classify_unknown("GMAIL_SEND_EMAIL"), ToolScope::Write);
    assert_eq!(classify_unknown("NOTION_CREATE_PAGE"), ToolScope::Write);
    assert_eq!(classify_unknown("NOTION_UPDATE_PAGE"), ToolScope::Write);
}

#[test]
fn anything_else_classifies_as_read() {
    assert_eq!(classify_unknown("GMAIL_FETCH_EMAILS"), ToolScope::Read);
    assert_eq!(classify_unknown("NOTION_SEARCH"), ToolScope::Read);
    assert_eq!(classify_unknown("GMAIL_GET_PROFILE"), ToolScope::Read);
}

#[test]
fn admin_verbs_are_checked_before_write_verbs() {
    // `DELETE_DRAFT` contains `DRAFT`, a write verb. If the two lists were
    // checked in the other order this would gate as a write and a destructive
    // action would run under a write-only preference.
    assert_eq!(classify_unknown("GMAIL_DELETE_DRAFT"), ToolScope::Admin);
}

#[test]
fn classification_ignores_slug_casing() {
    assert_eq!(classify_unknown("gmail_delete_email"), ToolScope::Admin);
    assert_eq!(classify_unknown("gmail_send_email"), ToolScope::Write);
}

#[test]
fn a_toolkit_slug_is_the_lowercased_first_segment() {
    assert_eq!(
        toolkit_from_slug("GMAIL_SEND_EMAIL").as_deref(),
        Some("gmail")
    );
    assert_eq!(
        toolkit_from_slug("NOTION_FETCH_DATA").as_deref(),
        Some("notion")
    );
    assert_eq!(
        toolkit_from_slug("noUnderscore").as_deref(),
        Some("nounderscore")
    );
}

#[test]
fn an_empty_slug_names_no_toolkit() {
    assert_eq!(toolkit_from_slug(""), None);
    assert_eq!(toolkit_from_slug("   "), None);
}

#[test]
fn multi_segment_toolkits_keep_their_whole_prefix() {
    // Without these three, `ZOHO_MAIL_*` resolves to `zoho`, matches no
    // connected toolkit, and every action for it is silently dropped.
    assert_eq!(
        toolkit_from_slug("ZOHO_MAIL_SEND_EMAIL").as_deref(),
        Some("zoho_mail")
    );
    assert_eq!(
        toolkit_from_slug("ONE_DRIVE_GET_FILE").as_deref(),
        Some("one_drive")
    );
    assert_eq!(
        toolkit_from_slug("MICROSOFT_TEAMS_SEND_MESSAGE").as_deref(),
        Some("microsoft_teams")
    );
}

#[test]
fn a_curated_lookup_ignores_casing_and_reports_a_miss() {
    let catalog = &[CuratedTool {
        toolkit: "gmail".to_string(),
        slug: "GMAIL_SEND_EMAIL".to_string(),
        scope: ToolScope::Write,
    }];
    assert!(find_curated(catalog, "gmail_send_email").is_some());
    assert!(find_curated(catalog, "GMAIL_SEND_EMAIL").is_some());
    assert!(find_curated(catalog, "GMAIL_DELETE_EMAIL").is_none());
}

#[test]
fn spaced_google_and_microsoft_prefixes_resolve_to_the_real_toolkit() {
    // Production: `get_tool_contract { slug: "GOOGLE_DRIVE_FIND_FILE" }` looked
    // up the non-existent toolkit `google` and told the model to retry a fetch
    // that could never succeed.
    for (slug, toolkit) in [
        ("GOOGLE_DRIVE_FIND_FILE", "googledrive"),
        ("GOOGLE_CALENDAR_EVENTS_LIST", "googlecalendar"),
        ("GOOGLE_SHEETS_BATCH_GET", "googlesheets"),
        ("GOOGLE_DOCS_GET_DOCUMENT_BY_ID", "googledocs"),
        ("GOOGLE_MEET_CREATE_MEET", "googlemeet"),
        ("GOOGLE_TASKS_LIST_TASKS", "googletasks"),
        ("GOOGLE_SLIDES_CREATE_PRESENTATION", "googleslides"),
        ("GOOGLE_PHOTOS_LIST_ALBUMS", "googlephotos"),
        ("GOOGLE_MAPS_TEXT_SEARCH", "google_maps"),
        ("MICROSOFT_OUTLOOK_SEND_EMAIL", "outlook"),
        ("google_drive_find_file", "googledrive"),
    ] {
        assert_eq!(toolkit_from_slug(slug).as_deref(), Some(toolkit), "{slug}");
    }
    // The canonical spellings are unchanged.
    assert_eq!(
        toolkit_from_slug("GOOGLEDRIVE_FIND_FILE").as_deref(),
        Some("googledrive")
    );
    assert_eq!(
        toolkit_from_slug("GOOGLECALENDAR_FIND_EVENT").as_deref(),
        Some("googlecalendar")
    );
    // A prefix only matches at a segment boundary.
    assert_eq!(
        toolkit_from_slug("GOOGLE_DRIVERS_X").as_deref(),
        Some("google")
    );
}

#[test]
fn a_spaced_alias_is_rewritten_to_the_canonical_action_slug() {
    assert_eq!(
        canonical_action_slug("GOOGLE_DRIVE_FIND_FILE").as_deref(),
        Some("GOOGLEDRIVE_FIND_FILE")
    );
    assert_eq!(
        canonical_action_slug("google_calendar_events_list").as_deref(),
        Some("GOOGLECALENDAR_EVENTS_LIST")
    );
    assert_eq!(
        canonical_action_slug("MICROSOFT_OUTLOOK_SEND_EMAIL").as_deref(),
        Some("OUTLOOK_SEND_EMAIL")
    );
    // Already canonical, or a toolkit whose real actions carry the underscore.
    assert_eq!(canonical_action_slug("GOOGLEDRIVE_FIND_FILE"), None);
    assert_eq!(canonical_action_slug("MICROSOFT_TEAMS_SEND_MESSAGE"), None);
    assert_eq!(canonical_action_slug("GMAIL_SEND_EMAIL"), None);

    assert!(action_slug_matches(
        "GOOGLEDRIVE_FIND_FILE",
        "GOOGLE_DRIVE_FIND_FILE"
    ));
    assert!(action_slug_matches(
        "GOOGLEDRIVE_FIND_FILE",
        "googledrive_find_file"
    ));
    assert!(!action_slug_matches(
        "GOOGLEDRIVE_FIND_FILE",
        "GOOGLE_DRIVE_LIST_FILES"
    ));
}
