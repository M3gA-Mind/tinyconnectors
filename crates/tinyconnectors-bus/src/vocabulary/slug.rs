//! Synchronous, text-only helpers for Composio action slugs.
//!
//! A host's hot paths (tool visibility, flow gates, approval hooks) must
//! classify a slug without awaiting the connector module, so the parsing lives
//! in the contract crate: one owner, no I/O, and no tables beyond the handful
//! of toolkit prefixes below. The curated catalogs and the stored preference
//! are the module's (`ListCuratedCatalog`, `GetUserScopes`); `EvaluateActions`
//! answers the same questions asynchronously and uses these helpers.

use super::{CuratedTool, ToolScope};

/// Heuristic fallback for gating a tool that is not in any provider's curated
/// list.
///
/// Prefer the curated classification when one exists; only reach for this when
/// a toolkit has no catalog or the catalog does not mention the slug. Admin
/// verbs are checked first so `MODIFY_LABELS` does not slip into the write
/// bucket on the `UPDATE` substring rule — the ordering is the whole point of
/// the function and not an implementation detail.
#[must_use]
pub fn classify_unknown(slug: &str) -> ToolScope {
    const ADMIN: &[&str] = &[
        "DELETE",
        "TRASH",
        "REMOVE",
        "MODIFY_LABELS",
        "SHARE",
        "REVOKE",
        "DESTROY",
    ];
    const WRITE: &[&str] = &[
        "SEND", "CREATE", "UPDATE", "REPLY", "APPEND", "INSERT", "ADD", "POST", "PATCH", "WRITE",
        "DRAFT",
    ];
    let upper = slug.to_ascii_uppercase();
    if ADMIN.iter().any(|kw| upper.contains(kw)) {
        return ToolScope::Admin;
    }
    if WRITE.iter().any(|kw| upper.contains(kw)) {
        return ToolScope::Write;
    }
    ToolScope::Read
}

/// Look up a slug inside a curated catalog, case-insensitively.
#[must_use]
pub fn find_curated<'a>(catalog: &'a [CuratedTool], slug: &str) -> Option<&'a CuratedTool> {
    catalog.iter().find(|t| t.slug.eq_ignore_ascii_case(slug))
}

/// Toolkits whose identifier itself contains an underscore, so the first
/// segment of their action slugs is not the toolkit: `(action prefix, toolkit)`.
/// Their real action slugs carry the prefix as written.
const MULTI_SEGMENT_TOOLKIT_PREFIXES: &[(&str, &str)] = &[
    ("MICROSOFT_TEAMS_", "microsoft_teams"),
    ("ONE_DRIVE_", "one_drive"),
    ("ZOHO_MAIL_", "zoho_mail"),
    ("GOOGLE_MAPS_", "google_maps"),
];

/// Spellings models use for toolkits whose real slugs have no separator
/// (`GOOGLE_DRIVE_FIND_FILE` for `GOOGLEDRIVE_FIND_FILE`):
/// `(alias prefix, toolkit, canonical action prefix)`. Without them the first
/// segment, `google`, names a toolkit that does not exist.
const ALIASED_TOOLKIT_PREFIXES: &[(&str, &str, &str)] = &[
    ("GOOGLE_DRIVE_", "googledrive", "GOOGLEDRIVE_"),
    ("GOOGLE_CALENDAR_", "googlecalendar", "GOOGLECALENDAR_"),
    ("GOOGLE_SHEETS_", "googlesheets", "GOOGLESHEETS_"),
    ("GOOGLE_DOCS_", "googledocs", "GOOGLEDOCS_"),
    ("GOOGLE_MEET_", "googlemeet", "GOOGLEMEET_"),
    ("GOOGLE_TASKS_", "googletasks", "GOOGLETASKS_"),
    ("GOOGLE_SLIDES_", "googleslides", "GOOGLESLIDES_"),
    ("GOOGLE_PHOTOS_", "googlephotos", "GOOGLEPHOTOS_"),
    ("MICROSOFT_OUTLOOK_", "outlook", "OUTLOOK_"),
];

/// Extract the toolkit slug from a Composio action slug.
///
/// Most action slugs follow `<TOOLKIT>_<VERB>_…` — `GMAIL_SEND_EMAIL` yields
/// `gmail`. A few toolkit identifiers contain underscores themselves, so those
/// need known-prefix handling or a connected-toolkit check silently drops every
/// action for them (`ZOHO_MAIL_*` would resolve to the non-existent `zoho`).
/// Separated spellings of separator-less toolkits (`GOOGLE_DRIVE_*`) resolve to
/// the real toolkit too (`googledrive`); see [`canonical_action_slug`].
///
/// Returns `None` only for an empty or whitespace-only slug.
#[must_use]
pub fn toolkit_from_slug(slug: &str) -> Option<String> {
    let trimmed = slug.trim();
    if trimmed.is_empty() {
        return None;
    }
    let upper = trimmed.to_ascii_uppercase();
    for (prefix, toolkit) in MULTI_SEGMENT_TOOLKIT_PREFIXES {
        if upper.starts_with(prefix) {
            return Some((*toolkit).to_string());
        }
    }
    for (alias, toolkit, _) in ALIASED_TOOLKIT_PREFIXES {
        if upper.starts_with(alias) {
            return Some((*toolkit).to_string());
        }
    }
    let prefix = trimmed.split('_').next()?;
    if prefix.is_empty() {
        None
    } else {
        Some(prefix.to_ascii_lowercase())
    }
}

/// The real action slug for an aliased spelling — `GOOGLE_DRIVE_FIND_FILE`
/// becomes `GOOGLEDRIVE_FIND_FILE` — or `None` when `slug` uses no alias.
#[must_use]
pub fn canonical_action_slug(slug: &str) -> Option<String> {
    let upper = slug.trim().to_ascii_uppercase();
    ALIASED_TOOLKIT_PREFIXES
        .iter()
        .find_map(|(alias, _, canonical)| {
            upper
                .strip_prefix(alias)
                .map(|rest| format!("{canonical}{rest}"))
        })
}

/// Whether catalog slug `candidate` is the action `requested` names, ignoring
/// case and accepting an aliased spelling ([`canonical_action_slug`]).
#[must_use]
pub fn action_slug_matches(candidate: &str, requested: &str) -> bool {
    let requested = requested.trim();
    candidate.eq_ignore_ascii_case(requested)
        || canonical_action_slug(requested).is_some_and(|c| candidate.eq_ignore_ascii_case(&c))
}

#[cfg(test)]
#[doc = "Tests for the slug helpers."]
#[path = "slug_tests.rs"]
mod tests;
