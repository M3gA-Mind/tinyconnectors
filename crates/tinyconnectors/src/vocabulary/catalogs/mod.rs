//! Scope-aware lookups over the curated Composio catalogs.
//!
//! The tables themselves, the toolkit-to-catalog map and the capability
//! descriptions live in `tinyconnectors_sync::toolkits::catalog`, where the
//! providers read them too; this module adds the scope-preference gating the
//! vocabulary members need.

use super::scopes::{
    CuratedTool, ToolScope, UserScopePref, classify_unknown, find_curated, toolkit_from_slug,
};

pub(crate) use tinyconnectors_sync::toolkits::catalog::{
    catalog_for_toolkit, has_native_provider, toolkit_description,
};

/// Should this action slug appear in the agent's tool surface, given an
/// already-loaded user scope preference?
///
/// `true` when the action is in its toolkit's curated whitelist (or the toolkit
/// has no curation) **and** the preference allows its classification. Falls
/// back to [`classify_unknown`] for uncurated toolkits.
///
/// Takes a pre-loaded preference because the typical caller loops over
/// toolkits, where awaiting once per toolkit is cheaper than once per action.
#[must_use]
pub(crate) fn is_action_visible_with_pref(slug: &str, pref: UserScopePref) -> bool {
    let Some(toolkit) = toolkit_from_slug(slug) else {
        return true;
    };
    match catalog_for_toolkit(&toolkit) {
        Some(catalog) => match find_curated(catalog, slug) {
            Some(curated) => pref.allows(curated.scope),
            None => false,
        },
        None => pref.allows(classify_unknown(slug)),
    }
}

/// The curated scope `slug` requires, if it appears in any catalog.
///
/// `None` for a genuinely uncurated slug — a caller wanting a defensible
/// heuristic for those should reach for [`classify_unknown`] explicitly.
///
/// Sibling of [`is_action_visible_with_pref`]: that one answers "visible?",
/// this one answers "what scope is required?", so a caller can render an unlock
/// hint without redoing the catalog walk.
#[must_use]
pub(crate) fn curated_scope_for(slug: &str) -> Option<ToolScope> {
    let toolkit = toolkit_from_slug(slug)?;
    let catalog = catalog_for_toolkit(&toolkit)?;
    find_curated(catalog, slug).map(|c| c.scope)
}

#[cfg(test)]
#[doc = "Compatibility tests for the compiled connector vocabulary."]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
#[path = "mod_tests.rs"]
mod tests;
