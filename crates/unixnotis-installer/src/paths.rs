//! Filesystem layout helpers for UnixNotis installation paths.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};

pub struct InstallPaths {
    pub repo_root: PathBuf,
    pub release_dir: PathBuf,
    pub bin_dir: PathBuf,
    pub unit_dir: PathBuf,
    pub unit_path: PathBuf,
}

impl InstallPaths {
    pub fn discover() -> Result<Self> {
        let repo_root = find_repo_root()?;
        let release_dir = repo_root.join("target").join("release");
        let bin_dir = home_dir()?.join(".local").join("bin");
        let unit_dir = home_dir()?.join(".config").join("systemd").join("user");
        let unit_path = unit_dir.join("unixnotis-daemon.service");

        Ok(Self {
            repo_root,
            release_dir,
            bin_dir,
            unit_dir,
            unit_path,
        })
    }
}

pub fn home_dir() -> Result<PathBuf> {
    let home = env::var("HOME").map_err(|_| anyhow!("HOME is not set"))?;
    Ok(PathBuf::from(home))
}

pub fn format_with_home(path: &Path) -> String {
    if let Ok(home) = home_dir() {
        if let Ok(stripped) = path.strip_prefix(&home) {
            let mut rendered = PathBuf::from("$HOME");
            rendered.push(stripped);
            return rendered.display().to_string();
        }
    }
    path.display().to_string()
}

fn find_repo_root() -> Result<PathBuf> {
    if let Ok(root) = env::var("UNIXNOTIS_REPO_ROOT") {
        let root_path = PathBuf::from(root);
        let cargo = root_path.join("Cargo.toml");
        if cargo.is_file() && is_unixnotis_repo(&cargo) {
            return Ok(root_path);
        }
    }

    let mut dir = env::current_dir()?;
    loop {
        let cargo = dir.join("Cargo.toml");
        if cargo.is_file() && is_unixnotis_repo(&cargo) {
            return Ok(dir);
        }
        if !dir.pop() {
            break;
        }
    }

    Err(anyhow!(
        "repository root not found (set UNIXNOTIS_REPO_ROOT or run from UnixNotis repo)"
    ))
}

fn is_unixnotis_repo(cargo_toml: &Path) -> bool {
    let Ok(contents) = fs::read_to_string(cargo_toml) else {
        return false;
    };
    let markers = [
        "crates/unixnotis-daemon",
        "crates/unixnotis-core",
        "name = \"unixnotis-daemon\"",
    ];
    markers.iter().any(|marker| contents.contains(marker))
}
