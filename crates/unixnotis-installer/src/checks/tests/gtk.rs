use unixnotis_core::gtk_css_features_from_version_string;

use std::env;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use super::{gtk4_css_features_check, gtk4_layer_shell_check};
use crate::checks::{CheckItem, CheckState};

struct EnvGuard {
    key: &'static str,
    old: Option<String>,
}

impl EnvGuard {
    fn set(key: &'static str, value: impl AsRef<str>) -> Self {
        let old = env::var(key).ok();
        env::set_var(key, value.as_ref());
        Self { key, old }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        match &self.old {
            // pkg-config lookup goes through PATH, so test changes stay scoped
            Some(value) => env::set_var(self.key, value),
            None => env::remove_var(self.key),
        }
    }
}

#[test]
fn gtk_css_feature_parser_handles_major_and_minor_checks() {
    // GTK 4.16 is the first modern CSS feature level needed by the shipped theme path
    assert!(
        gtk_css_features_from_version_string("4.16.2")
            .expect("version")
            .custom_properties
    );
    assert!(
        gtk_css_features_from_version_string("4.18")
            .expect("version")
            .custom_properties
    );

    // Older GTK4 builds still work with legacy CSS but should not claim var() support
    assert!(
        !gtk_css_features_from_version_string("4.14.9")
            .expect("version")
            .custom_properties
    );

    // Future major versions should not regress feature detection
    assert!(
        gtk_css_features_from_version_string("5.0.0")
            .expect("version")
            .custom_properties
    );
}

#[test]
fn gtk_css_features_check_warns_for_old_gtk_and_okays_modern_gtk() {
    let _lock = crate::tests::env::test_env_lock();
    let root = test_root("gtk-css-features");
    let fake_bin = root.join("bin");
    fs::create_dir_all(&fake_bin).expect("fake bin");
    write_fake_pkg_config(&fake_bin, "4.14.9", None);
    let _path = EnvGuard::set("PATH", fake_bin.to_string_lossy());
    let pkg = CheckItem::ok("pkg-config", "available");

    let old = gtk4_css_features_check(&pkg);

    assert_eq!(old.state, CheckState::Warn);
    assert!(old.detail.contains("legacy theming"));

    write_fake_pkg_config(&fake_bin, "4.22.4", None);
    let modern = gtk4_css_features_check(&pkg);

    // Modern GTK should advertise the CSS variable support used by shipped themes
    assert_eq!(modern.state, CheckState::Ok);
    assert!(modern.detail.contains("modern css variables"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn gtk_checks_distinguish_pkg_config_missing_from_package_missing() {
    let _lock = crate::tests::env::test_env_lock();
    let root = test_root("gtk-pkg-config-missing");
    let fake_bin = root.join("bin");
    fs::create_dir_all(&fake_bin).expect("fake bin");
    write_failing_pkg_config(&fake_bin);
    let _path = EnvGuard::set("PATH", fake_bin.to_string_lossy());
    let pkg_missing = CheckItem::fail("pkg-config", "not installed");

    let css = gtk4_css_features_check(&pkg_missing);
    let layer = gtk4_layer_shell_check(&pkg_missing);

    // CSS is optional feature detail, but gtk4-layer-shell is required for the UI
    assert_eq!(css.state, CheckState::Warn);
    assert!(css.detail.contains("pkg-config missing"));
    assert_eq!(layer.state, CheckState::Fail);
    assert!(layer.detail.contains("pkg-config missing"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn gtk_layer_shell_check_reports_found_version() {
    let _lock = crate::tests::env::test_env_lock();
    let root = test_root("gtk-layer-shell-found");
    let fake_bin = root.join("bin");
    fs::create_dir_all(&fake_bin).expect("fake bin");
    write_fake_pkg_config(&fake_bin, "4.22.4", Some("1.3.0"));
    let _path = EnvGuard::set("PATH", fake_bin.to_string_lossy());
    let pkg = CheckItem::ok("pkg-config", "available");

    let layer = gtk4_layer_shell_check(&pkg);

    assert_eq!(layer.state, CheckState::Ok);
    assert!(layer.detail.contains("1.3.0"));
    let _ = fs::remove_dir_all(root);
}

fn write_fake_pkg_config(
    fake_bin: &std::path::Path,
    gtk_version: &str,
    layer_version: Option<&str>,
) {
    let layer_case = layer_version.map_or_else(
        || "gtk4-layer-shell-0) exit 1 ;;".to_string(),
        |version| format!("gtk4-layer-shell-0) printf '{version}\\n'; exit 0 ;;"),
    );
    let script = format!(
        "#!/bin/sh\n\
         case \"$2\" in\n\
         gtk4) printf '{gtk_version}\\n'; exit 0 ;;\n\
         {layer_case}\n\
         *) exit 1 ;;\n\
         esac\n"
    );
    let path = fake_bin.join("pkg-config");
    fs::write(&path, script).expect("fake pkg-config");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("fake pkg-config mode");
}

fn write_failing_pkg_config(fake_bin: &std::path::Path) {
    let path = fake_bin.join("pkg-config");
    fs::write(&path, "#!/bin/sh\nexit 1\n").expect("fake failing pkg-config");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755))
        .expect("fake failing pkg-config mode");
}

fn test_root(name: &str) -> PathBuf {
    let root = env::temp_dir().join(format!("unixnotis-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    root
}
