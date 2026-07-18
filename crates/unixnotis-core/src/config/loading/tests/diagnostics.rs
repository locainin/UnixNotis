use std::io::{self, Write};
use std::sync::{Arc, Mutex};

use super::*;
use crate::{Config, ConfigDiagnosticKind, CURRENT_CONFIG_VERSION};

struct CapturedWriter(Arc<Mutex<Vec<u8>>>);

impl Write for CapturedWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.0
            .lock()
            .map_err(|_poisoned| io::Error::other("captured log lock poisoned"))?
            .write(buffer)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn migration_diagnostic_reports_unversioned_input_without_exposing_text() {
    let diagnostic = migration_diagnostic("[panel]\ntitle = 'private title'\n")
        .expect("unversioned config should report migration");

    assert_eq!(diagnostic.code, "config.schema.migrated");
    assert_eq!(diagnostic.original.as_deref(), Some("0"));
    assert_eq!(
        diagnostic.effective.as_deref(),
        Some(CURRENT_CONFIG_VERSION.to_string().as_str())
    );
    assert!(!diagnostic.message.contains("private title"));
}

#[test]
fn current_schema_produces_no_migration_diagnostic() {
    let input = format!("config_version = {CURRENT_CONFIG_VERSION}\n");

    assert!(migration_diagnostic(&input).is_none());
}

#[test]
fn adjustment_diagnostics_report_safe_scalar_changes_and_hide_commands() {
    let mut before = Config::default();
    before.widgets.refresh_interval_ms = 1;
    before.widgets.volume.get_cmd = "private-volume-command-sentinel".to_string();
    let mut after = before.clone();
    after.widgets.refresh_interval_ms = 100;
    after.widgets.volume.get_cmd = "pactl get-sink-volume".to_string();

    let diagnostics = adjustment_diagnostics(&before, &after);

    assert!(diagnostics.iter().any(|item| {
        item.path.as_deref() == Some("widgets.refresh_interval_ms")
            && item.original.as_deref() == Some("1")
            && item.effective.as_deref() == Some("100")
    }));
    assert!(diagnostics.iter().any(|item| {
        item.path.as_deref() == Some("widgets.volume.get_cmd")
            && item.code == "config.widgets.volume-backend-selected"
    }));
    let rendered = format!("{diagnostics:?}");
    assert!(!rendered.contains("private-volume-command-sentinel"));
    assert!(!rendered.contains("pactl"));
}

#[test]
fn unknown_key_diagnostic_uses_stable_code_and_warning_kind() {
    let diagnostic = unknown_key_diagnostic("panel.search_visble".to_string());

    assert_eq!(diagnostic.code, "config.unknown-key");
    assert_eq!(diagnostic.kind, ConfigDiagnosticKind::Warning);
    assert_eq!(diagnostic.path.as_deref(), Some("panel.search_visble"));
}

#[test]
fn legacy_migration_reports_each_inserted_compatibility_path() {
    let report = Config::parse_with_report("").expect("empty legacy config should migrate");

    assert!(report.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "config.schema.field-migrated"
            && diagnostic.path.as_deref() == Some("panel.empty_offset_top")
    }));
    assert!(report.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "config.schema.field-migrated"
            && diagnostic.path.as_deref() == Some("media.art_size_px")
    }));
}

#[test]
fn array_adjustments_report_length_and_changed_items_exactly_once() {
    let before = Value::Array(vec![Value::Integer(1)]);
    let after = Value::Array(vec![Value::Integer(2), Value::Integer(3)]);
    let mut diagnostics = Vec::new();

    collect_adjustments("items", Some(&before), Some(&after), &mut diagnostics);

    assert_eq!(diagnostics.len(), 2);
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.path.as_deref() == Some("items")
            && diagnostic.original.as_deref() == Some("1 item(s)")
            && diagnostic.effective.as_deref() == Some("2 item(s)")
    }));
    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.path.as_deref() == Some("items[0]")));
}

#[test]
fn adjustment_codes_cover_each_runtime_correction_family() {
    for path in [
        "widgets.volume.enabled",
        "widgets.volume.get_cmd",
        "widgets.volume.set_cmd",
        "widgets.volume.toggle_cmd",
        "widgets.volume.watch_cmd",
        "widgets.volume.parse_mode",
    ] {
        assert_eq!(
            adjustment_code(path),
            "config.widgets.volume-backend-selected"
        );
    }
    assert_eq!(
        adjustment_code("widgets.brightness.watch_cmd"),
        "config.widgets.brightness-backend-corrected"
    );
    for path in [
        "widgets.refresh_interval_ms",
        "widgets.refresh_interval_slow_ms",
    ] {
        assert_eq!(adjustment_code(path), "config.widgets.refresh-clamped");
    }
    for path in ["history.max_entries", "history.max_active"] {
        assert_eq!(adjustment_code(path), "config.history.limit-clamped");
    }
    for path in [
        "widgets.toggles[0].plugin",
        "widgets.stats[0].plugin",
        "widgets.cards[0].plugin",
    ] {
        assert_eq!(adjustment_code(path), "config.widget.plugin-disabled");
    }
    for path in [
        "widgets.toggles[0].label",
        "widgets.stats[0].label",
        "widgets.cards[0].label",
    ] {
        assert_eq!(adjustment_code(path), "config.widgets.value-adjusted");
    }
    for path in ["widgets.volume.min", "widgets.brightness.min"] {
        assert_eq!(adjustment_code(path), "config.widgets.slider-adjusted");
    }
    for (path, code) in [
        ("panel.width", "config.panel.value-adjusted"),
        ("popups.width", "config.popups.value-adjusted"),
        ("media.art_size_px", "config.media.value-adjusted"),
        ("theme.border_width", "config.theme.value-adjusted"),
        ("general.dnd_default", "config.value-adjusted"),
    ] {
        assert_eq!(adjustment_code(path), code);
    }
}

#[test]
fn adjustment_messages_are_specific_and_never_empty() {
    for (path, expected) in [
        (
            "widgets.stats[0].plugin",
            "Invalid widget plugin configuration was disabled",
        ),
        (
            "widgets.refresh_interval_ms",
            "Refresh interval was adjusted to a safe runtime value",
        ),
        (
            "history.max_entries",
            "History limit was adjusted to a safe runtime value",
        ),
        (
            "widgets.toggle_columns",
            "Widget configuration was adjusted before use",
        ),
        ("panel.width", "Panel configuration was adjusted before use"),
        (
            "popups.width",
            "Popup configuration was adjusted before use",
        ),
        (
            "media.art_size_px",
            "Media configuration was adjusted before use",
        ),
        (
            "theme.border_width",
            "Theme configuration was adjusted before use",
        ),
        (
            "general.dnd_default",
            "Configuration value was adjusted before use",
        ),
    ] {
        assert_eq!(adjustment_message(path), expected);
    }
}

#[test]
fn safe_values_distinguish_finite_and_non_finite_numbers() {
    assert_eq!(safe_value(&Value::Float(1.25)), "1.25");
    assert_eq!(safe_value(&Value::Float(f64::NAN)), "non-finite number");
    assert_eq!(
        safe_value(&Value::Float(f64::INFINITY)),
        "non-finite number"
    );
}

#[test]
fn compatibility_logger_emits_each_diagnostic() {
    let output = Arc::new(Mutex::new(Vec::new()));
    let writer_output = output.clone();
    let subscriber = tracing_subscriber::fmt()
        .without_time()
        .with_ansi(false)
        .with_writer(move || CapturedWriter(writer_output.clone()))
        .finish();
    let diagnostics = vec![
        unknown_key_diagnostic("panel.unknown".to_string()),
        migrated_field_diagnostic("panel.width".to_string()),
    ];

    tracing::subscriber::with_default(subscriber, || {
        log_config_diagnostics(&diagnostics);
    });

    let rendered = String::from_utf8(
        output
            .lock()
            .expect("lock captured diagnostic output")
            .clone(),
    )
    .expect("diagnostic output should be UTF-8");
    assert!(rendered.contains("config.unknown-key"));
    assert!(rendered.contains("config.schema.field-migrated"));
}
