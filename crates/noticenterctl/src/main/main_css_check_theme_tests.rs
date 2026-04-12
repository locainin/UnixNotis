use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use unixnotis_core::Config;

use super::collect_css_check_inputs_from;

static TEST_TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TempDirGuard {
    path: PathBuf,
}

impl TempDirGuard {
    fn new(name: &str) -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock moved backwards")
            .as_nanos();
        let serial = TEST_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("unixnotis-css-check-{}-{}-{}", name, stamp, serial));
        fs::create_dir_all(&path).expect("create temp dir");
        Self { path }
    }

    fn write(&self, relative_path: &str, contents: &str) {
        let path = self.path.join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent dirs");
        }
        fs::write(path, contents).expect("write file");
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

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
        .info_lines
        .iter()
        .any(|line| line.contains("live outside $XDG_CONFIG_HOME/unixnotis")));
    assert!(inputs.warnings.iter().any(|warning| warning
        .message
        .contains("point outside $XDG_CONFIG_HOME/unixnotis")));
    assert!(inputs
        .info_lines
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

    assert!(inputs.warnings.iter().any(|warning| warning
        .message
        .contains("[theme].base_css, [theme].panel_css")));
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

    assert!(inputs.warnings.iter().any(|warning| warning
        .message
        .contains("configured panel css target is missing")));
}

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
        .info_lines
        .iter()
        .any(|line| line.contains("configured command path(s) point outside")));
    assert!(inputs.warnings.iter().any(|warning| warning
        .message
        .contains("shared presets should keep explicit command paths inside")));
}

#[test]
fn warns_when_css_asset_ref_points_outside_config_root() {
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
        .info_lines
        .iter()
        .any(|line| line.contains("css asset reference(s) point outside")));
    assert!(inputs.warnings.iter().any(|warning| warning
        .message
        .contains("css asset reference points outside")));
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
        .info_lines
        .iter()
        .any(|line| line.contains("host-local config-root paths")));
    assert!(inputs.warnings.iter().any(|warning| warning
        .message
        .contains("export should rewrite it before sharing")));
}
