//! User path and backend root discovery helpers

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};

pub fn home_dir() -> Result<PathBuf> {
    let home = env::var("HOME").map_err(|_| anyhow!("HOME is not set"))?;
    Ok(PathBuf::from(home))
}

pub(super) fn systemd_user_dir() -> Result<PathBuf> {
    if let Some(base) = xdg_config_home() {
        Ok(base.join("systemd").join("user"))
    } else {
        Ok(home_dir()?.join(".config").join("systemd").join("user"))
    }
}

pub(super) fn dinit_user_dir() -> Result<PathBuf> {
    if let Some(base) = xdg_config_home() {
        Ok(base.join("dinit.d"))
    } else {
        Ok(home_dir()?.join(".config").join("dinit.d"))
    }
}

pub(super) fn runit_user_dir() -> Result<PathBuf> {
    // Selection still follows documented override priority
    if let Some(path) = absolute_env_path("UNIXNOTIS_RUNIT_SERVICE_DIR")? {
        return Ok(path);
    }
    if let Some(path) = absolute_env_path("SVDIR")? {
        return Ok(path);
    }
    runit_default_user_dir()
}

pub(super) fn s6_user_dir() -> Result<PathBuf> {
    // Selection prefers an explicit data root, then Artix's documented local user root
    if let Some(path) = absolute_env_path("UNIXNOTIS_S6_DATA_DIR")? {
        // Custom roots are safe now that UnixNotis compiles the s6-rc database directly
        return Ok(path);
    }
    // Artix documents local user s6 data under ~/.local/share/s6
    s6_default_user_dir()
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
    if let Some(path) = absolute_env_path("UNIXNOTIS_S6RC_LIVE_DIR")? {
        // Explicit live roots are for testers and advanced users who already know their tree
        return Ok(path);
    }
    let user = env::var("USER").map_err(|_| anyhow!("USER is not set"))?;
    let integrated = PathBuf::from("/run").join(&user).join("s6-rc");
    if path_is_directory_or_symlink_to_directory(&integrated) {
        // Artix integrated local supervision wires the user s6-rc tree under /run/$USER
        // s6-rc-init normally exposes this live path as a symlink to a real live directory
        return Ok(integrated);
    }
    let standalone = PathBuf::from("/tmp").join(&user).join("s6-rc");
    if path_is_plain_directory(&standalone) {
        // Artix standalone local supervision uses /tmp/$USER/s6-rc in its documented setup
        return Ok(standalone);
    }
    let local = data_root.join("rc").join("live");
    if path_is_directory_or_symlink_to_directory(&local) {
        // Test and custom layouts can keep a live tree beside the compiled database root
        // Keep the symlink name because s6-rc-update expects the original live argument
        return Ok(local);
    }
    // Return the integrated path so readiness can show the normal setup hint
    Ok(integrated)
}

fn xdg_config_home() -> Option<PathBuf> {
    let raw = env::var("XDG_CONFIG_HOME").ok()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let path = PathBuf::from(trimmed);
    if path.is_absolute() {
        Some(path)
    } else {
        None
    }
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

fn path_is_directory_or_symlink_to_directory(path: &Path) -> bool {
    fs::metadata(path)
        // s6 live roots are expected to be symlinks that point at the current live tree
        .map(|metadata| metadata.is_dir())
        .unwrap_or(false)
}

fn path_is_plain_directory(path: &Path) -> bool {
    fs::symlink_metadata(path)
        // Auto-detected /tmp roots must not follow symlinks into surprising locations
        .map(|metadata| metadata.file_type().is_dir())
        .unwrap_or(false)
}
