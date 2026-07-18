use std::cell::RefCell;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

thread_local! {
    // Per-thread routing keeps parallel manager-flow tests isolated
    static FAKE_COMMAND_BIN: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

pub(super) fn command_program(program: &str) -> std::io::Result<OsString> {
    if let Some(fake_program) = fake_command_program(program) {
        return Ok(fake_program.into_os_string());
    }
    crate::system_tools::program_path(program).map(PathBuf::into_os_string)
}

fn fake_command_program(program: &str) -> Option<PathBuf> {
    if program.is_empty() || program.contains(std::path::MAIN_SEPARATOR) {
        return None;
    }
    FAKE_COMMAND_BIN.with(|fake_bin| {
        let candidate = fake_bin.borrow().as_ref()?.join(program);
        candidate.is_file().then_some(candidate)
    })
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
