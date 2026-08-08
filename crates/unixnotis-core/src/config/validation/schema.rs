//! Current configuration schema deserialization

use serde::de::IntoDeserializer;

use super::super::{Config, CURRENT_CONFIG_VERSION};

pub(in crate::config) fn deserialize_current_config(
    contents: &str,
) -> Result<(Config, Vec<String>), String> {
    let document = contents
        .parse::<toml::Value>()
        .map_err(|error| error.to_string())?;
    validate_current_version(&document)?;

    let mut ignored_keys = Vec::new();
    let deserializer = document.into_deserializer();
    // Unknown fields remain visible to diagnostics without weakening serde validation
    let config = serde_ignored::deserialize(deserializer, |path| {
        // This runtime-only field is intentionally ignored for stock theme compatibility
        let path = path.to_string();
        if path != "theme.mode" {
            ignored_keys.push(path);
        }
    })
    .map_err(|error| error.to_string())?;
    Ok((config, ignored_keys))
}

fn validate_current_version(document: &toml::Value) -> Result<(), String> {
    let root = document
        .as_table()
        .ok_or_else(|| "configuration root must be a TOML table".to_string())?;
    let version = match root.get("config_version") {
        None => 0,
        Some(toml::Value::Integer(version)) if *version >= 0 => u32::try_from(*version)
            .map_err(|_error| format!("unsupported config version {version}"))?,
        Some(_) => return Err("config_version must be a non-negative integer".to_string()),
    };
    if version != CURRENT_CONFIG_VERSION {
        // A clean schema break prevents old fields from receiving silently changed semantics
        return Err(format!("unsupported config version {version}"));
    }
    Ok(())
}

#[cfg(test)]
#[path = "tests/schema.rs"]
mod tests;
