//! Production validation for external diagnostic tools

use std::path::PathBuf;

pub(super) fn trusted_program_path(program: &str) -> Option<PathBuf> {
    // Core validates plain names and owns the fixed directory policy used by every binary
    unixnotis_core::util::trusted_system_program_path(program)
}
