use super::{
    discovery::{RELEASE_BIN_DIR, RELEASE_MANIFEST_FILE},
    format_with_home, is_unixnotis_release_archive, is_unixnotis_repo, InstallPaths,
    ServiceManagerChoice,
};
use std::env;
use std::fs;
use std::path::PathBuf;

fn env_lock() -> std::sync::MutexGuard<'static, ()> {
    // Path discovery tests share the crate-wide env lock with checks and flow tests
    crate::test_support::env::test_env_lock()
}

fn set_env(key: &str, value: Option<&str>) -> Option<String> {
    let previous = env::var(key).ok();
    match value {
        // Store test values through std::env so InstallPaths uses the real production path
        Some(value) => env::set_var(key, value),
        None => env::remove_var(key),
    }
    previous
}

fn restore_env(key: &str, previous: Option<String>) {
    match previous {
        // Restore every variable explicitly to keep later path tests independent
        Some(value) => env::set_var(key, value),
        None => env::remove_var(key),
    }
}

// Keep path discovery tests split by behavior so backend-specific rules do not pile up here
mod general;
mod runit;
mod s6_data;
mod s6_live;
mod systemd_dinit;
