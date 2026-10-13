//! Live tool contracts, output probes and the connected-integration overview.
//!
//! This is the pure half of three bus members: shaping a backend tool listing
//! into [`ComposioToolContract`]s, caching them, classifying an action for the
//! read-only probe, and assembling the overview. Fetching goes through the
//! module's route and lives in `tinybus_module::contracts`, so everything here
//! runs without a network.
//!
//! # Why the caches live here
//!
//! A tool listing is one backend round trip per toolkit and a probe is a real,
//! possibly billed provider call. Callers ask for the same contract many times
//! in one authoring session, so the module remembers answers for
//! [`CATALOG_CACHE_TTL`]. The cache fails closed in one direction only: an
//! action published after the first fetch stays unknown until the entry
//! expires, which refuses an action rather than permitting an unknown one.

pub(crate) mod overview;
pub(crate) mod schema_walk;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use serde_json::Value;
use tinyconnectors_bus::composio::{
    ComposioProbedOutputSample, ComposioToolContract, ComposioToolsResponse,
};
use tinyconnectors_bus::vocabulary::ToolScope;

use crate::vocabulary_impl::catalogs::catalog_for_toolkit;
use crate::vocabulary_impl::scopes::{classify_unknown, find_curated, toolkit_from_slug};

/// How long a fetched catalog or a probe result is served from memory.
///
/// Long enough to outlast one authoring session of repeat lookups, short
/// enough that "add an action, come back later today" works without a restart.
pub(crate) const CATALOG_CACHE_TTL: Duration = Duration::from_secs(30 * 60);

/// Top-level fields of an execute response that are never part of the tool's
/// own payload, skipped at the root when walking a real response for its list.
pub(crate) const ENVELOPE_META_KEYS_AT_ROOT: &[&str] =
    &["successful", "error", "costUsd", "markdownFormatted"];

/// A map whose entries expire after a fixed time.
#[derive(Debug)]
pub(crate) struct TtlCache<V> {
    ttl: Duration,
    entries: Mutex<HashMap<String, (Instant, V)>>,
}

impl<V: Clone> TtlCache<V> {
    fn new(ttl: Duration) -> Self {
        Self {
            ttl,
            entries: Mutex::new(HashMap::new()),
        }
    }

    /// The value stored under `key`, unless it has expired.
    pub(crate) fn get(&self, key: &str) -> Option<V> {
        let entries = self.entries.lock().unwrap_or_else(PoisonError::into_inner);
        let (stored_at, value) = entries.get(key)?;
        (stored_at.elapsed() < self.ttl).then(|| value.clone())
    }

    /// Stores `value` under `key`, replacing any earlier entry.
    pub(crate) fn insert(&self, key: String, value: V) {
        self.entries
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(key, (Instant::now(), value));
    }

    fn clear(&self) {
        self.entries
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();
    }
}

/// Everything the contract members remember between calls.
#[derive(Debug)]
pub(crate) struct ContractCaches {
    /// Lowercase toolkit slug to every contract published for it.
    catalogs: TtlCache<Vec<ComposioToolContract>>,
    /// Uppercase action slug to its observed output shape. Entries never hold
    /// the raw sample; see [`ContractCaches::remember_probe`].
    probes: TtlCache<ComposioProbedOutputSample>,
    /// One lock per toolkit so concurrent misses share a single fetch.
    in_flight: Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
}

impl Default for ContractCaches {
    fn default() -> Self {
        Self::with_ttl(CATALOG_CACHE_TTL)
    }
}

impl ContractCaches {
    pub(crate) fn with_ttl(ttl: Duration) -> Self {
        Self {
            catalogs: TtlCache::new(ttl),
            probes: TtlCache::new(ttl),
            in_flight: Mutex::new(HashMap::new()),
        }
    }

    /// Forgets every cached catalog and probe.
    ///
    /// Called when the route changes: a catalog fetched for one tenant must not
    /// answer for the next.
    pub(crate) fn clear(&self) {
        self.catalogs.clear();
        self.probes.clear();
    }

    pub(crate) fn catalog(&self, toolkit: &str) -> Option<Vec<ComposioToolContract>> {
        self.catalogs.get(toolkit)
    }

    pub(crate) fn remember_catalog(&self, toolkit: &str, contracts: Vec<ComposioToolContract>) {
        self.catalogs.insert(toolkit.to_string(), contracts);
    }

    /// The lock a caller holds while it fetches `toolkit`, so the second caller
    /// waits and then reads the first caller's cached result.
    pub(crate) fn fetch_lock(&self, toolkit: &str) -> Arc<tokio::sync::Mutex<()>> {
        self.in_flight
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .entry(toolkit.to_string())
            .or_default()
            .clone()
    }

    /// Stores the derived shape of a real probe, never the raw sample.
    ///
    /// The cache is keyed by slug alone, and a real response can hold one
    /// user's private data (issues, messages). Only the derived path and field
    /// names are read back, so the payload is redacted before it is stored.
    pub(crate) fn remember_probe(&self, slug: &str, sample: &ComposioProbedOutputSample) {
        let redacted = ComposioProbedOutputSample {
            sample: Value::Null,
            ..sample.clone()
        };
        self.probes
            .insert(slug.trim().to_ascii_uppercase(), redacted);
    }

    pub(crate) fn probe(&self, slug: &str) -> Option<ComposioProbedOutputSample> {
        self.probes.get(&slug.trim().to_ascii_uppercase())
    }

    /// Lays a cached probe over a schema-derived contract.
    ///
    /// A probe is an observed response, so it wins over a published schema.
    /// `primary_array_path` is replaced even with `None`: "the real response
    /// named no array" is itself an observation, and keeping a stale schema
    /// path would let a disproved path keep looking supported. `output_fields`
    /// is replaced only when the probe found some, because an empty probe
    /// result means unknown rather than confirmed empty.
    pub(crate) fn apply_probe(&self, mut contract: ComposioToolContract) -> ComposioToolContract {
        if let Some(probe) = self.probe(&contract.slug) {
            contract.primary_array_path = probe.primary_array_path;
            if !probe.output_fields.is_empty() {
                contract.output_fields = probe.output_fields;
            }
        }
        contract
    }
}

/// The path to the first list in a real tool result, with the execute
/// envelope's `data` wrapper accounted for.
///
/// The schema a backend publishes describes only the payload, the value under
/// `data`. A path walked over it is therefore missing the leading `data.`
/// segment a binding needs. It is added unconditionally: a payload that itself
/// has a top-level `data` property really lives at `data.data...`.
pub(crate) fn compute_composio_array_path(schema: Option<&Value>) -> Option<String> {
    let path = schema_walk::compute_primary_array_path(schema)?;
    Some(format!("data.{path}"))
}

/// Maps a backend tool listing for `toolkit` into contracts.
///
/// `toolkit` must already be normalized (trimmed, lowercase).
pub(crate) fn contracts_from_listing(
    toolkit: &str,
    listing: &ComposioToolsResponse,
) -> Vec<ComposioToolContract> {
    let curated = catalog_for_toolkit(toolkit);
    listing
        .tools
        .iter()
        .map(|tool| {
            let function = &tool.function;
            let required_args = function
                .parameters
                .as_ref()
                .and_then(|parameters| parameters.get("required"))
                .and_then(Value::as_array)
                .map(|names| {
                    names
                        .iter()
                        .filter_map(|name| name.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            ComposioToolContract {
                slug: function.name.clone(),
                toolkit: toolkit.to_string(),
                description: function.description.clone(),
                required_args,
                input_schema: function.parameters.clone(),
                output_fields: schema_walk::response_fields_from_schema(
                    function.output_parameters.as_ref(),
                ),
                output_schema: function.output_parameters.clone(),
                primary_array_path: compute_composio_array_path(
                    function.output_parameters.as_ref(),
                ),
                is_curated: curated
                    .is_some_and(|catalog| find_curated(catalog, &function.name).is_some()),
            }
        })
        .collect()
}

/// Whether `slug` is a confirmed read action, failing closed.
///
/// - A slug with no extractable toolkit has no scope at all.
/// - A toolkit with a curated catalog confirms only the catalog's own entries.
///   An uncurated slug there is `None` and never falls through to the verb
///   heuristic, which would call an uncurated write action a read by name.
/// - A toolkit with no catalog falls back to the verb heuristic.
///
/// The probe refuses anything but a confirmed read whatever the user's scope
/// preference says: the user never approved this specific call.
pub(crate) fn resolve_action_scope(slug: &str) -> Option<ToolScope> {
    let toolkit = toolkit_from_slug(slug)?;
    match catalog_for_toolkit(&toolkit) {
        Some(catalog) => find_curated(catalog, slug).map(|curated| curated.scope),
        None => Some(classify_unknown(slug)),
    }
}

/// The toolkit a slug belongs to, as the module resolves it.
pub(crate) fn toolkit_of(slug: &str) -> Option<String> {
    toolkit_from_slug(slug)
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
