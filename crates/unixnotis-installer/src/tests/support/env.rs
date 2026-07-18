//! Shared test helpers for process-wide environment mutation

use std::ffi::{OsStr, OsString};
use std::sync::{Mutex, MutexGuard, OnceLock};

pub fn test_env_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    // std::env is global to the whole test process, so every env test uses this same lock
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

pub struct EnvGuard {
    // Static names prevent a borrowed key from outliving the guard
    name: &'static str,
    // None records that the variable did not exist before the test
    original: Option<OsString>,
}

impl EnvGuard {
    pub fn set(name: &'static str, value: impl AsRef<OsStr>) -> Self {
        let original = std::env::var_os(name);
        std::env::set_var(name, value);
        Self { name, original }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        // Restore process-wide state even when a test returns through an assertion panic
        match &self.original {
            Some(value) => std::env::set_var(self.name, value),
            None => std::env::remove_var(self.name),
        }
    }
}
