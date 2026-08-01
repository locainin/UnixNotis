use std::path::PathBuf;
use std::time::Instant;

use super::super::cache::{
    CachedProvenance, NegativeCause, OwnershipLookup, PackageOwnershipCache,
    NOT_OWNED_NEGATIVE_TTL, TRANSIENT_NEGATIVE_TTL,
};
use super::super::{InstallProvenance, PackageProvider};

#[test]
fn transient_ownership_failures_expire_before_confirmed_not_owned_entries() {
    let now = Instant::now();
    let transient =
        CachedProvenance::from_lookup(OwnershipLookup::Negative(NegativeCause::Timeout), now);
    let not_owned =
        CachedProvenance::from_lookup(OwnershipLookup::Negative(NegativeCause::NotOwned), now);

    assert!(!transient.needs_refresh(now));
    assert!(transient.needs_refresh(now + TRANSIENT_NEGATIVE_TTL));
    assert!(!not_owned.needs_refresh(now + TRANSIENT_NEGATIVE_TTL));
    assert!(not_owned.needs_refresh(now + NOT_OWNED_NEGATIVE_TTL));
    assert_eq!(transient.provenance(), InstallProvenance::Unknown);
}

#[test]
fn cached_known_provenance_is_returned_for_every_requested_path() {
    let path = PathBuf::from("/usr/bin/example-cache-entry");
    let expected = InstallProvenance::Package {
        provider: PackageProvider::Pacman,
        package_id: "example-cache-entry".to_string(),
    };
    let cache = PackageOwnershipCache::default();
    cache
        .entries
        .lock()
        .expect("package cache should be writable")
        .insert(path.clone(), CachedProvenance::Known(expected.clone()));

    let resolved = cache.resolve_many([path.clone()]);

    assert_eq!(resolved.len(), 1);
    assert_eq!(resolved.get(&path), Some(&expected));
}
