use super::super::store::{
    default_css_parse_cache_path, CSS_PARSE_CACHE_MAX_BYTES, CSS_PARSE_CACHE_MAX_ENTRIES,
};
use super::helpers::{validate_with_counter, TempDirGuard};
use crate::test_support::{test_env_lock, EnvGuard};
use std::cell::Cell;
use std::fs;
use std::path::PathBuf;

#[test]
fn absolute_xdg_cache_home_selects_the_css_cache_root() {
    let _lock = test_env_lock();
    let _xdg = EnvGuard::set("XDG_CACHE_HOME", "/tmp/unixnotis-cache-home");
    let _home = EnvGuard::set("HOME", "/tmp/unixnotis-home");

    assert_eq!(
        default_css_parse_cache_path(),
        Some(PathBuf::from(
            "/tmp/unixnotis-cache-home/unixnotis/css-check-parse-cache-v2.json"
        ))
    );
}

#[test]
fn relative_xdg_cache_home_falls_back_to_the_home_cache() {
    let _lock = test_env_lock();
    let _xdg = EnvGuard::set("XDG_CACHE_HOME", "relative/cache");
    let _home = EnvGuard::set("HOME", "/tmp/unixnotis-home");

    assert_eq!(
        default_css_parse_cache_path(),
        Some(PathBuf::from(
            "/tmp/unixnotis-home/.cache/unixnotis/css-check-parse-cache-v2.json"
        ))
    );
}

#[test]
fn oversized_cache_file_is_discarded_and_replaced_with_bounded_state() {
    let root = TempDirGuard::new("oversized-file");
    let css_path = root.write("config/base.css", "clean");
    let cache_path = root.path().join("cache.json");
    fs::write(&cache_path, vec![b'x'; CSS_PARSE_CACHE_MAX_BYTES + 1])
        .expect("write oversized cache");
    let invocations = Cell::new(0usize);

    validate_with_counter(&invocations, &[css_path], root.path(), &cache_path)
        .expect("validate with oversized optional cache");

    assert_eq!(invocations.get(), 1);
    assert!(
        fs::metadata(cache_path).expect("cache metadata").len()
            <= u64::try_from(CSS_PARSE_CACHE_MAX_BYTES).expect("cache limit fits u64")
    );
}

#[test]
fn cache_entry_limit_evicts_the_least_recently_used_path() {
    let root = TempDirGuard::new("entry-lru");
    let cache_path = root.path().join("cache.json");
    let css_paths = (0..=CSS_PARSE_CACHE_MAX_ENTRIES)
        .map(|index| root.write(&format!("config/{index}.css"), "clean"))
        .collect::<Vec<_>>();
    let invocations = Cell::new(0usize);

    validate_with_counter(&invocations, &css_paths, root.path(), &cache_path)
        .expect("populate bounded cache");
    assert_eq!(invocations.get(), CSS_PARSE_CACHE_MAX_ENTRIES + 1);

    let cache: serde_json::Value =
        serde_json::from_slice(&fs::read(&cache_path).expect("read bounded cache"))
            .expect("parse bounded cache");
    assert_eq!(
        cache["entries"]
            .as_object()
            .expect("cache entries object")
            .len(),
        CSS_PARSE_CACHE_MAX_ENTRIES
    );

    invocations.set(0);
    validate_with_counter(
        &invocations,
        std::slice::from_ref(css_paths.last().expect("newest path")),
        root.path(),
        &cache_path,
    )
    .expect("reuse newest cache entry");
    assert_eq!(invocations.get(), 0);

    validate_with_counter(
        &invocations,
        std::slice::from_ref(css_paths.first().expect("oldest path")),
        root.path(),
        &cache_path,
    )
    .expect("reparse evicted oldest entry");
    assert_eq!(invocations.get(), 1);
}
