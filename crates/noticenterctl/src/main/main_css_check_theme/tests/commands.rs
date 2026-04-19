use std::fs;

use unixnotis_core::Config;

use super::super::collect_css_check_inputs_from;
use super::helpers::TempDirGuard;

#[test]
fn warns_when_command_path_points_outside_config_root() {
    let root = TempDirGuard::new("outside-command");
    let config_dir = root.path().join("xdg").join("unixnotis");
    fs::create_dir_all(&config_dir).expect("create config dir");

    root.write("xdg/unixnotis/base.css", ".unixnotis-panel { color: red; }");
    root.write(
        "xdg/unixnotis/config.toml",
        "[theme]\nbase_css = \"base.css\"\n[[widgets.stats]]\nlabel = \"Probe\"\n[widgets.stats.plugin]\napi_version = 1\ncommand = \"/tmp/outside-plugin\"\n",
    );

    let config_path = config_dir.join("config.toml");
    let config = Config::load_from_path(&config_path).expect("load config");
    let inputs = collect_css_check_inputs_from(
        &config_dir,
        "$XDG_CONFIG_HOME/unixnotis",
        &config_path,
        &config,
    )
    .expect("inputs");

    assert!(inputs
        .notes
        .iter()
        .any(|line| line.contains("configured command path(s) point outside")));
    assert!(inputs.diagnostics.iter().any(|warning| warning
        .message
        .contains("shared presets should keep explicit command paths inside")));
}

#[test]
fn warns_when_command_path_is_host_specific_under_config_root() {
    let root = TempDirGuard::new("host-specific-command");
    let config_dir = root.path().join("xdg").join("unixnotis");
    fs::create_dir_all(config_dir.join("scripts")).expect("create config dir");
    let script_path = config_dir.join("scripts/unixnotis-thermal-stat");

    root.write("xdg/unixnotis/base.css", ".unixnotis-panel { color: red; }");
    fs::write(&script_path, "#!/bin/sh\necho 42\n").expect("write script");
    root.write(
        "xdg/unixnotis/config.toml",
        &format!(
            "[theme]\nbase_css = \"base.css\"\n[[widgets.stats]]\nlabel = \"Probe\"\n[widgets.stats.plugin]\napi_version = 1\ncommand = {:?}\n",
            script_path.display().to_string()
        ),
    );

    let config_path = config_dir.join("config.toml");
    let config = Config::load_from_path(&config_path).expect("load config");
    let inputs = collect_css_check_inputs_from(
        &config_dir,
        "$XDG_CONFIG_HOME/unixnotis",
        &config_path,
        &config,
    )
    .expect("inputs");

    assert!(inputs
        .notes
        .iter()
        .any(|line| line.contains("host-local config-root paths")));
    assert!(inputs.diagnostics.iter().any(|warning| warning
        .message
        .contains("export should rewrite it before sharing")));
}
