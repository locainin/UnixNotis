//! Production route into installer tool lookup

use std::path::PathBuf;

pub(super) fn trusted_program_path(program: &str) -> Option<PathBuf> {
    // The separate route keeps test substitution out of production lookup code
    super::lookup::trusted_program_path(program)
}
