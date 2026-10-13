//! Compiled action catalogs, aliases and explicit compatibility classifier.

pub(crate) use tinyconnectors_bus::vocabulary::{ToolScope, UserScopePref};

pub(crate) use tinyconnectors_sync::CuratedTool;

pub(crate) use tinyconnectors_bus::vocabulary::{
    action_slug_matches, canonical_action_slug, classify_unknown, toolkit_from_slug,
};

/// Look up a slug inside a curated catalog, case-insensitively.
pub(crate) fn find_curated<'a>(catalog: &'a [CuratedTool], slug: &str) -> Option<&'a CuratedTool> {
    catalog.iter().find(|t| t.slug.eq_ignore_ascii_case(slug))
}

/// Every toolkit slug and accepted alias that has a curated, agent-ready catalog.
///
/// This is the source of truth behind the "preview / agent integration coming
/// soon" badge: a connected toolkit whose slug is *not* in this list can be
/// authorized but has no curated tool surface, so the agent cannot use it
/// productively and the UI should say so rather than offering it.
///
/// Returned sorted, so the RPC response is stable across builds.
///
/// This describes the extracted catalogs rather than provider-registry
/// capabilities. The compiled vocabulary operation returns owned names to
/// hosts; hosts do not link these tables or compute their own copy.
pub(crate) fn agent_ready_toolkits() -> Vec<&'static str> {
    let mut slugs: Vec<&'static str> = vec![
        // Native providers.
        "gmail",
        "notion",
        "github",
        "clickup",
        // Catalog-only toolkits.
        "slack",
        "discord",
        "googlecalendar",
        "google_calendar",
        "googledrive",
        "google_drive",
        "googledocs",
        "google_docs",
        "googlesheets",
        "google_sheets",
        "outlook",
        "microsoft_teams",
        "microsoft",
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
        "onedrive",
        "one",
        "excel",
        "todoist",
    ];
    slugs.sort_unstable();
    slugs
}

#[cfg(test)]
#[doc = "Compatibility tests for the compiled connector vocabulary."]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
#[path = "scopes_tests.rs"]
mod tests;
