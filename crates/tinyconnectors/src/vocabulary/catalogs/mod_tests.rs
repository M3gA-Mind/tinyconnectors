//! Compatibility tests for the compiled connector vocabulary.

use super::*;

// Tests for the surrounding module.

/// Every toolkit with a curated catalog, in display order.
const CAPABILITY_TOOLKITS: &[&str] = &[
    "gmail",
    "notion",
    "slack",
    "clickup",
    "github",
    "discord",
    "googlecalendar",
    "googledrive",
    "googledocs",
    "googlesheets",
    "outlook",
    "microsoft_teams",
    "linear",
    "jira",
    "trello",
    "asana",
    "dropbox",
    "twitter",
    "spotify",
    "telegram",
    "whatsapp",
    "shopify",
    "stripe",
    "hubspot",
    "salesforce",
    "airtable",
    "figma",
    "youtube",
    "one_drive",
    "excel",
    "todoist",
];

#[test]
fn curated_scope_for_reads_the_catalogs_entry_not_the_heuristic() {
    // Pick a real curated read action and assert the catalog's own verdict.
    let catalog = catalog_for_toolkit("gmail").expect("gmail catalog");
    let read_action = catalog
        .iter()
        .find(|t| t.scope == ToolScope::Read)
        .expect("gmail has a curated read action");
    assert_eq!(curated_scope_for(read_action.slug), Some(ToolScope::Read));

    // An uncurated slug on a curated toolkit is `None` — deliberately not the
    // `classify_unknown` heuristic, which callers opt into explicitly.
    assert_eq!(curated_scope_for("GMAIL_NO_SUCH_ACTION_EXISTS"), None);
    // A slug with no toolkit prefix at all.
    assert_eq!(curated_scope_for("nonsense"), None);
}

#[test]
fn is_action_visible_gates_on_the_curated_scope() {
    let catalog = catalog_for_toolkit("gmail").expect("gmail catalog");
    let read_action = catalog
        .iter()
        .find(|t| t.scope == ToolScope::Read)
        .expect("gmail has a curated read action");
    let admin_action = catalog
        .iter()
        .find(|t| t.scope == ToolScope::Admin)
        .expect("gmail has a curated admin action");

    let read_only = UserScopePref {
        read: true,
        write: false,
        admin: false,
    };
    assert!(is_action_visible_with_pref(read_action.slug, read_only));
    assert!(!is_action_visible_with_pref(admin_action.slug, read_only));

    let all = UserScopePref {
        read: true,
        write: true,
        admin: true,
    };
    assert!(is_action_visible_with_pref(admin_action.slug, all));

    // Uncurated action on a curated toolkit is hidden regardless of pref —
    // curation is a whitelist, so absence means "not surfaced", never
    // "fall back to the heuristic".
    assert!(!is_action_visible_with_pref("GMAIL_NO_SUCH_ACTION", all));

    // A slug with no toolkit prefix is not ours to gate.
    assert!(is_action_visible_with_pref("nonsense", read_only));
}

