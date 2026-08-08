use super::super::checks::{
    collect_imported_exec_content, validate_imported_command_paths_stay_in_root,
    validate_imported_icon_assets, validate_imported_theme_paths_stay_in_root,
};
use crate::preset::archive::BundleFile;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::test_support::current_config_bytes;

static TEST_TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

fn temp_root(name: &str) -> PathBuf {
    // Unique absolute paths keep these lexical checks stable under parallel cargo runs
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock moved backwards")
        .as_nanos();
    let serial = TEST_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "unixnotis-preset-import-checks-{name}-{stamp}-{serial}"
    ))
}

#[test]
fn imported_theme_checks_reject_parent_traversal_targets() {
    // `../` theme paths should be treated the same as any other root escape
    let config_dir = temp_root("relative-escape");
    let config = b"[theme]\nbase_css = \"../escaped-base.css\"\npanel_css = \"panel.css\"\npopup_css = \"popup.css\"\nwidgets_css = \"widgets.css\"\nmedia_css = \"media.css\"\n";

    let error =
        validate_imported_theme_paths_stay_in_root(&config_dir, &current_config_bytes(config))
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

    let error =
        validate_imported_command_paths_stay_in_root(&config_dir, &current_config_bytes(config))
            .expect_err("reject outside command path");

    assert!(error
        .to_string()
        .contains("resolves outside the UnixNotis config directory"));
}

#[test]
fn imported_icon_asset_checks_accept_relative_visual_asset() {
    let config = br#"
[[widgets.stats]]
label = "RAM"
icon = "drive-harddisk-symbolic"
icon_asset = "assets/ram.svg"
"#;

    validate_imported_icon_assets(config, &[]).expect("valid optional icon asset reference");
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

    assert!(validate_imported_icon_assets(escape, &[]).is_err());
    assert!(validate_imported_icon_assets(absolute, &[]).is_err());
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

    assert!(validate_imported_icon_assets(remote, &[]).is_err());
    assert!(validate_imported_icon_assets(script, &[]).is_err());
}

#[test]
fn imported_icon_asset_checks_reject_corrupt_oversized_and_executable_payloads() {
    let config = br#"
[[widgets.stats]]
label = "RAM"
icon_asset = "assets/ram.svg"
"#;
    let file = |contents: Vec<u8>, mode| BundleFile {
        relative_path: PathBuf::from("assets/ram.svg"),
        contents,
        mode,
    };

    assert!(validate_imported_icon_assets(config, &[file(b"not svg".to_vec(), 0o644)]).is_err());
    assert!(validate_imported_icon_assets(config, &[file(vec![b' '; 2_097_153], 0o644)]).is_err());
    assert!(validate_imported_icon_assets(
        config,
        &[file(
            br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16"/>"#.to_vec(),
            0o755,
        )],
    )
    .is_err());
}

#[test]
fn imported_icon_asset_checks_accept_valid_bounded_svg_payload() {
    let config = br#"
[[widgets.stats]]
label = "RAM"
icon_asset = "assets/ram.svg"
"#;
    let files = [BundleFile {
        relative_path: PathBuf::from("assets/ram.svg"),
        contents: br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16"/>"#.to_vec(),
        mode: 0o644,
    }];

    validate_imported_icon_assets(config, &files).expect("safe bundled SVG");
}

#[test]
fn imported_icon_asset_checks_reject_namespaced_external_image_nodes() {
    let config = br#"
[[widgets.cards]]
title = "Probe"
icon_asset = "assets/probe.svg"
"#;
    let files = [BundleFile {
        relative_path: PathBuf::from("assets/probe.svg"),
        contents: br#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:s="http://www.w3.org/2000/svg" width="16" height="16"><s:image href="/tmp/external.png" width="16" height="16"/></svg>"#.to_vec(),
        mode: 0o644,
    }];

    let error = validate_imported_icon_assets(config, &files)
        .expect_err("namespaced image elements must not reach usvg's file resolver");

    assert!(error.to_string().contains("unsafe icon asset"));
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

    let content = collect_imported_exec_content(&current_config_bytes(config), &[])
        .expect("collect exec content");

    assert_eq!(content.commands.len(), 1);
    assert_eq!(content.commands[0].slot, "widgets.stats[0].cmd");
    assert_eq!(content.commands[0].command, "scripts/check.sh");
}

#[test]
fn imported_exec_collection_ignores_unknown_command_keys_and_keeps_real_command() {
    let mut config = String::new();
    for index in 0..64 {
        config.push_str(&format!("[aaa{index:02}]\ncmd = \"true\"\n"));
    }
    config.push_str("[[widgets.stats]]\nlabel = \"Probe\"\ncmd = \"sh assets/payload.dat\"\n");

    let content = collect_imported_exec_content(&current_config_bytes(config.as_bytes()), &[])
        .expect("collect only typed command fields");

    assert_eq!(content.commands.len(), 1);
    assert_eq!(content.commands[0].slot, "widgets.stats[0].cmd");
    assert_eq!(content.commands[0].command, "sh assets/payload.dat");
}

#[test]
fn imported_exec_collection_covers_every_known_explicit_command_field() {
    let config = br#"
[widgets.volume]
get_cmd = "volume-get"
set_cmd = "volume-set"
toggle_cmd = "volume-toggle"
watch_cmd = "volume-watch"

[widgets.brightness]
get_cmd = "brightness-get"
set_cmd = "brightness-set"
toggle_cmd = "brightness-toggle"
watch_cmd = "brightness-watch"

[[widgets.toggles]]
state_cmd = "toggle-state"
toggle_cmd = "toggle-action"
on_cmd = "toggle-on"
off_cmd = "toggle-off"
watch_cmd = "toggle-watch"

[[widgets.stats]]
cmd = "stat-command"
[widgets.stats.plugin]
api_version = 1
command = "stat-plugin"

[[widgets.cards]]
cmd = "card-command"
[widgets.cards.plugin]
api_version = 1
command = "card-plugin"
"#;

    let content = collect_imported_exec_content(&current_config_bytes(config), &[])
        .expect("collect known commands");
    let slots = content
        .commands
        .iter()
        .map(|command| command.slot.as_str())
        .collect::<HashSet<_>>();
    let expected = [
        "widgets.volume.get_cmd",
        "widgets.volume.set_cmd",
        "widgets.volume.toggle_cmd",
        "widgets.volume.watch_cmd",
        "widgets.brightness.get_cmd",
        "widgets.brightness.set_cmd",
        "widgets.brightness.toggle_cmd",
        "widgets.brightness.watch_cmd",
        "widgets.toggles[0].state_cmd",
        "widgets.toggles[0].toggle_cmd",
        "widgets.toggles[0].on_cmd",
        "widgets.toggles[0].off_cmd",
        "widgets.toggles[0].watch_cmd",
        "widgets.stats[0].cmd",
        "widgets.stats[0].plugin.command",
        "widgets.cards[0].cmd",
        "widgets.cards[0].plugin.command",
    ]
    .into_iter()
    .collect::<HashSet<_>>();

    assert_eq!(slots, expected);
}

#[test]
fn imported_exec_collection_does_not_include_runtime_defaults() {
    let content = collect_imported_exec_content(&current_config_bytes(b""), &[])
        .expect("parse data-only config");

    assert!(content.commands.is_empty());
    assert!(content.files.is_empty());
}

#[test]
fn imported_exec_collection_inventories_neighbors_of_script_payloads() {
    let config = br#"
[theme]
base_css = "base.css"
"#;
    let bundle_files = vec![
        BundleFile {
            relative_path: PathBuf::from("scripts/demo-widget"),
            contents: b"#!/bin/sh\necho ok\n".to_vec(),
            mode: 0o755,
        },
        BundleFile {
            relative_path: PathBuf::from("assets/helper.dat"),
            contents: b"plain helper\n".to_vec(),
            mode: 0o644,
        },
    ];

    let content = collect_imported_exec_content(&current_config_bytes(config), &bundle_files)
        .expect("collect script payload");

    assert!(content.commands.is_empty());
    assert_eq!(content.files.len(), 2);
    assert_eq!(
        content.files[0].relative_path,
        PathBuf::from("scripts/demo-widget")
    );
    assert_eq!(
        content.files[1].relative_path,
        PathBuf::from("assets/helper.dat")
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
    let bundle_files = vec![
        BundleFile {
            relative_path: PathBuf::from("scripts/check.sh"),
            contents: b"#!/bin/sh\necho ok\n".to_vec(),
            mode: 0o755,
        },
        BundleFile {
            relative_path: PathBuf::from("assets/module.py"),
            contents: b"print('ok')\n".to_vec(),
            mode: 0o644,
        },
        BundleFile {
            relative_path: PathBuf::from("base.css"),
            contents: Vec::new(),
            mode: 0o644,
        },
    ];

    let content = collect_imported_exec_content(&current_config_bytes(config), &bundle_files)
        .expect("collect trusted exec");

    assert_eq!(content.commands.len(), 1);
    assert_eq!(content.files.len(), 3);
    assert!(content
        .files
        .iter()
        .any(|file| file.relative_path == Path::new("assets/module.py")));
}

#[test]
fn imported_exec_collection_inventories_plain_payloads_for_each_command_form() {
    let cases = [
        ("sh assets/payload.dat", "assets/payload.dat"),
        ("BASH_ENV=assets/startup bash -c true", "assets/startup"),
        ("LD_PRELOAD=assets/module.so true", "assets/module.so"),
        (
            "PYTHONPATH=assets python3 -c 'import module'",
            "assets/module.py",
        ),
    ];

    for (command, payload_path) in cases {
        let config = format!("[[widgets.stats]]\nlabel = \"Probe\"\ncmd = {command:?}\n");
        let files = [BundleFile {
            relative_path: PathBuf::from(payload_path),
            contents: b"plain file payload\n".to_vec(),
            mode: 0o644,
        }];

        let content =
            collect_imported_exec_content(&current_config_bytes(config.as_bytes()), &files)
                .expect("collect command-backed plain payload");

        assert_eq!(content.commands.len(), 1);
        assert_eq!(content.files.len(), 1);
        assert_eq!(content.files[0].relative_path, PathBuf::from(payload_path));
    }
}

#[test]
fn imported_exec_collection_ignores_plain_assets_without_commands() {
    let files = [BundleFile {
        relative_path: PathBuf::from("assets/readme.txt"),
        contents: b"data only\n".to_vec(),
        mode: 0o644,
    }];

    let content = collect_imported_exec_content(&current_config_bytes(b""), &files)
        .expect("collect data-only bundle");

    assert!(content.commands.is_empty());
    assert!(content.files.is_empty());
}
