//! Production validation for installer-owned external tools

use std::path::PathBuf;

pub(super) fn trusted_program_path(program: &str) -> Option<PathBuf> {
    // Path separators would bypass the fixed trusted-directory policy
    if program.is_empty() || program.contains(std::path::MAIN_SEPARATOR) {
        return None;
    }
    // Core checks file type, executable mode, and the supported system roots
    unixnotis_core::util::trusted_system_program_path(program)
}
