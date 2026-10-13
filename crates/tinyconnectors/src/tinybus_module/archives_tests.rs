//! Archive leases are bounded and explicit; unknown or closed handles fail.
#![allow(clippy::expect_used)]
use super::*;

fn streams() -> Arc<dyn StreamStore> {
    use tinystoragedrivers_core::StorageBackend;
    tinystoragedrivers_core::MemoryStorage::new()
        .for_scope(&tinystoragedrivers_core::Scope::new("lease").expect("scope"))
        .expect("scoped")
        .streams()
        .clone()
}

#[test]
fn lease_limit_releases_capacity() -> tinybus::Result<()> {
    let archives = Archives::default();
    let mut handles = Vec::new();
    for _ in 0..MAX_ARCHIVES {
        handles.push(archives.open(streams())?);
    }
    assert!(archives.open(streams()).is_err());
    archives.close(&handles[0])?;
    assert!(archives.open(streams()).is_ok());
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
    assert!(archives.open(streams()).is_err());
    assert!(archives.get(&handle).is_err());
    assert!(archives.close(&handle).is_err());
}
