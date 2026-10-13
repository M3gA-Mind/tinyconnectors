//! The fetching half of the contract, probe and overview members.
//!
//! The shaping rules are pure and live in [`crate::contracts`]. This file
//! reaches the backend through the module's live route and keeps the caches.

use std::collections::HashMap;

use serde_json::Value;
use tinyconnectors_bus::composio::{
    ComposioConnectedIntegration, ComposioConnectedIntegrationsMode, ComposioConnection,
    ComposioProbedOutputSample, ComposioToolContract, ComposioToolSchema,
};
use tinyconnectors_bus::vocabulary::{ToolScope, UserScopePref};
use tinyconnectors_sync::ScopePrefStore;

use super::{ConnectorService, TinyBusResult, to_bus_error};
use crate::contracts::{self, overview};

/// The route name a mode is served over.
const fn route_for(mode: ComposioConnectedIntegrationsMode) -> &'static str {
    match mode {
        ComposioConnectedIntegrationsMode::Backend => "proxy",
        ComposioConnectedIntegrationsMode::Direct => "direct",
    }
}

impl ConnectorService {
    /// Every contract of `toolkit`, from the cache or one backend listing.
    ///
    /// The listing is deliberately unfiltered: it covers every real action,
    /// connected or not, curated or not, so a caller grounds against the whole
    /// catalog. Concurrent misses for one toolkit share a single fetch.
    ///
    /// Cached contracts carry no probe overlay; [`Self::contract_with_probe`]
    /// applies it per read so a probe made later is never shadowed.
    pub(super) async fn toolkit_contracts(
        &self,
        toolkit: &str,
    ) -> TinyBusResult<(String, Vec<ComposioToolContract>)> {
        let key = toolkit.trim().to_ascii_lowercase();
        if key.is_empty() {
            return Err(tinybus::Error::failed("toolkit must not be empty"));
        }
        if let Some(cached) = self.contract_caches.catalog(&key) {
            return Ok((key, cached));
        }
        let lock = self.contract_caches.fetch_lock(&key);
        let _guard = lock.lock().await;
        // Another caller may have filled the cache while this one waited.
        if let Some(cached) = self.contract_caches.catalog(&key) {
            return Ok((key, cached));
        }
        tracing::debug!(toolkit = %key, "[connectors] tool contracts: fetching (miss or expired)");
        let listing = self
            .client()?
            .list_tools(std::slice::from_ref(&key), &[])
            .await
            .map_err(|error| {
                tracing::debug!(toolkit = %key, %error, "[connectors] tool contracts: fetch failed");
                to_bus_error(&error)
            })?;
        let fetched = contracts::contracts_from_listing(&key, &listing);
        self.contract_caches.remember_catalog(&key, fetched.clone());
        Ok((key, fetched))
    }

    /// One contract by slug, with any cached probe laid over it.
    pub(super) async fn contract_with_probe(
        &self,
        slug: &str,
    ) -> TinyBusResult<Option<ComposioToolContract>> {
        let slug = slug.trim();
        let Some(toolkit) = contracts::toolkit_of(slug) else {
            return Ok(None);
        };
        let (_, contracts) = self.toolkit_contracts(&toolkit).await?;
        Ok(contracts
            .into_iter()
            .find(|contract| contract.slug.eq_ignore_ascii_case(slug))
            .map(|contract| self.contract_caches.apply_probe(contract)))
    }

    /// Runs one confirmed read action and reports the real output shape.
    ///
    /// Gates, in order, each failing closed: the slug is a confirmed read, its
    /// toolkit has an active connection, and the call itself succeeded. The
    /// derived shape is cached; the raw sample is returned to this caller only.
    pub(super) async fn probe_output(
        &self,
        slug: &str,
        args: Value,
    ) -> TinyBusResult<ComposioProbedOutputSample> {
        let slug = slug.trim();
        if slug.is_empty() {
            return Err(tinybus::Error::failed("probe: slug must not be empty"));
        }
        match contracts::resolve_action_scope(slug) {
            Some(ToolScope::Read) => {}
            Some(other) => {
                tracing::warn!(%slug, scope = other.as_str(), "[connectors] probe refused: not a read action");
                return Err(tinybus::Error::failed(format!(
                    "probe refuses `{slug}`: classified as {} - a probe is read-only and never \
                     performs a real mutation, whatever the user's scope preference. Use the \
                     tool contract for its schema-derived, possibly unknown, output shape.",
                    other.as_str()
                )));
            }
            None => {
                tracing::warn!(%slug, "[connectors] probe refused: no confirmed read scope");
                return Err(tinybus::Error::failed(format!(
                    "probe refuses `{slug}`: could not confirm it is a read action. Either no \
                     toolkit could be extracted from the slug, or its toolkit has a curated \
                     catalog and this slug is not in it. A probe never falls back to a verb \
                     heuristic there, since an uncurated action could really be a write."
                )));
            }
        }
        let Some(toolkit) = contracts::toolkit_of(slug) else {
            return Err(tinybus::Error::failed(format!(
                "probe: could not extract a toolkit from slug `{slug}`"
            )));
        };

        let connections = self
            .client()?
            .list_connections()
            .await
            .map_err(|error| to_bus_error(&error))?
            .connections;
        if !overview::active_toolkits(&connections).contains(&toolkit) {
            tracing::warn!(%slug, %toolkit, "[connectors] probe refused: toolkit not connected");
            return Err(tinybus::Error::failed(format!(
                "probe refuses `{slug}`: the `{toolkit}` toolkit has no active connection for \
                 this user. Connect it first, or fall back to the tool contract's \
                 schema-derived hint."
            )));
        }

        tracing::debug!(%slug, %toolkit, "[connectors] probe: one bounded read-only call");
        let arguments = if args.is_null() { None } else { Some(args) };
        let response = self
            .execute_checked(slug, arguments, None)
            .await
            .map_err(|error| {
                tinybus::Error::failed(format!("probe: real call to `{slug}` failed: {error}"))
            })?;
        if !response.successful {
            let detail = response
                .error
                .as_deref()
                .map(str::trim)
                .filter(|detail| !detail.is_empty())
                .unwrap_or("no error detail returned by the provider");
            return Err(tinybus::Error::failed(format!(
                "probe: `{slug}` reported failure at the provider: {detail}"
            )));
        }

        let envelope = serde_json::to_value(&response).map_err(|error| {
            tinybus::Error::failed(format!("probe: could not serialize the response: {error}"))
        })?;
        let sample = ComposioProbedOutputSample {
            primary_array_path: contracts::schema_walk::compute_primary_array_path_from_value(
                &envelope,
                contracts::ENVELOPE_META_KEYS_AT_ROOT,
            ),
            output_fields: response
                .data
                .as_object()
                .map(|object| object.keys().cloned().collect())
                .unwrap_or_default(),
            sample: envelope,
        };
        self.contract_caches.remember_probe(slug, &sample);
        tracing::info!(
            %slug,
            primary_array_path = ?sample.primary_array_path,
            "[connectors] probe: cached the observed output shape"
        );
        Ok(sample)
    }

    /// Assembles the overview for `mode`.
    ///
    /// Any failure that would make the answer misleading is an error, so the
    /// host does not cache a degraded overview: an empty allowlist cached after
    /// one 5xx would hide every integration until restart. The one tolerated
    /// failure is the direct route's tool listing, which only costs the tools.
    pub(super) async fn connected_integrations(
        &self,
        mode: ComposioConnectedIntegrationsMode,
    ) -> TinyBusResult<Vec<ComposioConnectedIntegration>> {
        let client = self.client()?;
        if client.route_name() != route_for(mode) {
            return Err(tinybus::Error::failed(format!(
                "connected integrations requested over the {mode:?} route, but this module is \
                 configured for `{}`",
                client.route_name()
            )));
        }

        let mut descriptions = HashMap::new();
        let (allowlist, connections, tools): (
            Vec<String>,
            Vec<ComposioConnection>,
            Vec<ComposioToolSchema>,
        ) = match mode {
            ComposioConnectedIntegrationsMode::Backend => {
                let toolkits = client.list_toolkits().await.map_err(|error| {
                    tracing::warn!(%error, "[connectors] overview: list_toolkits failed");
                    to_bus_error(&error)
                })?;
                descriptions = overview::catalog_descriptions(&toolkits.catalog);
                let allowlist =
                    overview::connectable_toolkit_slugs(&toolkits.toolkits, &toolkits.catalog);
                if allowlist.is_empty() {
                    tracing::debug!("[connectors] overview: backend allowlist is empty");
                    return Ok(Vec::new());
                }
                let connections = client
                    .list_connections()
                    .await
                    .map_err(|error| {
                        tracing::warn!(%error, "[connectors] overview: list_connections failed");
                        to_bus_error(&error)
                    })?
                    .connections;
                let connected = overview::active_toolkits(&connections);
                let tools = if connected.is_empty() {
                    Vec::new()
                } else {
                    client
                        .list_tools(&connected, &[])
                        .await
                        .map_err(|error| {
                            tracing::warn!(%error, "[connectors] overview: list_tools failed");
                            to_bus_error(&error)
                        })?
                        .tools
                };
                (allowlist, connections, tools)
            }
            ComposioConnectedIntegrationsMode::Direct => {
                // No central allowlist: the user's active connections are
                // the universe of toolkits.
                let connections = client
                        .list_connections()
                        .await
                        .map_err(|error| {
                            tracing::warn!(%error, "[connectors] overview: list_connections (direct) failed");
                            to_bus_error(&error)
                        })?
                        .connections;
                let allowlist = overview::active_toolkits(&connections);
                if allowlist.is_empty() {
                    tracing::info!(
                        "[connectors] overview: direct tenant has no active connections"
                    );
                    return Ok(Vec::new());
                }
                let tools = match client.list_tools(&allowlist, &[]).await {
                    Ok(listing) => listing.tools,
                    Err(error) => {
                        tracing::info!(%error, "[connectors] overview: direct list_tools failed; continuing without tools");
                        Vec::new()
                    }
                };
                (allowlist, connections, tools)
            }
        };

        let mut preferences = HashMap::new();
        for toolkit in overview::active_toolkits(&connections) {
            let preference = UserScopePref::load(self.prefs.as_ref(), &toolkit)
                .await
                .map_err(|error| tinybus::Error::failed(error.to_string()))?;
            preferences.insert(toolkit, preference);
        }

        let integrations = overview::assemble(&overview::Listings {
            allowlist,
            connections: &connections,
            tools: &tools,
            descriptions: &descriptions,
            preferences: &preferences,
        });
        tracing::info!(
            total = integrations.len(),
            connected = integrations.iter().filter(|i| i.connected).count(),
            "[connectors] overview: done"
        );
        Ok(integrations)
    }
}
