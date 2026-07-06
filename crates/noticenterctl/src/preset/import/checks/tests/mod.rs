use super::{
    collect_imported_exec_content, validate_imported_command_paths_stay_in_root,
    validate_imported_icon_asset_references, validate_imported_theme_paths_stay_in_root,
};
use crate::preset::archive::BundleFile;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static TEST_TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

fn temp_root(name: &str) -> PathBuf {
    // Unique absolute paths keep these lexical checks stable under parallel cargo runs
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock moved backwards")
        .as_nanos();
    let serial = TEST_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "unixnotis-preset-import-checks-{}-{}-{}",
        name, stamp, serial
    ))
}

#[test]
fn imported_theme_checks_reject_parent_traversal_targets() {
    // `../` theme paths should be treated the same as any other root escape
    let config_dir = temp_root("relative-escape");
    let config = b"[theme]\nbase_css = \"../escaped-base.css\"\npanel_css = \"panel.css\"\npopup_css = \"popup.css\"\nwidgets_css = \"widgets.css\"\nmedia_css = \"media.css\"\n";

    let error = validate_imported_theme_paths_stay_in_root(&config_dir, config)
        .expect_err("reject relative theme escape");

    assert!(error
        .to_string()
        .contains("tries to leave the UnixNotis config directory"));
}

#[test]
fn imported_command_checks_reject_absolute_plugin_command() {
    // Shared presets should not carry explicit command paths that leave the config root
    let config_dir = temp_root("outside-command");
    let config = b"[theme]\nbase_css = \"base.css\"\n[[widgets.stats]]\nlabel = \"Probe\"\n[widgets.stats.plugin]\napi_version = 1\ncommand = \"/tmp/outside-plugin\"\n";

    let error = validate_imported_command_paths_stay_in_root(&config_dir, config)
        .expect_err("reject outside command path");

    assert!(error
        .to_string()
        .contains("points outside the UnixNotis config directory"));
}

#[test]
fn imported_icon_asset_checks_accept_relative_visual_asset() {
    let config = br#"
[[widgets.stats]]
label = "RAM"
icon = "drive-harddisk-symbolic"
icon_asset = "assets/ram.svg"
"#;

    validate_imported_icon_asset_references(config).expect("valid icon asset reference");
}

#[test]
fn imported_icon_asset_checks_reject_escape_and_absolute_targets() {
    let escape = br#"
[[widgets.stats]]
label = "RAM"
icon_asset = "../evil.svg"
"#;
    let absolute = br#"
[[widgets.cards]]
title = "Weather"
icon_asset = "/etc/passwd"
"#;

    assert!(validate_imported_icon_asset_references(escape).is_err());
    assert!(validate_imported_icon_asset_references(absolute).is_err());
}

#[test]
fn imported_icon_asset_checks_reject_remote_url_and_script_extension() {
    let remote = br#"
[[widgets.toggles]]
label = "Remote"
icon_asset = "https://example.com/icon.svg"
"#;
    let script = br#"
[[widgets.cards]]
title = "Run"
icon_asset = "assets/run.sh"
"#;

    assert!(validate_imported_icon_asset_references(remote).is_err());
    assert!(validate_imported_icon_asset_references(script).is_err());
}

#[test]
fn imported_exec_collection_finds_command_bearing_config() {
    let config = br#"
[theme]
base_css = "base.css"
[[widgets.stats]]
label = "Probe"
cmd = "scripts/check.sh"
"#;

    let content = collect_imported_exec_content(config, &[]).expect("collect exec content");

    assert_eq!(content.commands.len(), 1);
    assert_eq!(content.commands[0].slot, "widgets.stats[0].cmd");
    assert_eq!(content.commands[0].command, "scripts/check.sh");
}

#[test]
fn imported_exec_collection_finds_script_payloads() {
    let config = br#"
[theme]
base_css = "base.css"
"#;
    let bundle_files = vec![BundleFile {
        relative_path: PathBuf::from("scripts/demo-widget"),
        contents: b"#!/bin/sh\necho ok\n".to_vec(),
        mode: 0o755,
    }];

    let content =
        collect_imported_exec_content(config, &bundle_files).expect("collect script payload");

    assert_eq!(content.files.len(), 1);
    assert_eq!(
        content.files[0].relative_path,
        PathBuf::from("scripts/demo-widget")
    );
}

#[test]
fn imported_exec_collection_keeps_command_and_script_details() {
    let config = br#"
[theme]
base_css = "base.css"
[[widgets.stats]]
label = "Probe"
cmd = "scripts/check.sh"
"#;
    let bundle_files = vec![BundleFile {
        relative_path: PathBuf::from("scripts/check.sh"),
        contents: b"#!/bin/sh\necho ok\n".to_vec(),
        mode: 0o755,
    }];

    let content =
        collect_imported_exec_content(config, &bundle_files).expect("collect trusted exec");

    assert_eq!(content.commands.len(), 1);
    assert_eq!(content.files.len(), 1);
}
