//! Installer backup settings and config file helpers

use std::path::{Path, PathBuf};

use crate::paths::format_with_home;
use anyhow::Result;

use super::super::super::{log_line, ActionContext};

pub(in crate::actions::config) use unixnotis_core::InstallerConfig;

pub(in crate::actions::config) fn ensure_installer_config(
    ctx: &mut ActionContext,
    config_dir: &Path,
) -> Result<PathBuf> {
    let (config_path, created) = unixnotis_core::ensure_installer_config(config_dir)?;
    if !created {
        log_line(
            ctx,
            format!(
                "Installer config present: {}",
                format_with_home(&config_path)
            ),
        );
        return Ok(config_path);
    }

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
) -> Result<InstallerConfig> {
    unixnotis_core::load_installer_config(config_dir)
}
