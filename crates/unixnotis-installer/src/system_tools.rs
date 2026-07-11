//! Trusted external tool lookup for installer probes

#[cfg(test)]
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

#[cfg(test)]
use std::cell::RefCell;

#[cfg(test)]
thread_local! {
    // Tests can route trusted-tool probes without changing process-global PATH
    static FAKE_TOOL_BIN: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

pub(crate) fn command(program: &str) -> std::io::Result<Command> {
    let path = program_path(program)?;
    Ok(Command::new(path))
}

pub(crate) fn program_path(program: &str) -> std::io::Result<PathBuf> {
    trusted_program_path(program).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("{program} not found in trusted system tool directories"),
        )
    })
}

pub(crate) fn program_exists(program: &str) -> bool {
    trusted_program_path(program).is_some()
}

fn trusted_program_path(program: &str) -> Option<PathBuf> {
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
fn fake_program_path(program: &str) -> Option<PathBuf> {
    FAKE_TOOL_BIN.with(|fake_bin| {
        let fake_bin = fake_bin.borrow();
        let candidate = fake_bin.as_ref()?.join(program);
        executable_file(&candidate).then_some(candidate)
    })
}

#[cfg(test)]
fn fake_tool_bin_is_set() -> bool {
    FAKE_TOOL_BIN.with(|fake_bin| fake_bin.borrow().is_some())
}

#[cfg(test)]
pub(crate) struct FakeToolBinGuard {
    previous: Option<PathBuf>,
}

#[cfg(test)]
impl Drop for FakeToolBinGuard {
    fn drop(&mut self) {
        FAKE_TOOL_BIN.with(|fake_bin| {
            *fake_bin.borrow_mut() = self.previous.take();
        });
    }
}

#[cfg(test)]
pub(crate) fn use_fake_tool_bin(path: &Path) -> FakeToolBinGuard {
    FAKE_TOOL_BIN.with(|fake_bin| {
        let mut fake_bin = fake_bin.borrow_mut();
        let previous = fake_bin.replace(path.to_path_buf());
        FakeToolBinGuard { previous }
    })
}

#[cfg(test)]
#[path = "tests/system_tools.rs"]
mod tests;
