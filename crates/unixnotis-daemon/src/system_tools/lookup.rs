//! Production validation for daemon-owned external tools

use std::path::PathBuf;

pub(super) fn trusted_program_path(program: &str) -> Option<PathBuf> {
    // Only bare names may enter the fixed trusted-directory search
    if program.is_empty() || program.contains(std::path::MAIN_SEPARATOR) {
        return None;
    }
    // One shared policy keeps daemon and control helper behavior aligned
    unixnotis_core::util::trusted_system_program_path(program)
}
