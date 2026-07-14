//! Shared popup test helpers
//!
//! GTK and GDK register types in process-global tables. Rust tests run in
//! parallel by default, so every test that creates GTK/GDK objects must pass
//! through the same lock to avoid racing type registration

use std::sync::{Mutex, MutexGuard, OnceLock};

pub fn gtk_test_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    // A poisoned lock means a GTK-touching test already failed
    // Continuing would make the next failure harder to diagnose
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .expect("GTK test lock should not be poisoned")
}
