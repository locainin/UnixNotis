//! Shared process-environment guards for CLI tests

use std::borrow::Cow;
use std::ffi::{OsStr, OsString};
use std::path::Path;
use std::sync::{Mutex, MutexGuard, OnceLock};

use unixnotis_core::{parse_legacy_command, CURRENT_CONFIG_VERSION};

pub fn current_config_text(contents: &str) -> String {
    let Ok(mut document) = toml::from_str::<toml::Value>(contents) else {
        // Invalid fixtures stay invalid while still crossing the version gate first
        return format!("config_version = {CURRENT_CONFIG_VERSION}\n{contents}");
    };
    let Some(root) = document.as_table_mut() else {
        return format!("config_version = {CURRENT_CONFIG_VERSION}\n{contents}");
    };
    root.insert(
        "config_version".to_string(),
        toml::Value::Integer(i64::from(CURRENT_CONFIG_VERSION)),
    );
    normalize_fixture_commands(root);
    toml::to_string(&document).expect("serialize current-schema config fixture")
}

pub fn current_config_bytes(contents: &[u8]) -> Vec<u8> {
    if let Ok(contents) = std::str::from_utf8(contents) {
        current_config_text(contents).into_bytes()
    } else {
        // Invalid UTF-8 remains visible to the parser after the schema prefix
        let mut config = format!("config_version = {CURRENT_CONFIG_VERSION}\n").into_bytes();
        config.extend_from_slice(contents);
        config
    }
}

pub fn fixture_file_contents<'a>(relative_path: &str, contents: &'a str) -> Cow<'a, str> {
    let is_config = Path::new(relative_path).file_name() == Some(OsStr::new("config.toml"));
    let has_version = contents
        .lines()
        .any(|line| line.trim_start().starts_with("config_version"));
    if is_config && !has_version {
        // Functional config fixtures always exercise the schema shipped by this test binary
        Cow::Owned(current_config_text(contents))
    } else {
        Cow::Borrowed(contents)
    }
}

fn normalize_fixture_commands(root: &mut toml::Table) {
    let Some(widgets) = root.get_mut("widgets").and_then(toml::Value::as_table_mut) else {
        return;
    };

    // Slider command fields live in fixed widget tables
    for slider_name in ["volume", "brightness"] {
        let Some(slider) = widgets
            .get_mut(slider_name)
            .and_then(toml::Value::as_table_mut)
        else {
            continue;
        };
        normalize_table_commands(slider, &["get_cmd", "set_cmd", "toggle_cmd", "watch_cmd"]);
    }

    normalize_widget_array_commands(
        widgets,
        "toggles",
        &["state_cmd", "toggle_cmd", "on_cmd", "off_cmd", "watch_cmd"],
        false,
    );
    normalize_widget_array_commands(widgets, "stats", &["cmd"], true);
    normalize_widget_array_commands(widgets, "cards", &["cmd"], true);
}

fn normalize_widget_array_commands(
    widgets: &mut toml::Table,
    collection_name: &str,
    fields: &[&str],
    has_plugin: bool,
) {
    let Some(items) = widgets
        .get_mut(collection_name)
        .and_then(toml::Value::as_array_mut)
    else {
        return;
    };
    for item in items {
        let Some(table) = item.as_table_mut() else {
            continue;
        };
        normalize_table_commands(table, fields);
        if has_plugin {
            let Some(plugin) = table.get_mut("plugin").and_then(toml::Value::as_table_mut) else {
                continue;
            };
            normalize_table_commands(plugin, &["command"]);
        }
    }
}

fn normalize_table_commands(table: &mut toml::Table, fields: &[&str]) {
    for field in fields {
        let Some(command) = table.get(*field).and_then(toml::Value::as_str) else {
            continue;
        };
        let Ok(spec) = parse_legacy_command(command) else {
            continue;
        };
        let value = toml::Value::try_from(spec).expect("serialize command fixture");
        table.insert((*field).to_string(), value);
    }
}

pub fn test_env_lock() -> MutexGuard<'static, ()> {
    // Every test that mutates process environment must share this one lock
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

pub struct EnvGuard {
    name: &'static str,
    previous: Option<OsString>,
}

impl EnvGuard {
    pub fn set(name: &'static str, value: impl AsRef<OsStr>) -> Self {
        let previous = std::env::var_os(name);
        std::env::set_var(name, value);
        Self { name, previous }
    }

    pub fn remove(name: &'static str) -> Self {
        let previous = std::env::var_os(name);
        std::env::remove_var(name);
        Self { name, previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        // Restoring in Drop isolates later tests even when an assertion unwinds
        match self.previous.take() {
            Some(value) => std::env::set_var(self.name, value),
            None => std::env::remove_var(self.name),
        }
    }
}
