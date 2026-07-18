//! User path and backend root discovery helpers

use std::env;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};

pub fn home_dir() -> Result<PathBuf> {
    let home = env::var("HOME").context("HOME is not set")?;
    Ok(PathBuf::from(home))
}

pub(super) fn systemd_user_dir() -> Result<PathBuf> {
    unixnotis_core::service_manager::systemd_user_dir().map_err(Into::into)
}

pub(super) fn dinit_user_dir() -> Result<PathBuf> {
    unixnotis_core::service_manager::dinit_user_dir().map_err(Into::into)
}

pub(super) fn runit_user_dir() -> Result<PathBuf> {
    // Diagnostics and installation must inspect the same selected service root
    unixnotis_core::service_manager::runit_user_dir().map_err(Into::into)
}

pub(super) fn s6_user_dir() -> Result<PathBuf> {
    // Diagnostics and installation must inspect the same selected data root
    unixnotis_core::service_manager::s6_user_dir().map_err(Into::into)
}

pub(super) fn runit_user_dir_candidates() -> Vec<Result<PathBuf>> {
    let mut candidates = Vec::new();
    // Explicit UnixNotis override wins selection, but conflict scans still need lower roots
    push_optional_env_path(&mut candidates, "UNIXNOTIS_RUNIT_SERVICE_DIR");
    // SVDIR is common for runit tooling and can point at a watched user service tree
    push_optional_env_path(&mut candidates, "SVDIR");
    // The Void/Turnstile-style default remains important when testing with overrides
    candidates.push(runit_default_user_dir());
    // Conflict scans must inspect every possible root once, including same-backend fallbacks
    dedupe_path_results(candidates)
}

pub(super) fn s6_user_dir_candidates() -> Vec<Result<PathBuf>> {
    let mut candidates = Vec::new();
    // Custom source roots are allowed, but old local-user installs may remain in the default root
    push_optional_env_path(&mut candidates, "UNIXNOTIS_S6_DATA_DIR");
    candidates.push(s6_default_user_dir());
    // A selected custom source root should not hide an old install in the normal Artix root
    dedupe_path_results(candidates)
}

pub(super) fn s6_live_dir(data_root: &Path) -> Result<PathBuf> {
    unixnotis_core::service_manager::s6_live_dir(data_root).map_err(Into::into)
}

fn absolute_env_path(name: &str) -> Result<Option<PathBuf>> {
    let Ok(raw) = env::var(name) else {
        return Ok(None);
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let path = PathBuf::from(trimmed);
    if !path.is_absolute() {
        return Err(anyhow!("{name} must be an absolute path"));
    }
    Ok(Some(path))
}

fn runit_default_user_dir() -> Result<PathBuf> {
    Ok(home_dir()?.join(".config").join("service"))
}

fn s6_default_user_dir() -> Result<PathBuf> {
    Ok(home_dir()?.join(".local").join("share").join("s6"))
}

fn push_optional_env_path(candidates: &mut Vec<Result<PathBuf>>, name: &str) {
    match absolute_env_path(name) {
        // Valid overrides become scan candidates for same-backend conflict detection
        Ok(Some(path)) => candidates.push(Ok(path)),
        Ok(None) => {}
        // Invalid overrides are preserved as warnings instead of silently ignored
        Err(err) => candidates.push(Err(err)),
    }
}

fn dedupe_path_results(candidates: Vec<Result<PathBuf>>) -> Vec<Result<PathBuf>> {
    let mut deduped = Vec::new();
    let mut seen = Vec::new();
    for candidate in candidates {
        match candidate {
            Ok(path) => {
                // Keep first occurrence so priority order remains visible in tests and logs
                if !seen.iter().any(|existing| existing == &path) {
                    seen.push(path.clone());
                    deduped.push(Ok(path));
                }
            }
            Err(err) => deduped.push(Err(err)),
        }
    }
    deduped
}
