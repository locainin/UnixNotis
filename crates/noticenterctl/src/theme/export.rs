//! Safe export of editable embedded stock theme files

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Context, Result};
use unixnotis_core::filesystem::{
    create_directory_all, remove_directory_tree, rename_directory_no_replace,
    write_file_if_missing, CreateDirectoryOutcome, RenameDirectoryOutcome,
};
use unixnotis_core::{
    Config, ThemeManifest, DEFAULT_BASE_CSS, DEFAULT_MEDIA_CSS, DEFAULT_PANEL_CSS,
    DEFAULT_POPUP_CSS, DEFAULT_WIDGETS_CSS, THEME_API_VERSION,
};

const DEFAULT_EXPORT_DIRECTORY: &str = "stock-theme-v2";
const MAX_STAGING_ATTEMPTS: u8 = 16;
static STAGING_COUNTER: AtomicU64 = AtomicU64::new(0);

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
    let manifest = toml::to_string_pretty(&ThemeManifest {
        api_version: THEME_API_VERSION,
        name: "UnixNotis stock export".to_string(),
    })
    .context("serialize stock theme manifest")?;
    export_stock_theme_files(
        destination,
        &[
            ("base.css", DEFAULT_BASE_CSS),
            ("panel.css", DEFAULT_PANEL_CSS),
            ("popup.css", DEFAULT_POPUP_CSS),
            ("widgets.css", DEFAULT_WIDGETS_CSS),
            ("media.css", DEFAULT_MEDIA_CSS),
            ("theme.toml", manifest.as_str()),
        ],
    )
}

pub(super) fn export_stock_theme_files(destination: &Path, files: &[(&str, &str)]) -> Result<()> {
    match std::fs::symlink_metadata(destination) {
        Ok(_) => {
            return Err(anyhow!(
                "stock theme export directory already exists: {}",
                destination.display()
            ));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(error).with_context(|| {
                format!(
                    "inspect stock theme export destination {}",
                    destination.display()
                )
            });
        }
    }

    let staging = reserve_staging_directory(destination)?;
    let write_result = write_staged_theme_files(&staging, files);
    if let Err(error) = write_result {
        return Err(clean_up_failed_staging(&staging, error));
    }

    match rename_directory_no_replace(&staging, destination).with_context(|| {
        format!(
            "publish complete stock theme export {}",
            destination.display()
        )
    }) {
        Ok(RenameDirectoryOutcome::Renamed) => Ok(()),
        Ok(RenameDirectoryOutcome::DestinationExists) => {
            let error = anyhow!(
                "stock theme export directory already exists: {}",
                destination.display()
            );
            Err(clean_up_failed_staging(&staging, error))
        }
        Ok(RenameDirectoryOutcome::SourceMissing) => Err(anyhow!(
            "stock theme staging directory disappeared before publication: {}",
            staging.display()
        )),
        Err(error) => Err(clean_up_failed_staging(&staging, error)),
    }
}

fn write_staged_theme_files(staging: &Path, files: &[(&str, &str)]) -> Result<()> {
    for (name, contents) in files {
        let path = staging.join(name);
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

fn reserve_staging_directory(destination: &Path) -> Result<PathBuf> {
    let parent = export_parent(destination);
    create_directory_all(parent, 0o700)
        .with_context(|| format!("create stock theme export parent {}", parent.display()))?;
    let file_name = destination
        .file_name()
        .filter(|name| !name.is_empty())
        .ok_or_else(|| anyhow!("stock theme export path needs a directory name"))?;
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is earlier than the Unix epoch")?
        .as_nanos();

    for attempt in 0..MAX_STAGING_ATTEMPTS {
        let serial = STAGING_COUNTER.fetch_add(1, Ordering::Relaxed);
        let mut staging_name = OsString::from(".");
        staging_name.push(file_name);
        staging_name.push(format!(
            ".{}.{}.{serial}.{attempt}.staging",
            std::process::id(),
            nanos
        ));
        let staging = parent.join(staging_name);
        match create_directory_all(&staging, 0o700).with_context(|| {
            format!(
                "create private stock theme staging area {}",
                staging.display()
            )
        })? {
            CreateDirectoryOutcome::TargetCreated => return Ok(staging),
            CreateDirectoryOutcome::TargetAlreadyExisted => {}
        }
    }

    Err(anyhow!(
        "unable to reserve private staging beside {}",
        destination.display()
    ))
}

pub(super) fn export_parent(destination: &Path) -> &Path {
    destination
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}

fn clean_up_failed_staging(staging: &Path, error: anyhow::Error) -> anyhow::Error {
    match remove_directory_tree(staging) {
        Ok(_) => error,
        Err(cleanup_error) => error.context(format!(
            "also failed to remove stock theme staging area {}: {cleanup_error}",
            staging.display()
        )),
    }
}
