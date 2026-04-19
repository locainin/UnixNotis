use std::fs;

use unixnotis_core::Config;

use super::super::collect_css_check_inputs_from;
use super::helpers::TempDirGuard;

#[test]
fn warns_when_css_asset_ref_leaves_config_root() {
    let root = TempDirGuard::new("outside-css-asset");
    let config_dir = root.path().join("xdg").join("unixnotis");
    fs::create_dir_all(&config_dir).expect("create config dir");

    root.write(
        "xdg/unixnotis/base.css",
        ".unixnotis-panel { background-image: url(\"../outside.png\"); }",
    );
    root.write(
        "xdg/unixnotis/config.toml",
        "[theme]\nbase_css = \"base.css\"\n",
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
        .any(|line| line.contains("css asset reference(s) leave")));
    assert!(inputs
        .diagnostics
        .iter()
        .any(|warning| warning.message.contains("css asset reference leaves")));
}

#[test]
fn warns_when_css_asset_ref_uses_remote_url() {
    let root = TempDirGuard::new("remote-css-asset");
    let config_dir = root.path().join("xdg").join("unixnotis");
    fs::create_dir_all(&config_dir).expect("create config dir");

    root.write(
        "xdg/unixnotis/base.css",
        ".unixnotis-panel { background-image: url(\"https://example.com/panel.png\"); }",
    );
    root.write(
        "xdg/unixnotis/config.toml",
        "[theme]\nbase_css = \"base.css\"\n",
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
        .any(|line| line.contains("use remote URLs")));
    assert!(inputs
        .diagnostics
        .iter()
        .any(|warning| warning.message.contains("uses a remote URL")));
}
