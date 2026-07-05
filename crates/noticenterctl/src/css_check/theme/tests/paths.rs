use std::fs;

use unixnotis_core::Config;

use super::super::collect_css_check_inputs_from;
use super::helpers::TempDirGuard;

#[test]
fn includes_active_targets_outside_config_root_and_skips_unused_css() {
    let root = TempDirGuard::new("external");
    let config_dir = root.path().join("xdg").join("unixnotis");
    fs::create_dir_all(&config_dir).expect("create config dir");

    let external_panel = root.path().join("shared").join("panel.theme");
    fs::create_dir_all(external_panel.parent().expect("panel parent"))
        .expect("create panel parent");

    fs::write(
        config_dir.join("base.css"),
        ".unixnotis-panel { color: red; }",
    )
    .expect("write base.css");
    fs::write(
        config_dir.join("popup.css"),
        ".unixnotis-popup { color: red; }",
    )
    .expect("write popup.css");
    fs::write(
        config_dir.join("widgets.css"),
        ".unixnotis-toggle { color: red; }",
    )
    .expect("write widgets.css");
    fs::write(
        config_dir.join("media.css"),
        ".unixnotis-media-card { color: red; }",
    )
    .expect("write media.css");
    fs::write(&external_panel, ".unixnotis-panel { color: blue; }").expect("write external panel");
    fs::write(config_dir.join("unused.css"), ".unused { color: red; }").expect("write unused.css");
    root.write(
        "xdg/unixnotis/config.toml",
        &format!("[theme]\npanel_css = \"{}\"\n", external_panel.display()),
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

    assert_eq!(inputs.files.len(), 5);
    assert!(inputs.files.iter().any(|path| path == &external_panel));
    assert!(inputs
        .notes
        .iter()
        .any(|line| line.contains("live outside $XDG_CONFIG_HOME/unixnotis")));
    assert!(inputs.diagnostics.iter().any(|warning| warning
        .message
        .contains("point outside $XDG_CONFIG_HOME/unixnotis")));
    assert!(inputs
        .notes
        .iter()
        .any(|line| line.contains("extra css file(s)")));
}

#[test]
fn warns_when_theme_slots_share_one_file() {
    let root = TempDirGuard::new("duplicate");
    let config_dir = root.path().join("xdg").join("unixnotis");
    fs::create_dir_all(&config_dir).expect("create config dir");

    root.write("xdg/unixnotis/base.css", ".unixnotis-panel { color: red; }");
    root.write(
        "xdg/unixnotis/popup.css",
        ".unixnotis-popup { color: red; }",
    );
    root.write(
        "xdg/unixnotis/widgets.css",
        ".unixnotis-toggle { color: red; }",
    );
    root.write(
        "xdg/unixnotis/media.css",
        ".unixnotis-media-card { color: red; }",
    );
    root.write(
        "xdg/unixnotis/config.toml",
        "[theme]\nbase_css = \"shared.css\"\npanel_css = \"shared.css\"\npopup_css = \"popup.css\"\nwidgets_css = \"widgets.css\"\nmedia_css = \"media.css\"\n",
    );
    root.write(
        "xdg/unixnotis/shared.css",
        ".unixnotis-panel { color: blue; }",
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

    assert!(inputs.diagnostics.iter().any(|warning| warning
        .message
        .contains("[theme].base_css, [theme].panel_css")));
}

#[cfg(unix)]
#[test]
fn dedupes_theme_slots_that_resolve_to_the_same_real_file() {
    use std::os::unix::fs::symlink;

    let root = TempDirGuard::new("canonical-dedupe");
    let config_dir = root.path().join("xdg").join("unixnotis");
    fs::create_dir_all(&config_dir).expect("create config dir");

    root.write("xdg/unixnotis/base.css", ".unixnotis-panel { color: red; }");
    root.write(
        "xdg/unixnotis/widgets.css",
        ".unixnotis-toggle { color: red; }",
    );
    root.write(
        "xdg/unixnotis/media.css",
        ".unixnotis-media-card { color: red; }",
    );
    root.write(
        "xdg/unixnotis/shared/popup.css",
        ".unixnotis-popup { color: red; }",
    );
    fs::create_dir_all(config_dir.join("aliases")).expect("create alias dir");
    symlink(
        "../shared/popup.css",
        config_dir.join("aliases/popup-link.css"),
    )
    .expect("create symlink");
    root.write(
        "xdg/unixnotis/config.toml",
        "[theme]\npanel_css = \"shared/popup.css\"\npopup_css = \"aliases/popup-link.css\"\n",
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

    let popup_paths = inputs
        .files
        .iter()
        .filter(|path| path.ends_with("popup.css") || path.ends_with("popup-link.css"))
        .collect::<Vec<_>>();
    assert_eq!(popup_paths.len(), 1);
    assert_eq!(inputs.files.len(), 4);
}

#[test]
fn warns_when_configured_theme_target_is_missing() {
    let root = TempDirGuard::new("missing");
    let config_dir = root.path().join("xdg").join("unixnotis");
    fs::create_dir_all(&config_dir).expect("create config dir");

    root.write("xdg/unixnotis/base.css", ".unixnotis-panel { color: red; }");
    root.write(
        "xdg/unixnotis/popup.css",
        ".unixnotis-popup { color: red; }",
    );
    root.write(
        "xdg/unixnotis/widgets.css",
        ".unixnotis-toggle { color: red; }",
    );
    root.write(
        "xdg/unixnotis/media.css",
        ".unixnotis-media-card { color: red; }",
    );
    root.write(
        "xdg/unixnotis/config.toml",
        "[theme]\npanel_css = \"missing/panel.css\"\n",
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

    assert!(inputs.diagnostics.iter().any(|warning| warning
        .message
        .contains("configured panel css target is missing")));
}
