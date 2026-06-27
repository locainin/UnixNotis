use super::{format_with_home, is_unixnotis_repo, InstallPaths, ServiceManagerChoice};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

fn env_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    // Path discovery tests mutate process-wide env, so they must run under one lock
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .expect("env lock")
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
#[path = "paths/general.rs"]
mod general;
#[path = "paths/runit.rs"]
mod runit;
#[path = "paths/s6_data.rs"]
mod s6_data;
#[path = "paths/s6_live.rs"]
mod s6_live;
#[path = "paths/systemd_dinit.rs"]
mod systemd_dinit;
