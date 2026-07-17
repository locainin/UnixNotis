//! Production validation for external diagnostic tools

use std::path::PathBuf;

pub(super) fn trusted_program_path(program: &str) -> Option<PathBuf> {
    // A plain program name prevents callers from smuggling an alternate directory
    if program.is_empty() || program.contains(std::path::MAIN_SEPARATOR) {
        return None;
    }
    // Core owns the fixed directory policy shared by every UnixNotis executable
    unixnotis_core::util::trusted_system_program_path(program)
}
