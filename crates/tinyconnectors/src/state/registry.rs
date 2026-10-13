//! The storage a module instance is given, looked up by the host's key.
//!
//! A TinyBus module is configured with JSON, which cannot carry a trait
//! object. A host that links the crate registers the ports it owns under a key
//! (the module configuration's `state_dir`, kept as the key for wire
//! compatibility, and an archive lease's `state_dir`), and the module resolves
//! the ports from that key. A key nobody registered resolves to nothing, which
//! the module reports as "storage unavailable" rather than inventing a place to
//! write.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use tinystoragedrivers_core::{DocumentStore, StreamStore};

/// The ports one user's connector state is written through.
#[derive(Debug, Clone)]
pub struct ModuleStorage {
    /// Scope preferences.
    pub documents: Arc<dyn DocumentStore>,
    /// The trigger archive.
    pub streams: Arc<dyn StreamStore>,
}

static REGISTRY: Mutex<Option<HashMap<String, ModuleStorage>>> = Mutex::new(None);

/// Make `storage` the ports modules configured with `key` resolve to.
pub fn register_storage(key: impl Into<String>, storage: ModuleStorage) {
    REGISTRY
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get_or_insert_with(HashMap::new)
        .insert(key.into(), storage);
}

/// Forget the storage registered under `key`.
pub fn unregister_storage(key: &str) {
    if let Some(map) = REGISTRY
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_mut()
    {
        map.remove(key);
    }
}

/// The storage registered under `key`, if any.
#[must_use]
pub fn resolve_storage(key: &str) -> Option<ModuleStorage> {
    REGISTRY
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_ref()
        .and_then(|map| map.get(key).cloned())
}
