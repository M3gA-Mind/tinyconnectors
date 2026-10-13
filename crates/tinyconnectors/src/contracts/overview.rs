//! Assembling the connected-integration overview from raw backend listings.
//!
//! Pure: the caller fetches the allowlist, the connections and the tool
//! listing for the route it serves, loads the scope preference of each
//! connected toolkit, and hands them here. Nothing in this file talks to a
//! backend, so the shaping rules can be tested against literals.

use std::collections::{HashMap, HashSet};

use tinyconnectors_bus::composio::{
    ComposioConnectedIntegration, ComposioConnectedIntegrationTool, ComposioConnection,
    ComposioGatedIntegrationTool, ComposioIntegrationConnection, ComposioToolSchema,
    ComposioToolkitCatalogEntry,
};
use tinyconnectors_bus::vocabulary::UserScopePref;

use crate::vocabulary_impl::catalogs::{
    curated_scope_for, is_action_visible_with_pref, toolkit_description,
};

/// The toolkits a user may connect.
///
/// Sourced from the catalog's enabled entries, which is the same predicate the
/// backend's connect gate applies, so the advertised set cannot drift from what
/// connecting will allow. Falls back to the flat `toolkits` array only when the
/// catalog is absent (a backend that predates it). Disabled catalog entries are
/// dropped because the gate would refuse them. Slugs are trimmed and
/// lowercased.
pub(crate) fn connectable_toolkit_slugs(
    toolkits: &[String],
    catalog: &[ComposioToolkitCatalogEntry],
) -> Vec<String> {
    let normalize = |slug: &str| slug.trim().to_ascii_lowercase();
    if catalog.is_empty() {
        toolkits
            .iter()
            .map(|slug| normalize(slug))
            .filter(|slug| !slug.is_empty())
            .collect()
    } else {
        catalog
            .iter()
            .filter(|entry| entry.enabled.unwrap_or(false))
            .map(|entry| normalize(&entry.slug))
            .filter(|slug| !slug.is_empty())
            .collect()
    }
}

/// Descriptions the backend catalog supplies, keyed by lowercase slug. Empty
/// descriptions are skipped so the compiled table applies instead.
pub(crate) fn catalog_descriptions(
    catalog: &[ComposioToolkitCatalogEntry],
) -> HashMap<String, String> {
    catalog
        .iter()
        .filter_map(|entry| {
            let slug = entry.slug.trim().to_ascii_lowercase();
            if slug.is_empty() {
                return None;
            }
            let description = entry
                .description
                .as_deref()
                .map(str::trim)
                .filter(|description| !description.is_empty())?;
            Some((slug, description.to_string()))
        })
        .collect()
}

/// Sorted, deduplicated toolkits that have an active connection.
pub(crate) fn active_toolkits(connections: &[ComposioConnection]) -> Vec<String> {
    let mut slugs: Vec<String> = connections
        .iter()
        .filter(|connection| connection.is_active())
        .map(ComposioConnection::normalized_toolkit)
        .filter(|toolkit| !toolkit.is_empty())
        .collect();
    slugs.sort();
    slugs.dedup();
    slugs
}

/// How actionable a non-active status is: expired first, then failed, then an
/// OAuth handoff still in flight, then anything else.
fn status_priority(status: &str) -> u8 {
    match status.trim().to_ascii_uppercase().as_str() {
        "EXPIRED" => 4,
        "FAILED" | "ERROR" => 3,
        "INITIATED" | "INITIALIZING" | "PENDING" => 2,
        _ => 1,
    }
}

/// The most actionable non-active status per toolkit.
///
/// A toolkit with an active connection has none: its other rows are noise. On
/// a tie the first row wins.
fn non_active_status_by_toolkit(
    connections: &[ComposioConnection],
    connected: &HashSet<String>,
) -> HashMap<String, String> {
    let mut best: HashMap<String, (u8, String)> = HashMap::new();
    for connection in connections.iter().filter(|c| !c.is_active()) {
        let toolkit = connection.normalized_toolkit();
        if toolkit.is_empty() || connected.contains(&toolkit) {
            continue;
        }
        let priority = status_priority(&connection.status);
        match best.get_mut(&toolkit) {
            Some(current) if priority > current.0 => {
                *current = (priority, connection.status.clone());
            }
            Some(_) => {}
            None => {
                best.insert(toolkit, (priority, connection.status.clone()));
            }
        }
    }
    best.into_iter()
        .map(|(toolkit, (_, status))| (toolkit, status))
        .collect()
}

/// The raw listings an overview is assembled from.
pub(crate) struct Listings<'a> {
    /// Toolkits to list, in any order, duplicates allowed.
    pub allowlist: Vec<String>,
    /// Every connection row the route reports.
    pub connections: &'a [ComposioConnection],
    /// The tool listing for the connected toolkits.
    pub tools: &'a [ComposioToolSchema],
    /// Backend catalog descriptions by lowercase slug.
    pub descriptions: &'a HashMap<String, String>,
    /// The scope preference of each connected toolkit. A toolkit missing here
    /// gets the default preference.
    pub preferences: &'a HashMap<String, UserScopePref>,
}

/// Builds the overview: one entry per allowlisted toolkit, sorted by slug.
///
/// A connected toolkit carries the actions its user's preference permits, and
/// the curated actions it hides as `gated_tools`. Uncurated hidden actions are
/// not listed at all: they are not first-class capabilities, and naming them
/// would clutter the prompt with internal slugs.
pub(crate) fn assemble(listings: &Listings<'_>) -> Vec<ComposioConnectedIntegration> {
    let connected: HashSet<String> = active_toolkits(listings.connections).into_iter().collect();
    let non_active = non_active_status_by_toolkit(listings.connections, &connected);

    let mut toolkits = listings.allowlist.clone();
    toolkits.sort();
    toolkits.dedup();

    let mut integrations: Vec<ComposioConnectedIntegration> = toolkits
        .iter()
        .map(|slug| {
            let is_connected = connected.contains(slug);
            let (tools, gated_tools) = if is_connected {
                split_tools(slug, listings)
            } else {
                (Vec::new(), Vec::new())
            };
            ComposioConnectedIntegration {
                toolkit: slug.clone(),
                description: listings
                    .descriptions
                    .get(slug)
                    .cloned()
                    .unwrap_or_else(|| toolkit_description(slug).to_string()),
                tools,
                gated_tools,
                connected: is_connected,
                connections: if is_connected {
                    active_connections(slug, listings.connections)
                } else {
                    Vec::new()
                },
                non_active_status: if is_connected {
                    None
                } else {
                    non_active.get(slug).cloned()
                },
            }
        })
        .collect();
    integrations.sort_by(|a, b| a.toolkit.cmp(&b.toolkit));
    integrations
}

/// Splits a connected toolkit's actions into callable and gated.
///
/// The prefix is anchored with an underscore so `git` never claims `github`'s
/// actions.
fn split_tools(
    slug: &str,
    listings: &Listings<'_>,
) -> (
    Vec<ComposioConnectedIntegrationTool>,
    Vec<ComposioGatedIntegrationTool>,
) {
    let prefix = format!("{}_", slug.to_uppercase());
    let preference = listings.preferences.get(slug).copied().unwrap_or_default();
    let mut visible = Vec::new();
    let mut gated = Vec::new();
    for tool in listings
        .tools
        .iter()
        .filter(|tool| tool.function.name.starts_with(&prefix))
    {
        let name = tool.function.name.clone();
        let description = tool.function.description.clone().unwrap_or_default();
        if is_action_visible_with_pref(&name, preference) {
            visible.push(ComposioConnectedIntegrationTool {
                name,
                description,
                parameters: tool.function.parameters.clone(),
            });
        } else if let Some(required_scope) = curated_scope_for(&name) {
            gated.push(ComposioGatedIntegrationTool {
                name,
                description,
                required_scope,
            });
        }
    }
    (visible, gated)
}

/// Active connections of `slug`, oldest first; the first is the default.
fn active_connections(
    slug: &str,
    connections: &[ComposioConnection],
) -> Vec<ComposioIntegrationConnection> {
    let mut active: Vec<&ComposioConnection> = connections
        .iter()
        .filter(|c| c.is_active() && c.normalized_toolkit() == slug)
        .collect();
    active.sort_by(|a, b| a.created_at.cmp(&b.created_at));
    active
        .iter()
        .enumerate()
        .map(|(index, connection)| ComposioIntegrationConnection {
            connection_id: connection.id.clone(),
            label: [
                connection.account_email.as_deref(),
                connection.workspace.as_deref(),
                connection.username.as_deref(),
            ]
            .into_iter()
            .flatten()
            .map(str::trim)
            .find(|label| !label.is_empty())
            .map(str::to_string),
            is_default: index == 0,
        })
        .collect()
}

#[cfg(test)]
#[path = "overview_tests.rs"]
mod tests;
