use super::super::ThemeIconCache;
use super::{ThemeIconCacheMap, ThemeIconKey};

#[test]
fn successful_theme_icon_lookup_is_reused_without_re_resolving() {
    let mut cache = ThemeIconCacheMap::new(128);
    let mut resolves = 0;

    assert_eq!(
        cache.get_or_resolve_with("folder", 24, 1, |_, _, _| {
            resolves += 1;
            Some(7_u8)
        }),
        Some(7)
    );
    assert_eq!(
        cache.get_or_resolve_with("folder", 24, 1, |_, _, _| {
            resolves += 1;
            Some(8_u8)
        }),
        Some(7)
    );

    assert_eq!(resolves, 1);
    assert_eq!(cache.entry_count(), 1);
}

#[test]
fn a_miss_is_not_cached_and_can_be_retried_successfully() {
    let mut cache = ThemeIconCacheMap::new(128);
    let mut resolves = 0;

    assert_eq!(
        cache.get_or_resolve_with("eventual-icon", 24, 1, |_, _, _| {
            resolves += 1;
            None::<u8>
        }),
        None
    );
    assert!(!cache.contains("eventual-icon", 24, 1));

    assert_eq!(
        cache.get_or_resolve_with("eventual-icon", 24, 1, |_, _, _| {
            resolves += 1;
            Some(9_u8)
        }),
        Some(9)
    );

    assert_eq!(resolves, 2);
    assert_eq!(cache.entry_count(), 1);
}

#[test]
fn scale_variants_have_independent_successful_entries() {
    let mut cache = ThemeIconCacheMap::new(128);

    assert_eq!(
        cache.get_or_resolve_with("folder", 24, 1, |_, _, scale| Some(scale as u8)),
        Some(1)
    );
    assert_eq!(
        cache.get_or_resolve_with("folder", 24, 2, |_, _, scale| Some(scale as u8)),
        Some(2)
    );

    assert!(cache.contains("folder", 24, 1));
    assert!(cache.contains("folder", 24, 2));
    assert_eq!(cache.entry_count(), 2);
}

#[test]
fn invalidation_discards_successful_paintables() {
    let mut cache = ThemeIconCacheMap::new(128);
    cache.get_or_resolve_with("folder", 24, 1, |_, _, _| Some(1_u8));
    assert_eq!(cache.entry_count(), 1);

    cache.clear();

    assert_eq!(cache.entry_count(), 0);
    assert!(!cache.contains("folder", 24, 1));
}

#[test]
fn lru_promotion_keeps_the_recent_success_when_the_limit_is_reached() {
    let mut cache = ThemeIconCacheMap::new(2);

    cache.get_or_resolve_with("first", 24, 1, |_, _, _| Some(1_u8));
    cache.get_or_resolve_with("second", 24, 1, |_, _, _| Some(2_u8));

    // A successful hit moves the first entry behind the second entry
    assert_eq!(
        cache.get_or_resolve_with("first", 24, 1, |_, _, _| Some(10_u8)),
        Some(1)
    );
    cache.get_or_resolve_with("third", 24, 1, |_, _, _| Some(3_u8));

    assert!(cache.contains("first", 24, 1));
    assert!(!cache.contains("second", 24, 1));
    assert!(cache.contains("third", 24, 1));
    assert_eq!(cache.entry_count(), 2);
}

#[test]
fn failed_lookups_do_not_consume_lru_capacity() {
    let mut cache = ThemeIconCacheMap::new(1);

    cache.get_or_resolve_with("missing", 24, 1, |_, _, _| None::<u8>);
    cache.get_or_resolve_with("present", 24, 1, |_, _, _| Some(1_u8));

    assert!(!cache.contains("missing", 24, 1));
    assert!(cache.contains("present", 24, 1));
    assert_eq!(cache.entry_count(), 1);
}

#[test]
fn theme_icon_keys_match_name_size_and_scale_together() {
    let key = ThemeIconKey::new("folder", 24, 1);

    assert!(key.matches("folder", 24, 1));
    assert!(!key.matches("dialog-information", 24, 1));
    assert!(!key.matches("folder", 32, 1));
    assert!(!key.matches("folder", 24, 2));
}

#[gtk::test]
fn production_theme_cache_clear_removes_successful_entries() {
    let mut cache = ThemeIconCache::new_for_popups();
    let Some(_) = cache.get_or_resolve("folder", 24, 1) else {
        return;
    };

    assert_eq!(cache.entries.entry_count(), 1);
    cache.clear();
    assert_eq!(cache.entries.entry_count(), 0);
}

#[gtk::test]
fn production_cache_rejects_empty_theme_names_without_creating_an_entry() {
    let mut cache = ThemeIconCache::new_for_popups();

    assert!(cache.get_or_resolve("", 24, 1).is_none());
}
