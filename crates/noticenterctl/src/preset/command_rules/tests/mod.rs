use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use unixnotis_core::Config;

use super::{
    collect_command_references_from_config, collect_host_specific_command_paths,
    collect_outside_command_paths, rewrite_host_specific_command_paths,
    validate_command_paths_in_config_bytes,
};

static TEST_TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

fn temp_root(name: &str) -> PathBuf {
    // Unique paths keep lexical path checks stable under parallel test runs
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock moved backwards")
        .as_nanos();
    let serial = TEST_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "unixnotis-preset-command-rules-{}-{}-{}",
        name, stamp, serial
    ))
}

#[test]
fn collects_widget_command_references() {
    let config: Config = toml::from_str(
        "\
[theme]\nbase_css = \"base.css\"\n\
[[widgets.toggles]]\nlabel = \"Action\"\nicon = \"applications-system-symbolic\"\ntoggle_cmd = \"scripts/action.sh\"\n\
[[widgets.stats]]\nlabel = \"Probe\"\n\
[widgets.stats.plugin]\napi_version = 1\ncommand = \"scripts/fetch.sh\"\n",
    )
    .expect("parse config");

    let commands = collect_command_references_from_config(&config);

    assert!(commands
        .iter()
        .any(|command| command.slot == "widgets.stats[0].plugin.command"));
    assert!(commands
        .iter()
        .any(|command| command.slot == "widgets.toggles[0].toggle_cmd"));
}

#[test]
fn outside_command_paths_include_absolute_plugin_command() {
    let config_dir = temp_root("outside-plugin");
    let config = "\
[theme]\nbase_css = \"base.css\"\n\
[[widgets.stats]]\nlabel = \"Probe\"\n\
[widgets.stats.plugin]\napi_version = 1\ncommand = \"/tmp/outside-plugin\"\n";

    let parsed = toml::from_str(config).expect("parse config");
    let outside = collect_outside_command_paths(&config_dir, &parsed);

    assert_eq!(outside.len(), 1);
    assert_eq!(outside[0].slot, "widgets.stats[0].plugin.command");
}

#[test]
fn validation_rejects_relative_command_path_that_leaves_root() {
    let config_dir = temp_root("relative-command");
    let config = b"[theme]\nbase_css = \"base.css\"\n[[widgets.toggles]]\nlabel = \"Probe\"\nicon = \"applications-system-symbolic\"\nwatch_cmd = \"../outside-watch\"\n";

    let error =
        validate_command_paths_in_config_bytes(&config_dir, config, "preset import blocked")
            .expect_err("reject relative command escape");

    assert!(error
        .to_string()
        .contains("points outside the UnixNotis config directory"));
}

#[test]
fn host_specific_command_paths_include_absolute_path_inside_root() {
    let config_dir = temp_root("inside-root-host-specific");
    let script_path = config_dir.join("scripts/unixnotis-thermal-stat");
    let config = format!(
        "\
[theme]\nbase_css = \"base.css\"\n\
[[widgets.stats]]\nlabel = \"Probe\"\n\
[widgets.stats.plugin]\napi_version = 1\ncommand = {:?}\n",
        script_path.display().to_string()
    );

    let parsed = toml::from_str(&config).expect("parse config");
    let leaks = collect_host_specific_command_paths(&config_dir, &parsed);

    assert_eq!(leaks.len(), 1);
    assert_eq!(leaks[0].slot, "widgets.stats[0].plugin.command");
}

#[test]
fn rewrite_host_specific_command_paths_makes_commands_config_relative() {
    let config_dir = temp_root("rewrite");
    let script_path = config_dir.join("scripts/unixnotis-thermal-stat");
    let config = format!(
        "\
[theme]\nbase_css = \"base.css\"\n\
[[widgets.stats]]\nlabel = \"Probe\"\n\
[widgets.stats.plugin]\napi_version = 1\ncommand = {:?}\n",
        format!("{} --json", script_path.display())
    );

    let mut parsed: Config = toml::from_str(&config).expect("parse config");
    let rewritten = rewrite_host_specific_command_paths(&config_dir, &mut parsed);

    assert_eq!(rewritten.len(), 1);
    assert_eq!(
        parsed.widgets.stats[0]
            .plugin
            .as_ref()
            .expect("plugin")
            .command,
        "scripts/unixnotis-thermal-stat --json"
    );
}

#[test]
fn rewrite_host_specific_toggle_command_paths_makes_commands_config_relative() {
    let config_dir = temp_root("rewrite-toggle-command");
    let script_path = config_dir.join("scripts/unixnotis-toggle-action");
    let config = format!(
        "\
[theme]\nbase_css = \"base.css\"\n\
[[widgets.toggles]]\nlabel = \"Probe\"\nicon = \"applications-system-symbolic\"\ntoggle_cmd = {:?}\n",
        format!("{} --json", script_path.display())
    );

    let mut parsed: Config = toml::from_str(&config).expect("parse config");
    let rewritten = rewrite_host_specific_command_paths(&config_dir, &mut parsed);

    assert_eq!(rewritten.len(), 1);
    assert_eq!(
        parsed.widgets.toggles[0].toggle_cmd.as_deref(),
        Some("scripts/unixnotis-toggle-action --json")
    );
}

#[test]
fn host_specific_command_paths_include_toggle_command() {
    let config_dir = temp_root("toggle-command-host-specific");
    let script_path = config_dir.join("scripts/unixnotis-toggle-action");
    let config = format!(
        "\
[theme]\nbase_css = \"base.css\"\n\
[[widgets.toggles]]\nlabel = \"Probe\"\nicon = \"applications-system-symbolic\"\ntoggle_cmd = {:?}\n",
        script_path.display().to_string()
    );

    let parsed = toml::from_str(&config).expect("parse config");
    let leaks = collect_host_specific_command_paths(&config_dir, &parsed);

    assert_eq!(leaks.len(), 1);
    assert_eq!(leaks[0].slot, "widgets.toggles[0].toggle_cmd");
    assert_eq!(leaks[0].command, script_path.display().to_string());
}
