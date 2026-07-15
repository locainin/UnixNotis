use super::*;

#[test]
fn negative_icon_cache_expires_at_the_ttl_boundary() {
    let now = Instant::now();

    let fresh = now
        .checked_sub(Duration::from_secs(14))
        .expect("fresh timestamp should remain representable");
    let expired = now
        .checked_sub(NEGATIVE_ICON_CACHE_TTL)
        .expect("expired timestamp should remain representable");

    assert!(negative_cache_is_fresh(fresh, now));
    assert!(!negative_cache_is_fresh(expired, now));
}

#[test]
fn negative_icon_cache_handles_future_timestamp_without_panicking() {
    let now = Instant::now();

    assert!(negative_cache_is_fresh(now + Duration::from_secs(1), now));
}
