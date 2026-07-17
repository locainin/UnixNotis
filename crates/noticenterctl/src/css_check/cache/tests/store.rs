use super::super::store::default_css_parse_cache_path;
use crate::test_support::{test_env_lock, EnvGuard};
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
