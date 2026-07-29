//! Safe export of editable embedded stock theme files

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use unixnotis_core::filesystem::{
    create_directory_all, write_file_if_missing, CreateDirectoryOutcome,
};
use unixnotis_core::{
    Config, ThemeManifest, DEFAULT_BASE_CSS, DEFAULT_MEDIA_CSS, DEFAULT_PANEL_CSS,
    DEFAULT_POPUP_CSS, DEFAULT_WIDGETS_CSS, THEME_API_VERSION,
};

const DEFAULT_EXPORT_DIRECTORY: &str = "stock-theme-v2";

pub(super) fn run(output: Option<PathBuf>) -> Result<()> {
    let destination = match output {
        Some(output) => output,
        None => default_export_directory()?,
    };
    export_stock_theme(&destination)?;
    crate::output::write_stdout(&format!(
        "Exported editable stock theme to {}\nThe active theme mode was not changed.\n",
        destination.display()
    ))
}

fn default_export_directory() -> Result<PathBuf> {
    let config_path = Config::active_config_path().context("resolve active config path")?;
    default_export_directory_for_config(&config_path)
}

pub(super) fn default_export_directory_for_config(config_path: &Path) -> Result<PathBuf> {
    let parent = config_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or_else(|| anyhow!("active config path has no parent directory"))?;
    Ok(parent.join(DEFAULT_EXPORT_DIRECTORY))
}

pub(super) fn export_stock_theme(destination: &Path) -> Result<()> {
    match create_directory_all(destination, 0o700).with_context(|| {
        format!(
            "create stock theme export directory {}",
            destination.display()
        )
    })? {
        CreateDirectoryOutcome::TargetCreated => {}
        CreateDirectoryOutcome::TargetAlreadyExisted => {
            return Err(anyhow!(
                "stock theme export directory already exists: {}",
                destination.display()
            ));
        }
    }

    let manifest = toml::to_string_pretty(&ThemeManifest {
        api_version: THEME_API_VERSION,
        name: "UnixNotis stock export".to_string(),
    })
    .context("serialize stock theme manifest")?;
    for (name, contents) in [
        ("base.css", DEFAULT_BASE_CSS),
        ("panel.css", DEFAULT_PANEL_CSS),
        ("popup.css", DEFAULT_POPUP_CSS),
        ("widgets.css", DEFAULT_WIDGETS_CSS),
        ("media.css", DEFAULT_MEDIA_CSS),
        ("theme.toml", manifest.as_str()),
    ] {
        let path = destination.join(name);
        let created = write_file_if_missing(&path, contents.as_bytes(), 0o600)
            .with_context(|| format!("write exported stock theme file {name}"))?;
        if !created {
            return Err(anyhow!(
                "stock theme export was interrupted by an existing file: {}",
                path.display()
            ));
        }
    }
    Ok(())
}
