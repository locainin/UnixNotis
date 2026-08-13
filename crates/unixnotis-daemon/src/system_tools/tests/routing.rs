use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};

pub(super) fn trusted_program_path(program: &str) -> Option<PathBuf> {
    if program.is_empty() || program.contains(std::path::MAIN_SEPARATOR) {
        return None;
    }
    // Unit tests never resolve or launch tools installed on the host
    fake_program_path(program)
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

fn fake_program_path(program: &str) -> Option<PathBuf> {
    let fake_bin = fake_tool_bin().lock().expect("fake tool bin lock");
    let candidate = fake_bin.as_ref()?.join(program);
    executable_file(&candidate).then_some(candidate)
}

pub struct FakeToolBinGuard {
    _lock: MutexGuard<'static, ()>,
    previous: Option<PathBuf>,
}

impl Drop for FakeToolBinGuard {
    fn drop(&mut self) {
        *fake_tool_bin().lock().expect("fake tool bin lock") = self.previous.take();
    }
}

pub fn use_fake_tool_bin(path: &Path) -> FakeToolBinGuard {
    let lock = fake_tool_bin_test_lock()
        .lock()
        .expect("fake tool bin test lock");
    let mut fake_bin = fake_tool_bin().lock().expect("fake tool bin lock");
    let previous = fake_bin.replace(path.to_path_buf());
    drop(fake_bin);
    FakeToolBinGuard {
        _lock: lock,
        previous,
    }
}

fn fake_tool_bin() -> &'static Mutex<Option<PathBuf>> {
    static FAKE_TOOL_BIN: OnceLock<Mutex<Option<PathBuf>>> = OnceLock::new();
    FAKE_TOOL_BIN.get_or_init(|| Mutex::new(None))
}

fn fake_tool_bin_test_lock() -> &'static Mutex<()> {
    // Async command tests may resume on another worker, so fixtures are process-global and serial
    static FAKE_TOOL_BIN_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    FAKE_TOOL_BIN_TEST_LOCK.get_or_init(|| Mutex::new(()))
}
