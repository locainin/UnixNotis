use unixnotis_core::CommandSpec;

use super::super::{
    collect_command_references_from_config, collect_host_specific_command_paths,
    collect_outside_command_paths, rewrite_host_specific_command_paths,
};
use super::support::{parse_current_config, temp_root, validate_command_paths_in_config_bytes};

#[test]
fn collects_widget_command_references() {
    let config = parse_current_config(
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

    let parsed = parse_current_config(config).expect("parse config");
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
        .contains("resolves outside the UnixNotis config directory"));
}

#[test]
fn validation_error_does_not_echo_rejected_command_text() {
    let config_dir = temp_root("redacted-command");
    let config = b"[theme]\nbase_css = \"base.css\"\n[[widgets.toggles]]\nlabel = \"Probe\"\nicon = \"applications-system-symbolic\"\nwatch_cmd = \"../outside-watch --token private-value\"\n";

    let error =
        validate_command_paths_in_config_bytes(&config_dir, config, "preset import blocked")
            .expect_err("reject escaped command without exposing its arguments");
    let message = error.to_string();

    assert!(message.contains("widgets.toggles[0].watch_cmd"));
    assert!(message.contains("resolves outside the UnixNotis config directory"));
    assert!(!message.contains("outside-watch"));
    assert!(!message.contains("private-value"));
}

#[test]
fn validation_rejects_absolute_command_path_with_equals_that_leaves_root() {
    let config_dir = temp_root("equals-command-path");
    let config = b"[theme]\nbase_css = \"base.css\"\n[[widgets.toggles]]\nlabel = \"Probe\"\nicon = \"applications-system-symbolic\"\ntoggle_cmd = \"/tmp/tool=evil --run\"\n";

    let error =
        validate_command_paths_in_config_bytes(&config_dir, config, "preset import blocked")
            .expect_err("path-like command containing equals should not parse as env");

    assert!(error
        .to_string()
        .contains("resolves outside the UnixNotis config directory"));
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

    let parsed = parse_current_config(&config).expect("parse config");
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

    let mut parsed = parse_current_config(&config).expect("parse config");
    let rewritten = rewrite_host_specific_command_paths(&config_dir, &mut parsed);

    assert_eq!(rewritten.len(), 1);
    assert_eq!(
        parsed.widgets.stats[0]
            .plugin
            .as_ref()
            .expect("plugin")
            .command,
        CommandSpec::direct("scripts/unixnotis-thermal-stat", ["--json"])
    );
}

#[test]
fn rewrite_host_specific_command_inside_env_wrapper_preserves_assignments() {
    let config_dir = temp_root("rewrite-env-wrapper");
    let script_path = config_dir.join("scripts/probe tool");
    let config = format!(
        "[theme]\nbase_css = \"base.css\"\n[[widgets.stats]]\nlabel = \"Probe\"\ncmd = {:?}\n",
        format!("env MODE='two words' '{}' --json", script_path.display())
    );
    let mut parsed = parse_current_config(&config).expect("parse config");

    let rewritten = rewrite_host_specific_command_paths(&config_dir, &mut parsed);

    assert_eq!(rewritten.len(), 1);
    assert_eq!(
        parsed.widgets.stats[0].cmd,
        Some(CommandSpec::direct(
            "env",
            ["MODE=two words", "scripts/probe tool", "--json"]
        ))
    );
}

#[test]
fn rewrite_host_specific_command_inside_env_wrapper_preserves_options() {
    let config_dir = temp_root("rewrite-env-options");
    let script_path = config_dir.join("scripts/probe");
    let config = format!(
        "[theme]\nbase_css = \"base.css\"\n[[widgets.stats]]\nlabel = \"Probe\"\ncmd = {:?}\n",
        format!("env -u HOME MODE=safe {} --json", script_path.display())
    );
    let mut parsed = parse_current_config(&config).expect("parse config");

    let rewritten = rewrite_host_specific_command_paths(&config_dir, &mut parsed);

    assert_eq!(rewritten.len(), 1);
    assert_eq!(
        parsed.widgets.stats[0].cmd,
        Some(CommandSpec::direct(
            "env",
            ["-u", "HOME", "MODE=safe", "scripts/probe", "--json"]
        ))
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

    let mut parsed = parse_current_config(&config).expect("parse config");
    let rewritten = rewrite_host_specific_command_paths(&config_dir, &mut parsed);

    assert_eq!(rewritten.len(), 1);
    assert_eq!(
        parsed.widgets.toggles[0].toggle_cmd,
        Some(CommandSpec::direct(
            "scripts/unixnotis-toggle-action",
            ["--json"]
        ))
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

    let parsed = parse_current_config(&config).expect("parse config");
    let leaks = collect_host_specific_command_paths(&config_dir, &parsed);

    assert_eq!(leaks.len(), 1);
    assert_eq!(leaks[0].slot, "widgets.toggles[0].toggle_cmd");
    assert_eq!(
        leaks[0].command,
        CommandSpec::direct(script_path, [] as [&str; 0])
    );
}
