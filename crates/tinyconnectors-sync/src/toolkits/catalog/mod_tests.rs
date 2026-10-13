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
fn catalog_for_toolkit_resolves_every_capability_toolkit() {
    // Every toolkit the capability surface reports on must have a catalog —
    // that is what `curated_tools: true` in the matrix claims about it.
    for toolkit in CAPABILITY_TOOLKITS {
        assert!(
            catalog_for_toolkit(toolkit).is_some(),
            "no curated catalog for advertised toolkit {toolkit}"
        );
    }
}

#[test]
fn catalog_for_toolkit_honours_slug_aliases() {
    // Multi-segment `ONE_DRIVE_*` extracts "one_drive"; "one" is a legacy alias.
    // Both UI/backend spellings remain accepted.
    for alias in ["one", "one_drive", "onedrive", "OneDrive"] {
        assert!(
            catalog_for_toolkit(alias).is_some(),
            "OneDrive alias {alias} did not resolve"
        );
    }
    // The legacy "microsoft" alias still reaches the Teams catalog.
    assert_eq!(
        catalog_for_toolkit("microsoft").map(<[CuratedTool]>::len),
        catalog_for_toolkit("microsoft_teams").map(<[CuratedTool]>::len)
    );
    for alias in ["google_calendar", "googlecalendar", "GOOGLECALENDAR"] {
        assert!(
            catalog_for_toolkit(alias).is_some(),
            "{alias} did not resolve"
        );
    }
    assert!(
        catalog_for_toolkit("  gmail  ").is_some(),
        "slug is not trimmed"
    );
    assert!(catalog_for_toolkit("nonexistent-toolkit").is_none());
}

#[test]
fn every_native_provider_has_a_catalog() {
    for slug in NATIVE_PROVIDERS {
        assert!(
            catalog_for_toolkit(slug).is_some(),
            "native provider {slug} has no curated catalog"
        );
        assert!(has_native_provider(slug));
        assert!(
            CAPABILITY_TOOLKITS.contains(slug),
            "native provider {slug} is missing from the capability list"
        );
    }
    assert!(!has_native_provider("jira"));
    assert!(!has_native_provider("nonexistent-toolkit"));
}

#[test]
fn toolkit_description_is_populated_for_every_capability_toolkit() {
    let generic = toolkit_description("definitely-not-a-real-toolkit");
    for toolkit in CAPABILITY_TOOLKITS {
        let d = toolkit_description(toolkit);
        assert!(!d.trim().is_empty(), "{toolkit} has an empty description");
        assert_ne!(
            d, generic,
            "{toolkit} falls through to the generic description"
        );
    }
}

#[test]
fn toolkit_description_recognizes_the_legacy_microsoft_alias() {
    // `catalog_for_toolkit` resolves both "microsoft" and "microsoft_teams" to
    // the same Teams catalog; the description must not diverge for the alias.
    assert_eq!(
        toolkit_description("microsoft"),
        toolkit_description("microsoft_teams")
    );
    assert_ne!(
        toolkit_description("microsoft"),
        toolkit_description("definitely-not-a-real-toolkit")
    );
}

#[test]
fn curated_catalogs_carry_no_duplicate_slugs() {
    // `find_curated` returns the first match, so a duplicate with a different
    // scope would make the gate's answer depend on table order.
    for toolkit in CAPABILITY_TOOLKITS {
        let catalog = catalog_for_toolkit(toolkit).expect("catalog");
        let mut seen: Vec<&str> = catalog.iter().map(|t| t.slug).collect();
        seen.sort_unstable();
        let before = seen.len();
        seen.dedup();
        assert_eq!(before, seen.len(), "{toolkit} catalog has duplicate slugs");
    }
}
