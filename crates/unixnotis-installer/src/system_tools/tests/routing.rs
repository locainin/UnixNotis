use std::cell::RefCell;
use std::path::{Path, PathBuf};

thread_local! {
    // Per-thread routing keeps parallel installer tests independent
    static FAKE_TOOL_BIN: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

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

fn fake_program_path(program: &str) -> Option<PathBuf> {
    FAKE_TOOL_BIN.with(|fake_bin| {
        let candidate = fake_bin.borrow().as_ref()?.join(program);
        executable_file(&candidate).then_some(candidate)
    })
}

fn fake_tool_bin_is_set() -> bool {
    FAKE_TOOL_BIN.with(|fake_bin| fake_bin.borrow().is_some())
}

pub struct FakeToolBinGuard {
    previous: Option<PathBuf>,
}

impl Drop for FakeToolBinGuard {
    fn drop(&mut self) {
        FAKE_TOOL_BIN.with(|fake_bin| {
            *fake_bin.borrow_mut() = self.previous.take();
        });
    }
}

pub fn use_fake_tool_bin(path: &Path) -> FakeToolBinGuard {
    FAKE_TOOL_BIN.with(|fake_bin| {
        let previous = fake_bin.borrow_mut().replace(path.to_path_buf());
        FakeToolBinGuard { previous }
    })
}
