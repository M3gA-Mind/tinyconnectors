//! Tests the storage names required by the module's grant.

use super::*;

#[test]
fn declarations_are_pure_and_cover_the_owner_migration_records() {
    assert_eq!(
        collections()
            .iter()
            .map(|collection| collection.name.as_str())
            .collect::<Vec<_>>(),
        [PREFS_NAMESPACE, MIGRATIONS_NAMESPACE]
    );
    let needs = storage_needs();
    assert_eq!(
        needs.collections,
        [
            PREFS_NAMESPACE,
            MIGRATIONS_NAMESPACE,
            LEGACY_PREFS_NAMESPACE
        ]
    );
    assert_eq!(needs.stream_prefixes, [STREAM_PREFIX, LEGACY_STREAM_PREFIX]);
}
