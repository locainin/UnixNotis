//! Explicit configuration schema migrations

use serde::de::IntoDeserializer;

use super::{Config, CURRENT_CONFIG_VERSION};

pub(super) fn deserialize_config(contents: &str) -> Result<(Config, Vec<String>), String> {
    let mut document = contents
        .parse::<toml::Value>()
        .map_err(|err| err.to_string())?;
    let migration = migrate_document(&mut document)?;
    let mut ignored_keys = Vec::new();
    let deserializer = document.into_deserializer();
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
    Ok((config, ignored_keys))
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
        return Err(format!(
            "config version {version} is newer than supported version {CURRENT_CONFIG_VERSION}"
        ));
    }

    let result = match version {
        0 | 1 => migrate_legacy_layout(root),
        CURRENT_CONFIG_VERSION => MigrationResult::default(),
        _ => return Err(format!("unsupported config version {version}")),
    };
    root.insert(
        "config_version".to_string(),
        toml::Value::Integer(i64::from(CURRENT_CONFIG_VERSION)),
    );
    Ok(result)
}

fn migrate_legacy_layout(root: &mut toml::Table) -> MigrationResult {
    if let Some(panel) = child_table(root, "panel") {
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
    if let Some(widgets) = child_table(root, "widgets") {
        insert_string(widgets, "density", "comfortable");
        insert_integer(widgets, "toggle_columns", 4);
        insert_integer(widgets, "stat_columns", 2);
        insert_integer(widgets, "card_columns", 2);
        restore_legacy_cards = !widgets.contains_key("cards");
        for slider_name in ["volume", "brightness"] {
            if let Some(slider) = child_table(widgets, slider_name) {
                insert_integer(slider, "segments", 0);
                insert_bool(slider, "show_sublabels", false);
                insert_string(slider, "sublabel_min", "");
                insert_string(slider, "sublabel_max", "");
            }
        }
    }

    if let Some(media) = child_table(root, "media") {
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

fn child_table<'a>(table: &'a mut toml::Table, key: &str) -> Option<&'a mut toml::Table> {
    table.get_mut(key)?.as_table_mut()
}

fn insert_string(table: &mut toml::Table, key: &str, value: &str) {
    table
        .entry(key.to_string())
        .or_insert_with(|| toml::Value::String(value.to_string()));
}

fn insert_integer(table: &mut toml::Table, key: &str, value: i64) {
    table
        .entry(key.to_string())
        .or_insert(toml::Value::Integer(value));
}

fn insert_bool(table: &mut toml::Table, key: &str, value: bool) {
    table
        .entry(key.to_string())
        .or_insert(toml::Value::Boolean(value));
}

fn insert_strings(table: &mut toml::Table, key: &str, values: &[&str]) {
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
