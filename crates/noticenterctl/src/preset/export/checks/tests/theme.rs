use std::path::Path;

use super::validate_theme_paths_stay_in_root;

#[test]
fn theme_paths_inside_config_root_are_accepted() {
    let root = Path::new("/tmp/unixnotis-config");
    let panel = root.join("panel.css");

    validate_theme_paths_stay_in_root(root, &[("panel_css", &panel)])
        .expect("contained theme path");
}

#[test]
fn theme_paths_outside_config_root_are_rejected() {
    let root = Path::new("/tmp/unixnotis-config");
    let outside = Path::new("/tmp/outside.css");

    let error = validate_theme_paths_stay_in_root(root, &[("panel_css", outside)])
        .expect_err("outside theme path");

    assert!(error.to_string().contains("panel_css"));
}
