//! Stock theme export behavior

use std::fs;
use std::os::unix::fs::symlink;

use unixnotis_core::{ThemeManifest, DEFAULT_BASE_CSS, THEME_API_VERSION};

use super::super::export::{default_export_directory_for_config, export_stock_theme};

fn test_root(name: &str) -> std::path::PathBuf {
    let serial = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time should follow the Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("unixnotis-{name}-{}-{serial}", std::process::id()))
}

#[test]
fn stock_export_creates_complete_versioned_editable_copies() {
    let root = test_root("theme-export");
    fs::create_dir_all(&root).expect("test root should be created");
    let destination = root.join("stock");

    export_stock_theme(&destination).expect("stock theme should be exported");

    assert_eq!(
        fs::read_to_string(destination.join("base.css")).expect("base CSS should be readable"),
        DEFAULT_BASE_CSS
    );
    for name in [
        "panel.css",
        "popup.css",
        "widgets.css",
        "media.css",
        "theme.toml",
    ] {
        assert!(
            destination.join(name).is_file(),
            "stock export should include {name}"
        );
    }
    let manifest = fs::read_to_string(destination.join("theme.toml"))
        .expect("theme manifest should be readable");
    let manifest =
        toml::from_str::<ThemeManifest>(&manifest).expect("theme manifest should be valid");
    assert_eq!(manifest.api_version, THEME_API_VERSION);
    fs::remove_dir_all(root).expect("test root should be removable");
}

#[test]
fn stock_export_refuses_an_existing_destination_without_changing_it() {
    let root = test_root("theme-export-collision");
    let destination = root.join("stock");
    fs::create_dir_all(&destination).expect("existing destination should be created");
    let sentinel = destination.join("personal.css");
    fs::write(&sentinel, "/* keep */").expect("sentinel should be written");

    export_stock_theme(&destination).expect_err("existing export directory must be rejected");

    assert_eq!(
        fs::read_to_string(&sentinel).expect("sentinel should remain readable"),
        "/* keep */"
    );
    assert!(
        !destination.join("base.css").exists(),
        "rejected export must not create theme files"
    );
    fs::remove_dir_all(root).expect("test root should be removable");
}

#[test]
fn stock_export_rejects_a_symlinked_destination_parent() {
    let root = test_root("theme-export-symlink");
    let outside = root.join("outside");
    let linked = root.join("linked");
    fs::create_dir_all(&outside).expect("outside directory should be created");
    symlink(&outside, &linked).expect("linked parent should be created");

    export_stock_theme(&linked.join("stock"))
        .expect_err("stock export must not traverse a symbolic link");

    assert!(
        !outside.join("stock").exists(),
        "symlink rejection must not create files outside the selected tree"
    );
    fs::remove_dir_all(root).expect("test root should be removable");
}

#[test]
fn default_stock_export_directory_is_sibling_of_active_config() {
    let config = std::path::Path::new("profile/unixnotis/config.toml");

    assert_eq!(
        default_export_directory_for_config(config).expect("default export path should resolve"),
        std::path::Path::new("profile/unixnotis/stock-theme-v2")
    );
}
