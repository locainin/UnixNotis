//! Production route into daemon tool lookup

use std::path::PathBuf;

pub(super) fn trusted_program_path(program: &str) -> Option<PathBuf> {
    // Tests replace only this routing layer while production keeps the shared policy
    super::lookup::trusted_program_path(program)
}
