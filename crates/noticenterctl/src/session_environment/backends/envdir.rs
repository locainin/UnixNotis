//! Hardened envdir publication below an installed service directory

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use unixnotis_core::filesystem::write_file_atomic;
use unixnotis_core::service_manager::envdir_file_contents;

use super::super::variables::IMPORT_VARS;

pub(in crate::session_environment) fn write_envdir(service: &Path, env_dir: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(service)
        .with_context(|| format!("inspect installed service directory {}", service.display()))?;
    // The service anchor must be a real directory before any child path is created
    if !metadata.file_type().is_dir() {
        bail!(
            "refusing to write environment outside a regular service directory: {}",
            service.display()
        );
    }
    // PATH remains fixed by the installed run script instead of session input
    for name in IMPORT_VARS.into_iter().filter(|name| *name != "PATH") {
        let value = env::var(name).ok();
        let contents = envdir_file_contents(value.as_deref());
        let target: PathBuf = env_dir.join(name);
        // Atomic descriptor-relative writes reject symlink traversal and partial files
        write_file_atomic(&target, contents.as_bytes(), 0o600)
            .with_context(|| format!("write service environment file {}", target.display()))?;
    }
    Ok(())
}
