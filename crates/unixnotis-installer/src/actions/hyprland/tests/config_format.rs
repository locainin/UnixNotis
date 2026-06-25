use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::super::super::HYPR_IMPORT_VARS;
use super::super::block::render_hyprland_bootstrap_block;
use super::super::detect::{
    has_import_command_with_vars, has_legacy_dbus_update, has_startup_command,
    hyprland_startup_line,
};
use super::super::paths::{
    existing_hyprland_config_targets_in, hyprland_config_target_in, HyprlandConfigSyntax,
};
use crate::service_manager::ServiceManager;

#[test]
fn hyprland_config_target_prefers_lua_when_lua_exists() {
    let root = temp_config_root("hyprland-lua");
    let hypr_dir = root.join("hypr");
    fs::create_dir_all(&hypr_dir).expect("hypr config directory should be created");
    fs::write(hypr_dir.join("hyprland.conf"), "").expect("legacy config should be written");
    fs::write(hypr_dir.join("hyprland.lua"), "").expect("lua config should be written");

    let target = hyprland_config_target_in(&root);

    assert_eq!(target.path, hypr_dir.join("hyprland.lua"));
    assert_eq!(target.syntax, HyprlandConfigSyntax::Lua);
    fs::remove_dir_all(root).expect("temp config root should be removed");
}

#[test]
fn hyprland_config_target_uses_legacy_conf_when_it_is_the_only_config() {
    let root = temp_config_root("hyprland-conf");
    let hypr_dir = root.join("hypr");
    fs::create_dir_all(&hypr_dir).expect("hypr config directory should be created");
    fs::write(hypr_dir.join("hyprland.conf"), "").expect("legacy config should be written");

    let target = hyprland_config_target_in(&root);

    assert_eq!(target.path, hypr_dir.join("hyprland.conf"));
    assert_eq!(target.syntax, HyprlandConfigSyntax::Hyprlang);
    fs::remove_dir_all(root).expect("temp config root should be removed");
}

#[test]
fn hyprland_config_target_defaults_to_lua_when_no_config_exists() {
    let root = temp_config_root("hyprland-missing");
    fs::create_dir_all(root.join("hypr")).expect("hypr config directory should be created");

    let target = hyprland_config_target_in(&root);

    assert_eq!(target.path, root.join("hypr").join("hyprland.lua"));
    assert_eq!(target.syntax, HyprlandConfigSyntax::Lua);
    fs::remove_dir_all(root).expect("temp config root should be removed");
}

#[test]
fn existing_hyprland_config_targets_include_both_migration_formats() {
    let root = temp_config_root("hyprland-existing");
    let hypr_dir = root.join("hypr");
    fs::create_dir_all(&hypr_dir).expect("hypr config directory should be created");
    fs::write(hypr_dir.join("hyprland.conf"), "").expect("legacy config should be written");
    fs::write(hypr_dir.join("hyprland.lua"), "").expect("lua config should be written");

    let targets = existing_hyprland_config_targets_in(&root);

    assert_eq!(targets.len(), 2);
    assert_eq!(targets[0].path, hypr_dir.join("hyprland.lua"));
    assert_eq!(targets[0].syntax, HyprlandConfigSyntax::Lua);
    assert_eq!(targets[1].path, hypr_dir.join("hyprland.conf"));
    assert_eq!(targets[1].syntax, HyprlandConfigSyntax::Hyprlang);
    fs::remove_dir_all(root).expect("temp config root should be removed");
}

#[test]
fn rendered_lua_bootstrap_is_detected_as_complete() {
    let manager = ServiceManager::systemd_user(PathBuf::from("/tmp/systemd/user"));
    let commands = manager.hyprland_startup_commands(&HYPR_IMPORT_VARS);
    let lines = commands
        .iter()
        .map(|command| hyprland_startup_line(HyprlandConfigSyntax::Lua, command))
        .collect::<Vec<_>>();
    let block = render_hyprland_bootstrap_block(HyprlandConfigSyntax::Lua, &lines);

    assert!(block.contains("hl.on(\"hyprland.start\", function()"));
    assert!(block.contains("hl.exec_cmd(\"systemctl --user import-environment"));
    assert!(block.contains("end)"));
    assert!(has_legacy_dbus_update(
        "exec-once = dbus-update-activation-environment --systemd --all"
    ));
    assert!(commands
        .iter()
        .all(|command| has_startup_command(&block, command)
            || has_import_command_with_vars(&block, &HYPR_IMPORT_VARS)));
}

#[test]
fn commented_lua_bootstrap_commands_are_ignored() {
    let contents =
        "-- hl.exec_cmd(\"systemctl --user --no-block restart unixnotis-daemon.service\")\n";

    assert!(!has_startup_command(
        contents,
        "systemctl --user --no-block restart unixnotis-daemon.service"
    ));
}

fn temp_config_root(label: &str) -> PathBuf {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after Unix epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "unixnotis-installer-{label}-{}-{now}",
        std::process::id()
    ));
    recreate_empty_dir(&path);
    path
}

fn recreate_empty_dir(path: &Path) {
    if path.exists() {
        fs::remove_dir_all(path).expect("stale temp config root should be removed");
    }
    fs::create_dir_all(path).expect("temp config root should be created");
}
