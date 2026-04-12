use std::path::Path;

use anyhow::{anyhow, Context, Result};
use unixnotis_core::Config;

use super::super::pathing::normalize_lexical_path;
use super::collect::collect_command_references_from_config;
use super::tokens::{first_command_token, is_host_specific_path_token, resolve_command_path_token};
use super::{HostSpecificCommandPath, OutsideCommandPath};

pub(crate) fn collect_outside_command_paths(
    config_dir: &Path,
    config: &Config,
) -> Vec<OutsideCommandPath> {
    let normalized_root = normalize_lexical_path(config_dir);

    collect_command_references_from_config(config)
        .into_iter()
        .filter_map(|reference| {
            let resolved_path = resolve_command_path_token(config_dir, &reference.command)?;
            // Only explicit path commands are checked here
            let normalized_path = normalize_lexical_path(&resolved_path);
            if normalized_path.starts_with(&normalized_root) {
                return None;
            }

            Some(OutsideCommandPath {
                slot: reference.slot,
                command: reference.command,
                resolved_path,
            })
        })
        .collect()
}

pub(crate) fn collect_host_specific_command_paths(
    config_dir: &Path,
    config: &Config,
) -> Vec<HostSpecificCommandPath> {
    let normalized_root = normalize_lexical_path(config_dir);

    collect_command_references_from_config(config)
        .into_iter()
        .filter_map(|reference| {
            let token = first_command_token(&reference.command)?;
            let resolved_path = resolve_command_path_token(config_dir, &reference.command)?;
            let normalized_path = normalize_lexical_path(&resolved_path);
            // Only absolute host-local command paths under the config root are warned here
            if !normalized_path.starts_with(&normalized_root) || !is_host_specific_path_token(token)
            {
                return None;
            }

            Some(HostSpecificCommandPath {
                slot: reference.slot,
                command: reference.command,
                resolved_path,
            })
        })
        .collect()
}

pub(crate) fn validate_config_command_paths_stay_in_root(
    config_dir: &Path,
    config: &Config,
    mode_label: &str,
) -> Result<()> {
    let outside_paths = collect_outside_command_paths(config_dir, config);
    if outside_paths.is_empty() {
        return Ok(());
    }

    let first = &outside_paths[0];
    Err(anyhow!(
        "{} because {} points outside the UnixNotis config directory: {}",
        mode_label,
        first.slot,
        first.command
    ))
}

pub(crate) fn validate_command_paths_in_config_bytes(
    config_dir: &Path,
    config_bytes: &[u8],
    mode_label: &str,
) -> Result<()> {
    let config_text =
        std::str::from_utf8(config_bytes).context("preset config.toml is not valid UTF-8")?;
    let config: Config =
        toml::from_str(config_text).context("parse bundled config.toml for command path checks")?;
    validate_config_command_paths_stay_in_root(config_dir, &config, mode_label)
}
