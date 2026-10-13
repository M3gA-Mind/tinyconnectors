//! What the module needs the host to grant on its storage bus.
//!
//! The module reaches storage only through the host's storage bus, and the host
//! decides per module which collections and stream names it may touch. A grant
//! that is narrower than this list makes the module's calls fail closed, so the
//! list is the contract a host writes its grant from rather than something to
//! rediscover by reading the code. Only the host runs `ensure_collection`;
//! the module never creates a collection.

use tinyconnectors_sync::PREFS_NAMESPACE;

/// Collections and stream prefixes the module reads and writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StorageNeeds {
    /// Document collections: one document per toolkit, body `{ "value": … }`.
    pub collections: &'static [&'static str],
    /// Stream-name prefixes: one stream per UTC day, `triggers/<YYYY-MM-DD>`.
    pub stream_prefixes: &'static [&'static str],
}

/// The grant the host gives this module.
pub const STORAGE_NEEDS: StorageNeeds = StorageNeeds {
    collections: &[PREFS_NAMESPACE],
    stream_prefixes: &[crate::triggers::STREAM_PREFIX],
};
