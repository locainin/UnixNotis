//! Explicit configuration schema migrations

use serde::de::IntoDeserializer;

use super::super::{Config, CURRENT_CONFIG_VERSION};
use crate::{parse_legacy_command, CommandSpec};

pub(in crate::config) fn deserialize_config_with_migrations(
    contents: &str,
) -> Result<(Config, Vec<String>, Vec<String>), String> {
    // Keep the original tree so migration reporting can describe every inserted field
    let mut document = contents
        .parse::<toml::Value>()
        .map_err(|err| err.to_string())?;
    let original_document = document.clone();
    let migration = migrate_document(&mut document)?;
    let mut migrated_paths = Vec::new();
    collect_changed_paths(
        "",
        Some(&original_document),
        Some(&document),
        &mut migrated_paths,
    );
    if migration.restore_legacy_cards {
        // Card restoration changes the typed config after the document migration finishes
        // Card restoration happens after deserialization, so record it outside the TOML diff
        migrated_paths.push("widgets.cards".to_string());
    }
    migrated_paths.sort_unstable();
    migrated_paths.dedup();
    let mut ignored_keys = Vec::new();
    let deserializer = document.into_deserializer();
    // Unknown fields are collected without weakening normal serde type validation
    let mut config: Config = serde_ignored::deserialize(deserializer, |path| {
        ignored_keys.push(path.to_string());
    })
    .map_err(|err| err.to_string())?;

    // Older configs enabled the original calendar and weather cards when the key was absent
    if migration.restore_legacy_cards {
        config.widgets.cards = Config::default().widgets.cards;
        for card in &mut config.widgets.cards {
            card.enabled = true;
        }
    }
    config.config_version = CURRENT_CONFIG_VERSION;
    Ok((config, ignored_keys, migrated_paths))
}

fn collect_changed_paths(
    path: &str,
    before: Option<&toml::Value>,
    after: Option<&toml::Value>,
    paths: &mut Vec<String>,
) {
    if before == after {
        return;
    }
    match (before, after) {
        (Some(toml::Value::Table(before)), Some(toml::Value::Table(after))) => {
            // Union traversal catches inserted, removed, and changed child keys
            let mut keys = before.keys().chain(after.keys()).collect::<Vec<_>>();
            keys.sort_unstable();
            keys.dedup();
            for key in keys {
                let child = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                collect_changed_paths(&child, before.get(key), after.get(key), paths);
            }
        }
        (None, Some(toml::Value::Table(after))) => {
            // Newly created compatibility tables report their leaf fields instead of one table
            for (key, value) in after {
                let child = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                collect_changed_paths(&child, None, Some(value), paths);
            }
        }
        _ if !path.is_empty() => paths.push(path.to_string()),
        // The root itself is not a useful config-key path
        _ => {}
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct MigrationResult {
    restore_legacy_cards: bool,
}

fn migrate_document(document: &mut toml::Value) -> Result<MigrationResult, String> {
    let root = document
        .as_table_mut()
        .ok_or_else(|| "configuration root must be a TOML table".to_string())?;
    let version = match root.get("config_version") {
        None => 0,
        Some(toml::Value::Integer(version)) if *version >= 0 => *version as u32,
        Some(_) => return Err("config_version must be a non-negative integer".to_string()),
    };
    if version > CURRENT_CONFIG_VERSION {
        // Future schemas fail closed because silently dropping fields would corrupt intent
        return Err(format!(
            "config version {version} is newer than supported version {CURRENT_CONFIG_VERSION}"
        ));
    }

    let result = match version {
        // Schema one used the same legacy layout compatibility values as unversioned files
        0 | 1 => {
            let result = migrate_legacy_layout(root);
            migrate_legacy_commands(root)?;
            result
        }
        2 => {
            migrate_legacy_commands(root)?;
            MigrationResult::default()
        }
        3 | CURRENT_CONFIG_VERSION => MigrationResult::default(),
        _ => return Err(format!("unsupported config version {version}")),
    };
    // Only an absent field receives the current default. An explicit policy,
    // including an empty exact allowlist, remains the user's decision.
    ensure_media_art_policy_default(root);
    root.insert(
        "config_version".to_string(),
        toml::Value::Integer(i64::from(CURRENT_CONFIG_VERSION)),
    );
    Ok(result)
}

fn ensure_media_art_policy_default(root: &mut toml::Table) {
    let Some(media) = root.get_mut("media").and_then(toml::Value::as_table_mut) else {
        return;
    };

    // A missing policy receives the current native-art default before serde defaults run
    if media.contains_key("local_art_policy") {
        return;
    }
    media.insert(
        "local_art_policy".to_string(),
        toml::Value::String("all_admitted".to_string()),
    );
}

fn migrate_legacy_commands(root: &mut toml::Table) -> Result<(), String> {
    let Some(widgets) = root.get_mut("widgets").and_then(toml::Value::as_table_mut) else {
        return Ok(());
    };

    for slider_name in ["volume", "brightness"] {
        let Some(slider) = widgets
            .get_mut(slider_name)
            .and_then(toml::Value::as_table_mut)
        else {
            continue;
        };
        for field in ["get_cmd", "set_cmd", "toggle_cmd", "watch_cmd"] {
            migrate_command_field(slider, field, &format!("widgets.{slider_name}.{field}"))?;
        }
    }

    migrate_command_array(
        widgets,
        "toggles",
        &["state_cmd", "toggle_cmd", "on_cmd", "off_cmd", "watch_cmd"],
    )?;
    for collection in ["stats", "cards"] {
        migrate_command_array(widgets, collection, &["cmd"])?;
        migrate_plugin_commands(widgets, collection)?;
    }
    Ok(())
}

fn migrate_command_array(
    widgets: &mut toml::Table,
    collection: &str,
    fields: &[&str],
) -> Result<(), String> {
    let Some(entries) = widgets
        .get_mut(collection)
        .and_then(toml::Value::as_array_mut)
    else {
        return Ok(());
    };
    for (index, entry) in entries.iter_mut().enumerate() {
        let Some(table) = entry.as_table_mut() else {
            continue;
        };
        for field in fields {
            migrate_command_field(
                table,
                field,
                &format!("widgets.{collection}[{index}].{field}"),
            )?;
        }
    }
    Ok(())
}

fn migrate_plugin_commands(widgets: &mut toml::Table, collection: &str) -> Result<(), String> {
    let Some(entries) = widgets
        .get_mut(collection)
        .and_then(toml::Value::as_array_mut)
    else {
        return Ok(());
    };
    for (index, entry) in entries.iter_mut().enumerate() {
        let Some(plugin) = entry
            .as_table_mut()
            .and_then(|table| table.get_mut("plugin"))
            .and_then(toml::Value::as_table_mut)
        else {
            continue;
        };
        migrate_command_field(
            plugin,
            "command",
            &format!("widgets.{collection}[{index}].plugin.command"),
        )?;
    }
    Ok(())
}

fn migrate_command_field(table: &mut toml::Table, field: &str, path: &str) -> Result<(), String> {
    let Some(value) = table.get_mut(field) else {
        return Ok(());
    };
    let Some(command) = value.as_str() else {
        return Ok(());
    };
    let spec = if command.trim().is_empty() {
        CommandSpec::direct("", std::iter::empty::<&str>())
    } else {
        parse_legacy_command(command)
            .map_err(|error| format!("failed to migrate {path}: {error}"))?
    };
    *value = toml::Value::try_from(spec)
        .map_err(|error| format!("failed to migrate {path}: {error}"))?;
    Ok(())
}

fn migrate_legacy_layout(root: &mut toml::Table) -> MigrationResult {
    // Missing legacy tables still represent omitted old fields, not a request for new defaults
    if let Some(panel) = child_table_or_insert(root, "panel") {
        insert_string(panel, "quick_actions_label", "");
        insert_string(panel, "system_status_label", "");
        insert_integer(panel, "empty_offset_top", 120);
        insert_strings(panel, "section_order", &["widgets", "notifications"]);
        insert_strings(
            panel,
            "widget_order",
            &["sliders", "media", "toggles", "stats", "cards"],
        );
    }

    let mut restore_legacy_cards = false;
    if let Some(widgets) = child_table_or_insert(root, "widgets") {
        insert_string(widgets, "density", "comfortable");
        insert_integer(widgets, "toggle_columns", 4);
        insert_integer(widgets, "stat_columns", 2);
        insert_integer(widgets, "card_columns", 2);
        restore_legacy_cards = !widgets.contains_key("cards");
        for slider_name in ["volume", "brightness"] {
            // Both sliders existed in the old effective config even when their tables were omitted
            if let Some(slider) = child_table_or_insert(widgets, slider_name) {
                insert_integer(slider, "segments", 0);
                insert_bool(slider, "show_sublabels", false);
                insert_string(slider, "sublabel_min", "");
                insert_string(slider, "sublabel_max", "");
            }
        }
    }

    if let Some(media) = child_table_or_insert(root, "media") {
        insert_integer(media, "art_size_px", 50);
        insert_integer(media, "text_width_floor_px", 140);
        insert_integer(media, "content_spacing_px", 10);
        insert_integer(media, "control_spacing_px", 6);
        insert_integer(media, "navigation_spacing_px", 6);
    }

    MigrationResult {
        restore_legacy_cards,
    }
}

fn child_table_or_insert<'a>(table: &'a mut toml::Table, key: &str) -> Option<&'a mut toml::Table> {
    // Existing invalid scalar values stay intact so deserialization can report the real type error
    table
        .entry(key.to_string())
        .or_insert_with(|| toml::Value::Table(toml::Table::new()))
        .as_table_mut()
}

fn insert_string(table: &mut toml::Table, key: &str, value: &str) {
    // Explicit user values always win over compatibility defaults
    table
        .entry(key.to_string())
        .or_insert_with(|| toml::Value::String(value.to_string()));
}

fn insert_integer(table: &mut toml::Table, key: &str, value: i64) {
    // Entry insertion preserves existing values including values later rejected by serde
    table
        .entry(key.to_string())
        .or_insert(toml::Value::Integer(value));
}

fn insert_bool(table: &mut toml::Table, key: &str, value: bool) {
    // Missing booleans receive legacy behavior without rewriting explicit false values
    table
        .entry(key.to_string())
        .or_insert(toml::Value::Boolean(value));
}

fn insert_strings(table: &mut toml::Table, key: &str, values: &[&str]) {
    // Ordered arrays preserve the historic panel and widget placement
    table.entry(key.to_string()).or_insert_with(|| {
        toml::Value::Array(
            values
                .iter()
                .map(|value| toml::Value::String((*value).to_string()))
                .collect(),
        )
    });
}

#[cfg(test)]
#[path = "tests/schema.rs"]
mod tests;
