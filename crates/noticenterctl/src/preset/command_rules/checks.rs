use std::path::Path;

use anyhow::{anyhow, Context, Result};
use unixnotis_core::{parse_command, Config};

use super::super::pathing::normalize_lexical_path;
use super::collect::collect_command_references_from_config;
use super::tokens::{
    collect_outside_env_path_tokens, first_command_token, is_host_specific_path_token,
    resolve_command_path_token, validate_env_command_layout, validate_env_path_semantics,
};
use super::{HostSpecificCommandPath, OutsideCommandPath};

pub fn collect_outside_command_paths(
    config_dir: &Path,
    config: &Config,
) -> Vec<OutsideCommandPath> {
    // Lexical comparison avoids requiring referenced files to exist during review
    let normalized_root = normalize_lexical_path(config_dir);

    collect_command_references_from_config(config)
        .into_iter()
        .flat_map(|reference| {
            let mut outside = Vec::new();
            if let Some(resolved_path) = resolve_command_path_token(config_dir, &reference.command)
            {
                // Only explicit path commands are checked here
                let normalized_path = normalize_lexical_path(&resolved_path);
                if !normalized_path.starts_with(&normalized_root) {
                    outside.push(OutsideCommandPath {
                        slot: reference.slot.clone(),
                        command: reference.command.clone(),
                        resolved_path,
                    });
                }
            }

            // Loader-sensitive environment paths receive the same containment check
            outside.extend(
                collect_outside_env_path_tokens(config_dir, &reference.command)
                    .into_iter()
                    .map(|(_name, resolved_path)| OutsideCommandPath {
                        slot: reference.slot.clone(),
                        command: reference.command.clone(),
                        resolved_path,
                    }),
            );
            outside
        })
        .collect()
}

pub fn collect_host_specific_command_paths(
    config_dir: &Path,
    config: &Config,
) -> Vec<HostSpecificCommandPath> {
    // Host-specific paths inside the bundle remain portable warnings rather than escapes
    let normalized_root = normalize_lexical_path(config_dir);

    collect_command_references_from_config(config)
        .into_iter()
        .filter_map(|reference| {
            let token = first_command_token(&reference.command)?;
            let resolved_path = resolve_command_path_token(config_dir, &reference.command)?;
            let normalized_path = normalize_lexical_path(&resolved_path);
            // Only absolute host-local command paths under the config root are warned here
            if !normalized_path.starts_with(&normalized_root)
                || !is_host_specific_path_token(&token)
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

pub fn validate_config_command_paths_stay_in_root(
    config_dir: &Path,
    config: &Config,
    mode_label: &str,
) -> Result<()> {
    // Wrapper validation runs before path collection so ambiguous env forms fail closed
    for reference in collect_command_references_from_config(config) {
        let parsed = parse_command(&reference.command).with_context(|| {
            format!(
                "{mode_label} because {} contains an invalid command",
                reference.slot
            )
        })?;
        validate_env_command_layout(&parsed).map_err(|reason| {
            anyhow!(
                "{mode_label} because {} contains an unsafe env wrapper: {reason}",
                reference.slot
            )
        })?;
        validate_env_path_semantics(&parsed).map_err(|reason| {
            anyhow!(
                "{mode_label} because {} contains unsafe environment path semantics: {reason}",
                reference.slot
            )
        })?;
    }

    // A single stable error avoids exposing every configured command in normal output
    let outside_paths = collect_outside_command_paths(config_dir, config);
    if outside_paths.is_empty() {
        return Ok(());
    }

    let first = &outside_paths[0];
    Err(anyhow!(
        "{} because {} resolves outside the UnixNotis config directory",
        mode_label,
        first.slot
    ))
}

pub fn validate_command_paths_in_config_bytes(
    config_dir: &Path,
    config_bytes: &[u8],
    mode_label: &str,
) -> Result<()> {
    // Byte validation is used before imported configuration reaches the live directory
    let config_text =
        std::str::from_utf8(config_bytes).context("preset config.toml is not valid UTF-8")?;
    let config: Config =
        toml::from_str(config_text).context("parse bundled config.toml for command path checks")?;
    validate_config_command_paths_stay_in_root(config_dir, &config, mode_label)
}
