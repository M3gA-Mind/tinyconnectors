//! Archive leases are bounded, explicit and reject unavailable resources.
#![allow(clippy::expect_used)]
use super::*;

fn register(key: &str) {
    use tinystoragedrivers_core::StorageBackend;
    let storage = tinystoragedrivers_core::MemoryStorage::new()
        .for_scope(&tinystoragedrivers_core::Scope::new("lease").expect("scope"))
        .expect("scoped");
    crate::state::register_storage(
        key,
        crate::state::ModuleStorage {
            documents: storage.documents().clone(),
            streams: storage.streams().clone(),
        },
    );
}

#[test]
fn lease_limit_releases_capacity_and_unregistered_storage_is_refused() -> tinybus::Result<()> {
    let key = "archives-lease-capacity";
    register(key);
    let archives = Archives::default();
    let mut handles = Vec::new();
    for _ in 0..MAX_ARCHIVES {
        handles.push(archives.open(key)?);
    }
    assert!(archives.open(key).is_err());
    archives.close(&handles[0])?;
    assert!(archives.open(key).is_ok());
    let unavailable = Archives::default();
    let error = unavailable
        .open("archives-never-registered")
        .err()
        .ok_or_else(|| tinybus::Error::failed("expected fault"))?;
    assert!(!error.to_string().contains("archives-never-registered"));
    crate::state::unregister_storage(key);
    Ok(())
}

#[test]
fn poisoned_archive_state_fails_all_operations_without_panicking() {
    let archives = Archives::default();
    let _ = std::panic::catch_unwind(|| {
        let _guard = archives.0.lock();
        // A caught fixture panic models a failed previous owner; production
        // refuses the poisoned state instead of recovering corrupted leases.
        std::panic::resume_unwind(Box::new("fixture poison"));
    });
    let handle = ArchiveHandle("unknown".into());
    assert!(archives.open("unused").is_err());
    assert!(archives.get(&handle).is_err());
    assert!(archives.close(&handle).is_err());
}
