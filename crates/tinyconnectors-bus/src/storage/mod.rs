//! Pure storage names and collection declarations used by hosts.

mod names;
mod preference;

pub use names::{
    LEGACY_PREFS_NAMESPACE, LEGACY_STREAM_PREFIX, NON_OBJECT_FIELD, PREFS_NAMESPACE,
    PREFS_SCHEMA_VERSION, STREAM_NAME, STREAM_PREFIX,
};
pub use preference::{decode_preference, encode_preference};

use tinystoragedrivers_types::CollectionSpec;

/// The collections a host should provision for this module.
#[must_use]
pub fn collections() -> Vec<CollectionSpec> {
    vec![CollectionSpec::new(PREFS_NAMESPACE)]
}

/// Collections and stream prefixes the module may access through its storage grant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageNeeds {
    /// Document collections containing one preference per toolkit.
    pub collections: Vec<String>,
    /// Stream prefixes containing daily trigger archives.
    pub stream_prefixes: Vec<String>,
}

/// The storage names required by TinyConnectors.
#[must_use]
pub fn storage_needs() -> StorageNeeds {
    StorageNeeds {
        collections: vec![PREFS_NAMESPACE.to_owned()],
        stream_prefixes: vec![STREAM_PREFIX.to_owned()],
    }
}
