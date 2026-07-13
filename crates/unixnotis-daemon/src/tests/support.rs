//! Shared test helpers for daemon modules that touch process-global state

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use std::sync::Arc;

use unixnotis_core::Config;
use zbus::Connection;

use crate::daemon::DaemonState;
use crate::sound::SoundSettings;
use crate::store::NotificationStore;

pub fn env_lock() -> MutexGuard<'static, ()> {
    // Environment variables are process-global, so all tests that edit them share one lock
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .expect("env lock should not be poisoned")
}

pub async fn daemon_state_for_test(trial_mode: bool) -> Arc<DaemonState> {
    // Signal-heavy daemon tests only need a session connection and default state
    let connection = Connection::session()
        .await
        .expect("session bus should be available for daemon signal tests");
    let config = Config::default();
    let sound = SoundSettings::from_config(&config);
    let store = NotificationStore::new_with_state_store(config, None);
    DaemonState::new_with_store(connection, store, sound, trial_mode)
}

pub struct EnvVarGuard {
    name: &'static str,
    previous: Option<std::ffi::OsString>,
}

impl EnvVarGuard {
    pub(super) fn set(name: &'static str, value: impl AsRef<std::ffi::OsStr>) -> Self {
        let previous = std::env::var_os(name);
        std::env::set_var(name, value);
        Self { name, previous }
    }

    pub(super) fn remove(name: &'static str) -> Self {
        let previous = std::env::var_os(name);
        std::env::remove_var(name);
        Self { name, previous }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        // Restore the process environment even when a test panics
        match &self.previous {
            Some(value) => std::env::set_var(self.name, value),
            None => std::env::remove_var(self.name),
        }
    }
}

pub struct TempRoot {
    path: PathBuf,
}

impl TempRoot {
    pub(super) fn new(label: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "unixnotis-daemon-{label}-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).expect("create temp root");
        Self { path }
    }

    pub(super) fn path(&self) -> &Path {
        &self.path
    }

    pub(super) fn join(&self, path: impl AsRef<Path>) -> PathBuf {
        self.path.join(path)
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        // Tests create only project-owned temp roots, so recursive cleanup is safe here
        let _ = std::fs::remove_dir_all(&self.path);
    }
}
