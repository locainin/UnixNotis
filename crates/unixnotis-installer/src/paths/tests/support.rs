pub(super) use super::super::{
    discovery::{
        is_unixnotis_release_archive, is_unixnotis_repo, service_manager_choice_from_environment,
        RELEASE_BIN_DIR, RELEASE_MANIFEST_FILE,
    },
    format_with_home, InstallPaths, ServiceManagerChoice,
};
pub(super) use std::env;
pub(super) use std::fs;
pub(super) use std::path::PathBuf;

pub(super) fn env_lock() -> std::sync::MutexGuard<'static, ()> {
    // Path discovery tests share the crate-wide env lock with checks and flow tests
    crate::test_support::env::test_env_lock()
}

pub(super) fn set_env(key: &str, value: Option<&str>) -> Option<String> {
    let previous = env::var(key).ok();
    match value {
        // Store test values through std::env so InstallPaths uses the real production path
        Some(value) => env::set_var(key, value),
        None => env::remove_var(key),
    }
    previous
}

pub(super) fn restore_env(key: &str, previous: Option<String>) {
    match previous {
        // Restore every variable explicitly to keep later path tests independent
        Some(value) => env::set_var(key, value),
        None => env::remove_var(key),
    }
}
