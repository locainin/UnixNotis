use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};

pub(super) fn trusted_program_path(program: &str) -> Option<PathBuf> {
    if program.is_empty() || program.contains(std::path::MAIN_SEPARATOR) {
        return None;
    }
    if fake_tool_bin_is_set() {
        return fake_program_path(program);
    }
    super::lookup::trusted_program_path(program)
}

fn executable_file(path: &Path) -> bool {
    let Ok(metadata) = path.metadata() else {
        return false;
    };
    metadata.is_file() && executable_mode(&metadata)
}

#[cfg(unix)]
fn executable_mode(metadata: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;

    metadata.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn executable_mode(_metadata: &std::fs::Metadata) -> bool {
    true
}

fn fake_tool_bin_is_set() -> bool {
    lock_fake_tool_bin().is_some()
}

fn fake_program_path(program: &str) -> Option<PathBuf> {
    let configured_bin = lock_fake_tool_bin().clone()?;
    let candidate = configured_bin.join(program);
    executable_file(&candidate).then_some(candidate)
}

pub struct FakeToolBinGuard {
    _lock: MutexGuard<'static, ()>,
    previous: Option<PathBuf>,
}

impl Drop for FakeToolBinGuard {
    fn drop(&mut self) {
        *lock_fake_tool_bin() = self.previous.take();
    }
}

pub fn use_fake_tool_bin(path: &Path) -> FakeToolBinGuard {
    // Recovering a poisoned fixture lock preserves isolation after another test unwinds
    let lock = fake_tool_bin_test_lock()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut fake_bin = lock_fake_tool_bin();
    let previous = fake_bin.replace(path.to_path_buf());
    FakeToolBinGuard {
        _lock: lock,
        previous,
    }
}

fn fake_tool_bin() -> &'static Mutex<Option<PathBuf>> {
    static FAKE_TOOL_BIN: OnceLock<Mutex<Option<PathBuf>>> = OnceLock::new();
    FAKE_TOOL_BIN.get_or_init(|| Mutex::new(None))
}

fn lock_fake_tool_bin() -> MutexGuard<'static, Option<PathBuf>> {
    fake_tool_bin()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn fake_tool_bin_test_lock() -> &'static Mutex<()> {
    static FAKE_TOOL_BIN_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    FAKE_TOOL_BIN_TEST_LOCK.get_or_init(|| Mutex::new(()))
}
