//! Trusted external tool lookup for local diagnostic helpers

#[cfg(test)]
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

#[cfg(test)]
use std::sync::{Mutex, MutexGuard, OnceLock};

pub fn command(program: &str) -> std::io::Result<Command> {
    let path = trusted_program_path(program).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("{program} not found in trusted system tool directories"),
        )
    })?;
    Ok(Command::new(path))
}

pub fn trusted_program_path(program: &str) -> Option<PathBuf> {
    if program.contains(std::path::MAIN_SEPARATOR) || program.is_empty() {
        return None;
    }

    #[cfg(test)]
    if fake_tool_bin_is_set() {
        return fake_program_path(program);
    }

    unixnotis_core::util::trusted_system_program_path(program)
}

#[cfg(test)]
fn executable_file(path: &Path) -> bool {
    let Ok(metadata) = path.metadata() else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    executable_mode(&metadata)
}

#[cfg(all(test, unix))]
fn executable_mode(metadata: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;

    metadata.permissions().mode() & 0o111 != 0
}

#[cfg(all(test, not(unix)))]
fn executable_mode(_metadata: &std::fs::Metadata) -> bool {
    true
}

#[cfg(test)]
fn fake_tool_bin_is_set() -> bool {
    fake_tool_bin()
        .lock()
        .expect("fake tool bin lock")
        .is_some()
}

#[cfg(test)]
fn fake_program_path(program: &str) -> Option<PathBuf> {
    let configured_bin = fake_tool_bin()
        .lock()
        .expect("fake tool bin lock")
        .clone()?;
    // Release the test lock before touching the filesystem so unrelated probes can proceed
    let candidate = configured_bin.join(program);
    executable_file(&candidate).then_some(candidate)
}

#[cfg(test)]
pub struct FakeToolBinGuard {
    _lock: MutexGuard<'static, ()>,
    previous: Option<PathBuf>,
}

#[cfg(test)]
impl Drop for FakeToolBinGuard {
    fn drop(&mut self) {
        *fake_tool_bin().lock().expect("fake tool bin lock") = self.previous.take();
    }
}

#[cfg(test)]
pub fn use_fake_tool_bin(path: &Path) -> FakeToolBinGuard {
    let lock = fake_tool_bin_test_lock()
        .lock()
        .expect("fake tool bin test lock");
    let mut fake_bin = fake_tool_bin().lock().expect("fake tool bin lock");
    let previous = fake_bin.replace(path.to_path_buf());
    FakeToolBinGuard {
        _lock: lock,
        previous,
    }
}

#[cfg(test)]
fn fake_tool_bin() -> &'static Mutex<Option<PathBuf>> {
    static FAKE_TOOL_BIN: OnceLock<Mutex<Option<PathBuf>>> = OnceLock::new();
    FAKE_TOOL_BIN.get_or_init(|| Mutex::new(None))
}

#[cfg(test)]
fn fake_tool_bin_test_lock() -> &'static Mutex<()> {
    static FAKE_TOOL_BIN_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    FAKE_TOOL_BIN_TEST_LOCK.get_or_init(|| Mutex::new(()))
}

#[cfg(test)]
#[path = "tests/system_tools.rs"]
mod tests;
