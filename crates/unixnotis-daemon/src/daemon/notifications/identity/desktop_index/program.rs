//! Desktop launch-program parsing and path resolution

use std::path::{Path, PathBuf};

use gio::prelude::AppInfoExt;

pub(super) fn resolve_program(program: &Path) -> Option<PathBuf> {
    // Canonical paths are presentation data while device and inode carry the proof
    if program.is_absolute() {
        return program.canonicalize().ok();
    }
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|directory| directory.join(program))
        .find_map(|candidate| candidate.canonicalize().ok())
}

pub(in crate::daemon::notifications::identity) fn desktop_executable(
    desktop: &gio::DesktopAppInfo,
) -> Option<PathBuf> {
    // GIO exposes a nullable executable for valid D-Bus-activated entries without Exec
    desktop.commandline()?;
    let executable = desktop.executable();
    (!executable.as_os_str().is_empty()).then_some(executable)
}
