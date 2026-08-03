//! Config and theme file creation or reset logic

use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use unixnotis_core::{
    filesystem::open_regular_file,
    filesystem::{create_directory_all, write_file_atomic, write_file_if_missing, ContainedPath},
    render_default_config_toml, reset_config_to_defaults, Config, ResetConfigOptions,
    DEFAULT_BASE_CSS, DEFAULT_MEDIA_CSS, DEFAULT_PANEL_CSS, DEFAULT_POPUP_CSS, DEFAULT_WIDGETS_CSS,
};

use crate::paths::format_with_home;

use super::super::{log_line, ActionContext};
use super::backup::{ensure_installer_config, load_installer_config};

pub fn ensure_config(ctx: &mut ActionContext) -> Result<()> {
    let config_dir = Config::default_config_dir().map_err(|err| anyhow!(err.to_string()))?;
    let config_path = Config::default_config_path().map_err(|err| anyhow!(err.to_string()))?;
    log_line(
        ctx,
        format!("Config directory: {}", format_with_home(&config_dir)),
    );

    let config = if config_path.exists() {
        log_line(
            ctx,
            format!("Config file present: {}", format_with_home(&config_path)),
        );

        // Existing theme paths are part of the configuration contract
        Config::load_from_path(&config_path)
            .map_err(|error| anyhow!(error.to_string()))
            .context("load existing configuration before provisioning theme files")?
    } else {
        let config = Config::default();
        // Write a default config so there is always a working base to edit
        let config_toml = render_default_config_toml(&config)?;
        write_file_atomic(&config_path, config_toml.as_bytes(), 0o644)
            .with_context(|| "failed to write config.toml")?;
        log_line(
            ctx,
            format!("Config file created: {}", format_with_home(&config_path)),
        );

        config
    };

    ensure_installer_config(ctx, &config_dir)?;
    ensure_default_scripts(ctx, &config_dir)?;
    for provision in ensure_default_theme_files(&config, &config_dir)? {
        let path = format_with_home(&provision.path);
        let message = match provision.status {
            ThemeFileStatus::Created => format!("Default theme CSS created: {path}"),
            ThemeFileStatus::Present => format!("Default theme CSS present: {path}"),
            ThemeFileStatus::ExternalManaged => {
                format!("External theme CSS preserved: {path}")
            }
            ThemeFileStatus::ExternalMissing => {
                format!("External theme CSS missing; runtime fallback remains active: {path}")
            }
            ThemeFileStatus::ExternalUnsafe => {
                format!("External theme CSS is unsafe; runtime fallback remains active: {path}")
            }
        };
        log_line(ctx, message);
    }

    log_line(ctx, "Theme CSS provisioning complete".to_string());

    Ok(())
}

pub fn reset_config(ctx: &mut ActionContext) -> Result<()> {
    let config_dir = Config::default_config_dir().map_err(|err| anyhow!(err.to_string()))?;
    ensure_installer_config(ctx, &config_dir)?;
    let installer_config = load_installer_config(&config_dir).context("load installer settings")?;
    let report = reset_config_to_defaults(&ResetConfigOptions {
        config_dir: config_dir.clone(),
        backup_retention: installer_config.backups.keep,
    })
    .context("reset configuration to defaults")?;
    if let Some(backup_dir) = report.backup_dir {
        log_line(
            ctx,
            format!(
                "Backed up existing configuration to {}",
                format_with_home(&backup_dir)
            ),
        );
    }
    log_line(
        ctx,
        "Reset config file and bundled scripts to defaults".to_string(),
    );
    log_line(ctx, "Reset theme CSS files to current defaults".to_string());
    Ok(())
}

fn ensure_default_scripts(ctx: &mut ActionContext, config_dir: &Path) -> Result<()> {
    // Snapshot presence first so logs can say created without duplicating write logic
    let pre_existing = unixnotis_core::DEFAULT_SCRIPTS
        .iter()
        .map(|script| config_dir.join(script.relative_path).exists())
        .collect::<Vec<_>>();

    // Core owns script provisioning because center startup needs the same guarantee
    Config::ensure_default_scripts_in(config_dir).map_err(|err| anyhow!(err.to_string()))?;

    for (script, existed) in unixnotis_core::DEFAULT_SCRIPTS
        .iter()
        .zip(pre_existing.iter())
    {
        let path = config_dir.join(script.relative_path);
        let status = if *existed { "present" } else { "created" };
        log_line(
            ctx,
            format!("Default script {status}: {}", format_with_home(&path)),
        );
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) enum ThemeFileStatus {
    Created,
    Present,
    ExternalManaged,
    ExternalMissing,
    ExternalUnsafe,
}

#[derive(Debug, Eq, PartialEq)]
struct ThemeFileProvision {
    path: PathBuf,
    status: ThemeFileStatus,
}

fn ensure_default_theme_files(
    config: &Config,
    config_dir: &Path,
) -> Result<Vec<ThemeFileProvision>> {
    // Use the generated configuration paths so provisioning matches runtime loading
    let paths = config
        .resolve_theme_paths_from(config_dir)
        .map_err(|error| anyhow!(error.to_string()))?;
    let files = [
        (paths.base_css, DEFAULT_BASE_CSS),
        (paths.panel_css, DEFAULT_PANEL_CSS),
        (paths.popup_css, DEFAULT_POPUP_CSS),
        (paths.widgets_css, DEFAULT_WIDGETS_CSS),
        (paths.media_css, DEFAULT_MEDIA_CSS),
    ];

    files
        .into_iter()
        .map(|(path, contents)| {
            // Provisioning may only create files beneath the active config directory
            let path = match ContainedPath::resolve(config_dir, &path) {
                Ok(contained) => contained.absolute(),
                Err(_) => {
                    return Ok(ThemeFileProvision {
                        status: classify_external_theme_file(&path),
                        path,
                    });
                }
            };
            // Nested configured paths need secure parents before exclusive creation
            if let Some(parent) = path.parent() {
                create_directory_all(parent, 0o700)
                    .with_context(|| format!("create theme directory {}", parent.display()))?;
            }
            // Exclusive creation preserves custom files and rejects unsafe targets
            let created = write_file_if_missing(&path, contents.as_bytes(), 0o644)
                .with_context(|| format!("provision {}", path.display()))?;
            Ok(ThemeFileProvision {
                path,
                status: if created {
                    ThemeFileStatus::Created
                } else {
                    ThemeFileStatus::Present
                },
            })
        })
        .collect()
}

pub(super) fn classify_external_theme_file(path: &Path) -> ThemeFileStatus {
    match open_regular_file(path) {
        Ok(file) => {
            drop(file);
            ThemeFileStatus::ExternalManaged
        }
        Err(error) if error.kind() == ErrorKind::NotFound => ThemeFileStatus::ExternalMissing,
        Err(_) => ThemeFileStatus::ExternalUnsafe,
    }
}
