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
        let unit_dir = systemd_user_dir()?;
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

fn systemd_user_dir() -> Result<PathBuf> {
    if let Some(base) = xdg_config_home() {
        Ok(base.join("systemd").join("user"))
    } else {
        Ok(home_dir()?.join(".config").join("systemd").join("user"))
    }
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

#[cfg(test)]
mod tests {
    use super::{format_with_home, is_unixnotis_repo, InstallPaths};
    use std::env;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .expect("env lock")
    }

    fn set_env(key: &str, value: Option<&str>) -> Option<String> {
        let previous = env::var(key).ok();
        match value {
            Some(value) => env::set_var(key, value),
            None => env::remove_var(key),
        }
        previous
    }

    fn restore_env(key: &str, previous: Option<String>) {
        match previous {
            Some(value) => env::set_var(key, value),
            None => env::remove_var(key),
        }
    }

    #[test]
    fn format_with_home_rewrites_prefix() {
        // Confirms home-prefixed paths are rendered with the $HOME shorthand.
        let Ok(home) = env::var("HOME") else {
            return;
        };
        if home.is_empty() {
            return;
        }
        let path = PathBuf::from(&home).join(".config").join("unixnotis");
        let rendered = format_with_home(&path);
        assert!(rendered.starts_with("$HOME"));
    }

    #[test]
    fn is_unixnotis_repo_detects_markers() {
        // Validates that known workspace markers are detected in a Cargo.toml file.
        let Ok(home) = env::var("HOME") else {
            return;
        };
        if home.is_empty() {
            return;
        }
        let dir = PathBuf::from(home)
            .join(".cache")
            .join(format!("unixnotis-test-{}", std::process::id()));
        if fs::create_dir_all(&dir).is_err() {
            return;
        }
        let cargo_path = dir.join("Cargo.toml");
        let contents = r#"
[package]
name = "unixnotis-daemon"

[workspace]
members = ["crates/unixnotis-daemon", "crates/unixnotis-core"]
"#;
        if fs::write(&cargo_path, contents).is_err() {
            let _ = fs::remove_dir_all(&dir);
            return;
        }

        assert!(is_unixnotis_repo(&cargo_path));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn install_paths_use_xdg_config_home_for_systemd_units() {
        let _guard = env_lock();
        let Ok(home) = env::var("HOME") else {
            return;
        };
        if home.is_empty() {
            return;
        }
        let xdg_root = PathBuf::from(&home).join(".config-xdg-test");
        let previous = set_env("XDG_CONFIG_HOME", Some(xdg_root.to_string_lossy().as_ref()));

        let paths = InstallPaths::discover().expect("paths should resolve in repo tests");
        assert_eq!(paths.unit_dir, xdg_root.join("systemd").join("user"));

        restore_env("XDG_CONFIG_HOME", previous);
    }
}
