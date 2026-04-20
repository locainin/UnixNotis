//! Installer backup settings and config file helpers

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::paths::format_with_home;

use super::super::super::{log_line, ActionContext};
use super::write::write_atomic;

const INSTALLER_CONFIG_FILE: &str = "installer.toml";
const INSTALLER_CONFIG_TEMPLATE: &str = r#"# UnixNotis installer settings
# Backup retention for config/theme resets
[backups]
keep = 3
"#;

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(in crate::actions::config) struct InstallerConfig {
    pub(in crate::actions::config) backups: BackupConfig,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub(in crate::actions::config) struct BackupConfig {
    // Number of dated backup directories to keep in the config root
    pub(in crate::actions::config) keep: usize,
}

impl Default for BackupConfig {
    fn default() -> Self {
        Self { keep: 3 }
    }
}

pub(in crate::actions::config) fn ensure_installer_config(
    ctx: &mut ActionContext,
    config_dir: &Path,
) -> Result<PathBuf> {
    let config_path = config_dir.join(INSTALLER_CONFIG_FILE);
    if config_path.exists() {
        log_line(
            ctx,
            format!(
                "Installer config present: {}",
                format_with_home(&config_path)
            ),
        );
        return Ok(config_path);
    }

    write_atomic(&config_path, INSTALLER_CONFIG_TEMPLATE)
        .with_context(|| "failed to write installer.toml")?;
    log_line(
        ctx,
        format!(
            "Installer config created: {}",
            format_with_home(&config_path)
        ),
    );
    Ok(config_path)
}

pub(in crate::actions::config) fn load_installer_config(
    config_dir: &Path,
    ctx: &mut ActionContext,
) -> InstallerConfig {
    let config_path = config_dir.join(INSTALLER_CONFIG_FILE);
    let Ok(contents) = fs::read_to_string(&config_path) else {
        return InstallerConfig::default();
    };

    match toml::from_str(&contents) {
        Ok(config) => config,
        Err(err) => {
            log_line(
                ctx,
                format!(
                    "Warning: invalid installer config at {}: {}",
                    format_with_home(&config_path),
                    err
                ),
            );
            InstallerConfig::default()
        }
    }
}
