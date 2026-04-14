use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::{env, fs};

use super::{build_panel_overrides, build_popup_overrides, build_widgets_overrides};
use unixnotis_core::{gtk_css_features_for_version, ThemeConfig};

#[test]
fn base_overrides_clamp_alpha_values() {
    // Confirms alpha values are clamped into the CSS-friendly [0.0, 1.0] range
    let theme = ThemeConfig {
        surface_alpha: 1.5,
        surface_strong_alpha: -0.25,
        shadow_soft_alpha: 2.0,
        shadow_strong_alpha: -1.0,
        ..ThemeConfig::default()
    };

    let overrides =
        super::build_base_overrides_for_runtime(&theme, gtk_css_features_for_version(4, 15));
    let surface = format!(
        "alpha(@unixnotis-surface-base, {})",
        1.0_f32.clamp(0.0, 1.0)
    );
    let surface_strong = format!(
        "alpha(@unixnotis-surface-strong-base, {})",
        (-0.25_f32).clamp(0.0, 1.0)
    );
    let shadow_soft = format!("alpha(#000000, {})", 2.0_f32.clamp(0.0, 1.0));
    let shadow_strong = format!("alpha(#000000, {})", (-1.0_f32).clamp(0.0, 1.0));

    assert!(overrides.contains(&surface));
    assert!(overrides.contains(&surface_strong));
    assert!(overrides.contains(&shadow_soft));
    assert!(overrides.contains(&shadow_strong));
}

#[test]
fn base_overrides_can_emit_modern_custom_properties() {
    // Modern tokens are additive so the old color path stays intact
    let theme = ThemeConfig {
        border_width: 3,
        card_radius: 18,
        card_alpha: 0.52,
        ..ThemeConfig::default()
    };

    let overrides =
        super::build_base_overrides_for_runtime(&theme, gtk_css_features_for_version(4, 16));
    assert!(overrides.contains(":root {"));
    assert!(overrides.contains("--unixnotis-border-width: 3px;"));
    assert!(overrides.contains("--unixnotis-card-radius: 18px;"));
    assert!(overrides.contains("--unixnotis-card-alpha: 0.52;"));
    assert!(overrides.contains("--unixnotis-panel-card-padding-y: 10px;"));
    assert!(overrides.contains("--unixnotis-popup-reveal-duration: 200ms;"));
    assert!(overrides.contains("--unixnotis-accent-color: @unixnotis-accent;"));
    assert!(overrides.contains("@define-color unixnotis-surface alpha(@unixnotis-surface-base,"));
}

#[test]
fn panel_overrides_use_theme_values() {
    // Ensures panel overrides reflect the configured card styling values
    let theme = ThemeConfig {
        border_width: 3,
        card_radius: 12,
        card_alpha: 0.42,
        ..ThemeConfig::default()
    };

    let overrides = build_panel_overrides(&theme);
    assert!(overrides.contains("border-width: 3px;"));
    assert!(overrides.contains("border-radius: 12px;"));
    assert!(overrides.contains("background: @unixnotis-card;"));
}

#[test]
fn widgets_overrides_use_theme_values() {
    // Ensures widget card styling uses the configured theme values
    let theme = ThemeConfig {
        border_width: 2,
        card_radius: 8,
        card_alpha: 0.77,
        ..ThemeConfig::default()
    };

    let overrides = build_widgets_overrides(&theme);
    assert!(overrides.contains("border-width: 2px;"));
    assert!(overrides.contains("border-radius: 8px;"));
    assert!(overrides.contains("background: @unixnotis-card;"));
}

#[test]
fn popup_overrides_use_theme_values() {
    // Ensures popup card styling uses the configured theme values
    let theme = ThemeConfig {
        border_width: 5,
        card_radius: 24,
        ..ThemeConfig::default()
    };

    let overrides = build_popup_overrides(&theme);
    assert!(overrides.contains("border-width: 5px;"));
    assert!(overrides.contains("border-radius: 24px;"));
}

#[test]
fn generated_override_css_loads_without_parse_errors_for_legacy_runtime() {
    // Old GTK should still accept the generated fallback path cleanly
    let theme = ThemeConfig::default();
    let css = format!(
        "{}\n{}\n{}\n{}",
        super::build_base_overrides_for_runtime(&theme, gtk_css_features_for_version(4, 15)),
        build_panel_overrides(&theme),
        build_widgets_overrides(&theme),
        build_popup_overrides(&theme),
    );

    assert_css_validates_in_gtk(&css);
}

#[test]
fn generated_override_css_loads_without_parse_errors_for_modern_runtime() {
    // New GTK should also accept the additive custom property path cleanly
    let theme = ThemeConfig {
        border_width: 2,
        card_radius: 18,
        card_alpha: 0.77,
        ..ThemeConfig::default()
    };
    let css = format!(
        "{}\n{}\n{}\n{}",
        super::build_base_overrides_for_runtime(&theme, gtk_css_features_for_version(4, 16)),
        build_panel_overrides(&theme),
        build_widgets_overrides(&theme),
        build_popup_overrides(&theme),
    );

    assert_css_validates_in_gtk(&css);
}

fn assert_css_validates_in_gtk(css: &str) {
    // GTK parses this in a helper because the test harness does not own the main thread
    let helper = css_provider_validator_binary();
    let mut child = Command::new(helper)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|error| panic!("failed to spawn css validator {:?}: {error}", helper));

    child
        .stdin
        .take()
        .expect("css validator stdin")
        .write_all(css.as_bytes())
        .expect("write css to validator");

    let output = child.wait_with_output().expect("wait for css validator");
    assert!(
        output.status.success(),
        "gtk css validation failed\n{}\n{css}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn css_provider_validator_binary() -> &'static std::path::PathBuf {
    static BIN: OnceLock<std::path::PathBuf> = OnceLock::new();

    BIN.get_or_init(|| {
        if let Some(path) = option_env!("CARGO_BIN_EXE_css_provider_validate") {
            return path.into();
        }

        let current_exe = env::current_exe().expect("current test binary path");
        let target_dir = current_exe
            .parent()
            .and_then(|path| path.parent())
            .expect("target debug dir");
        // Unit tests do not always prebuild sibling binaries, so build the validator on demand
        let candidate =
            target_dir.join(format!("css_provider_validate{}", env::consts::EXE_SUFFIX));
        if fs::metadata(&candidate).is_err() {
            build_css_provider_validator();
        }
        assert!(
            fs::metadata(&candidate).is_ok(),
            "css validator binary is missing at {:?}",
            candidate
        );
        candidate
    })
}

fn build_css_provider_validator() {
    // Build the helper lazily so normal cargo test output stays clean and local
    let cargo = env::var("CARGO").unwrap_or_else(|_| String::from("cargo"));
    let output = Command::new(cargo)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args(["build", "--bin", "css_provider_validate"])
        .output()
        .expect("run cargo build for css validator");

    assert!(
        output.status.success(),
        "failed to build css validator\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
