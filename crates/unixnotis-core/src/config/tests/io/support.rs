use std::env;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

pub(super) fn env_lock() -> std::sync::MutexGuard<'static, ()> {
    // Environment variables are process-global, so config path tests serialize access
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .expect("env lock")
}

pub(super) fn set_env(key: &str, value: Option<&str>) -> Option<String> {
    // Save the prior value so tests can restore process-global state exactly
    let previous = env::var(key).ok();
    match value {
        Some(value) => env::set_var(key, value),
        None => env::remove_var(key),
    }
    previous
}

pub(super) fn restore_env(key: &str, previous: Option<String>) {
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
