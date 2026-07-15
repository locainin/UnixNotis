use std::time::Instant;

use super::IconCacheEntry;

#[test]
fn unresolved_icon_cache_entry_preserves_negative_lookup_state() {
    let cached_at = Instant::now();
    let entry = IconCacheEntry {
        resolved: None,
        cached_at,
    };

    assert!(entry.resolved.is_none());
    assert_eq!(entry.cached_at, cached_at);
}
