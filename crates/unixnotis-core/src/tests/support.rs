//! Shared helpers for core crate unit tests

use std::env;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) fn test_env_lock() -> MutexGuard<'static, ()> {
    // Environment variables are process-global, so every env-mutating core test shares one lock
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .expect("core test env lock should not be poisoned")
}

pub(crate) fn unique_temp_path(name: &str) -> PathBuf {
    // Mix the process id with a timestamp so parallel test binaries do not collide
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after Unix epoch")
        .as_nanos();
    env::temp_dir().join(format!(
        "unixnotis-core-{name}-{}-{stamp}",
        std::process::id()
    ))
}

pub(crate) fn set_env(key: &str, value: Option<&str>) -> Option<String> {
    let previous = env::var(key).ok();
    match value {
        Some(value) => env::set_var(key, value),
        None => env::remove_var(key),
    }
    previous
}

pub(crate) fn restore_env(key: &str, previous: Option<String>) {
    match previous {
        Some(value) => env::set_var(key, value),
        None => env::remove_var(key),
    }
}
