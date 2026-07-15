use std::time::{Duration, Instant};

use super::{MissingIconCache, MISSING_ICON_TTL};
use crate::ui::icons::cache::IconKey;

fn key(name: &str) -> IconKey {
    IconKey::Name {
        name: name.to_string(),
        size: 24,
        scale: 1,
    }
}

#[test]
fn missing_icon_cache_evicts_oldest_entry_at_capacity() {
    let mut cache = MissingIconCache::new(1);
    let first = key("first");
    let second = key("second");

    cache.insert(first.clone());
    cache.insert(second.clone());

    assert!(!cache.contains(&first));
    assert!(cache.contains(&second));
}

#[test]
fn missing_icon_cache_removes_expired_entries() {
    let mut cache = MissingIconCache::new(2);
    let expired = key("expired");
    cache.set.insert(expired.clone());
    let expired_at = Instant::now()
        .checked_sub(MISSING_ICON_TTL + Duration::from_millis(1))
        .expect("expired timestamp should remain representable");
    cache.order.push_back((expired.clone(), expired_at));

    assert!(!cache.contains(&expired));
    assert!(cache.order.is_empty());
}

#[test]
fn missing_icon_cache_expires_at_the_exact_ttl_boundary() {
    let mut cache = MissingIconCache::new(2);
    let expired = key("boundary");
    let now = Instant::now();
    let expired_at = now
        .checked_sub(MISSING_ICON_TTL)
        .expect("boundary timestamp should remain representable");
    cache.set.insert(expired.clone());
    cache.order.push_back((expired.clone(), expired_at));

    cache.purge_expired(now);

    assert!(!cache.set.contains(&expired));
    assert!(cache.order.is_empty());
}

#[test]
fn clear_removes_order_and_membership_state() {
    let mut cache = MissingIconCache::new(2);
    let stored = key("stored");
    cache.insert(stored.clone());

    cache.clear();

    assert!(!cache.contains(&stored));
    assert!(cache.order.is_empty());
}
