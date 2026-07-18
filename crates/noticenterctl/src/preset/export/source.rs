//! Descriptor-pinned source snapshot for one preset export

use std::collections::BTreeMap;
use std::os::fd::OwnedFd;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use unixnotis_core::Config;

use super::super::archive::MAX_PRESET_FILE_BYTES;
use super::super::config_root::SecureFileCapture;
use super::super::filesystem::{
    ensure_dir_fd_matches_live_path, open_secure_dir_all, read_relative_file_secure_bounded,
};

pub(super) struct ExportSourceSnapshot {
    // One open root keeps validation, dependency scans, and archive reads on the same directory
    root_fd: OwnedFd,
    // Configuration bytes are retained so icon discovery uses the exact parsed source
    config_bytes: Vec<u8>,
    // Captures prevent later path replacement from changing bundle contents
    captures: BTreeMap<PathBuf, SecureFileCapture>,
    config: Config,
}

impl ExportSourceSnapshot {
    pub(super) fn capture(config_dir: &Path) -> Result<Self> {
        let root_fd = open_secure_dir_all(config_dir)
            .with_context(|| format!("open config directory {}", config_dir.display()))?;
        let config_relative = PathBuf::from("config.toml");
        let (config_bytes, mode) =
            read_relative_file_secure_bounded(&root_fd, &config_relative, MAX_PRESET_FILE_BYTES)
                .context("securely capture config.toml for preset export")?;
        let config_text = std::str::from_utf8(&config_bytes)
            .context("config.toml is not valid UTF-8 for preset export")?;
        // Parsing the captured bytes ties every later decision to the archived configuration
        let config = Config::parse(config_text).context("parse captured config.toml for export")?;
        let captures = BTreeMap::from([(
            config_relative,
            SecureFileCapture {
                contents: config_bytes.clone(),
                mode,
            },
        )]);
        Ok(Self {
            root_fd,
            config_bytes,
            captures,
            config,
        })
    }

    pub(super) const fn config(&self) -> &Config {
        &self.config
    }

    pub(super) fn config_bytes(&self) -> &[u8] {
        &self.config_bytes
    }

    pub(super) const fn root_fd(&self) -> &OwnedFd {
        &self.root_fd
    }

    pub(super) const fn captures(&self) -> &BTreeMap<PathBuf, SecureFileCapture> {
        &self.captures
    }

    pub(super) fn capture_active_files(
        &mut self,
        config_dir: &Path,
        paths: &[PathBuf],
    ) -> Result<Vec<PathBuf>> {
        let mut relatives = Vec::new();
        for path in paths {
            let relative = path
                .strip_prefix(config_dir)
                .with_context(|| format!("make active file relative: {}", path.display()))?
                .to_path_buf();
            if self.captures.contains_key(&relative) {
                relatives.push(relative);
                continue;
            }

            let metadata = match std::fs::symlink_metadata(path) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => {
                    return Err(error)
                        .with_context(|| format!("inspect active file {}", path.display()))
                }
            };
            if !metadata.is_file() {
                return Err(anyhow!(
                    "active preset file is not a regular file: {}",
                    path.display()
                ));
            }

            // The descriptor read rejects symlink swaps and bounds files before allocation
            let (contents, mode) =
                read_relative_file_secure_bounded(&self.root_fd, &relative, MAX_PRESET_FILE_BYTES)
                    .with_context(|| format!("securely capture active file {}", path.display()))?;
            self.captures
                .insert(relative.clone(), SecureFileCapture { contents, mode });
            relatives.push(relative);
        }
        relatives.sort();
        relatives.dedup();
        Ok(relatives)
    }

    pub(super) fn extend_captures(&mut self, captures: BTreeMap<PathBuf, SecureFileCapture>) {
        // Script captures share the same root and can safely join the immutable source view
        self.captures.extend(captures);
    }

    pub(super) fn ensure_live_root(&self, config_dir: &Path) -> Result<()> {
        ensure_dir_fd_matches_live_path(&self.root_fd, config_dir)
    }
}

#[cfg(test)]
#[path = "tests/source.rs"]
mod tests;
