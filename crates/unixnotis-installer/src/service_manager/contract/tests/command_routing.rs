use std::cell::RefCell;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

thread_local! {
    // Per-thread routing keeps parallel manager-flow tests isolated
    static FAKE_COMMAND_BIN: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

pub(super) fn command_program(program: &str) -> std::io::Result<OsString> {
    if let Some(fake_bin) = configured_fake_command_bin() {
        if program.is_empty() || program.contains(std::path::MAIN_SEPARATOR) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "invalid isolated test tool name",
            ));
        }
        let candidate = fake_bin.join(program);
        // Fake executable links resolve through the stable test dispatcher
        if candidate.is_file() {
            return Ok(candidate.into_os_string());
        }
        match std::fs::symlink_metadata(&candidate) {
            Ok(_) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    format!("{program} is not a regular test tool"),
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("{program} is unavailable in the isolated test tool directory"),
        ));
    }
    crate::system_tools::program_path(program).map(PathBuf::into_os_string)
}

fn configured_fake_command_bin() -> Option<PathBuf> {
    FAKE_COMMAND_BIN.with(|fake_bin| fake_bin.borrow().clone())
}

pub struct FakeCommandBinGuard {
    previous: Option<PathBuf>,
}

impl Drop for FakeCommandBinGuard {
    fn drop(&mut self) {
        FAKE_COMMAND_BIN.with(|fake_bin| {
            *fake_bin.borrow_mut() = self.previous.take();
        });
    }
}

pub fn use_fake_command_bin(path: &Path) -> FakeCommandBinGuard {
    FAKE_COMMAND_BIN.with(|fake_bin| {
        let previous = fake_bin.borrow_mut().replace(path.to_path_buf());
        FakeCommandBinGuard { previous }
    })
}
