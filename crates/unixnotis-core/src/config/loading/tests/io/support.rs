use std::env;
use std::ffi::{OsStr, OsString};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::test_support::test_env_lock;

pub(super) fn env_lock() -> std::sync::MutexGuard<'static, ()> {
    // Environment variables are process-global, so config path tests serialize access
    test_env_lock()
}

pub(super) struct EnvGuard {
    key: &'static str,
    previous: Option<OsString>,
}

impl EnvGuard {
    pub(super) fn set(key: &'static str, value: impl AsRef<OsStr>) -> Self {
        let previous = set_env(key, Some(value.as_ref()));
        Self { key, previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        // Panic paths must restore process-global env before the next test runs
        restore_env(self.key, self.previous.take());
    }
}

fn set_env(key: &str, value: Option<&OsStr>) -> Option<OsString> {
    // Preserve raw OS strings so non-UTF-8 environment values survive tests
    let previous = env::var_os(key);
    match value {
        Some(value) => env::set_var(key, value),
        None => env::remove_var(key),
    }
    previous
}

fn restore_env(key: &str, previous: Option<OsString>) {
    // Restoring through one helper keeps remove-vs-set behavior consistent
    match previous {
        Some(value) => env::set_var(key, value),
        None => env::remove_var(key),
    }
}

pub(super) fn test_root(name: &str) -> PathBuf {
    // Target-local temp roots make cleanup cheap and avoid touching user config
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    std::env::current_dir()
        .expect("current dir")
        .join("target")
        .join(format!("unixnotis-{name}-{}-{unique}", std::process::id()))
}
