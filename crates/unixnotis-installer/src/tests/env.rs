//! Shared test helpers for process-wide environment mutation

use std::sync::{Mutex, MutexGuard, OnceLock};

pub(crate) fn test_env_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    // std::env is global to the whole test process, so every env test uses this same lock
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
