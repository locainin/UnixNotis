//! Configured icon asset discovery for portable exports

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use toml::Value;
use unixnotis_core::validate_icon_asset_reference;

pub(super) fn collect_existing_icon_assets(
    config_path: &Path,
    config_dir: &Path,
) -> Result<Vec<PathBuf>> {
    let config_text = std::fs::read_to_string(config_path)
        .with_context(|| format!("read config file {}", config_path.display()))?;
    let value: Value = toml::from_str(&config_text).context("parse config.toml icon assets")?;
    let mut raw_assets = Vec::new();
    collect_icon_asset_values(&value, &mut raw_assets);

    let mut paths = Vec::new();
    for asset in raw_assets {
        // Validate before joining so an icon cannot escape the portable root
        validate_icon_asset_reference(&asset)
            .with_context(|| format!("validate configured icon asset {asset}"))?;
        let relative = PathBuf::from(asset);
        if config_dir.join(&relative).is_file() {
            paths.push(relative);
        }
    }
    paths.sort();
    paths.dedup();
    Ok(paths)
}

fn collect_icon_asset_values(value: &Value, assets: &mut Vec<String>) {
    match value {
        Value::Table(table) => {
            for (key, child) in table {
                if key == "icon_asset" {
                    if let Some(asset) = child.as_str().filter(|asset| !asset.trim().is_empty()) {
                        assets.push(asset.trim().to_string());
                    }
                }
                // Nested widget and media tables can each carry icon assets
                collect_icon_asset_values(child, assets);
            }
        }
        Value::Array(items) => {
            for child in items {
                collect_icon_asset_values(child, assets);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
#[path = "tests/assets.rs"]
mod tests;
