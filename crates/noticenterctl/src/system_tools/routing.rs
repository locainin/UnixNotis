//! Production route into the trusted lookup policy

use std::path::PathBuf;

pub(super) fn trusted_program_path(program: &str) -> Option<PathBuf> {
    // Keeping this hop small lets tests replace routing without replacing validation code
    super::lookup::trusted_program_path(program)
}
