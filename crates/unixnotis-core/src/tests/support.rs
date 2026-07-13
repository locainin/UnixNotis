//! Shared helpers for core crate unit tests

use std::env;
use std::ffi::{OsStr, OsString};
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn test_env_lock() -> MutexGuard<'static, ()> {
    // Environment variables are process-global, so every env-mutating core test shares one lock
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .expect("core test env lock should not be poisoned")
}

pub fn unique_temp_path(name: &str) -> PathBuf {
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

pub struct EnvGuard {
    key: &'static str,
    previous: Option<OsString>,
}

impl EnvGuard {
    pub(crate) fn set(key: &'static str, value: impl AsRef<OsStr>) -> Self {
        let previous = set_env(key, Some(value.as_ref()));
        Self { key, previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        // Restore even after a panic so later tests see the original process env
        restore_env(self.key, self.previous.take());
    }
}

pub fn set_env(key: &str, value: Option<&OsStr>) -> Option<OsString> {
    let previous = env::var_os(key);
    match value {
        Some(value) => env::set_var(key, value),
        None => env::remove_var(key),
    }
    previous
}

pub fn restore_env(key: &str, previous: Option<OsString>) {
    match previous {
        Some(value) => env::set_var(key, value),
        None => env::remove_var(key),
    }
}
