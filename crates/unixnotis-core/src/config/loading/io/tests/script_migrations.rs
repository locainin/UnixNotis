use std::fs;

use super::super::script_migrations::is_legacy_stock_script;
use super::support::test_root;

const LEGACY_BLUE_LIGHT_ON: &[u8] =
    include_bytes!("../../../../../assets/scripts/legacy/unixnotis-blue-light-on-v1");

#[test]
fn exact_legacy_stock_script_is_recognized() {
    let root = test_root("legacy-stock-script");
    let path = root.join("unixnotis-blue-light-on");
    fs::create_dir_all(&root).expect("legacy helper directory");
    fs::write(&path, LEGACY_BLUE_LIGHT_ON).expect("write legacy helper");

    assert!(is_legacy_stock_script(
        &path,
        "scripts/unixnotis-blue-light-on"
    ));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn edited_legacy_script_is_not_recognized_as_stock() {
    let root = test_root("edited-legacy-script");
    let path = root.join("unixnotis-blue-light-on");
    fs::create_dir_all(&root).expect("edited helper directory");
    let mut edited = LEGACY_BLUE_LIGHT_ON.to_vec();
    edited.extend_from_slice(b"\n# local setting\n");
    fs::write(&path, edited).expect("write edited helper");

    assert!(!is_legacy_stock_script(
        &path,
        "scripts/unixnotis-blue-light-on"
    ));
    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn legacy_bytes_reached_through_a_same_length_symlink_are_not_stock() {
    use std::os::unix::fs::symlink;

    let root = test_root("linked-legacy-script");
    fs::create_dir_all(&root).expect("linked helper directory");
    let fixture = root.join("fixture");
    let link = root.join("unixnotis-blue-light-on");
    fs::write(&fixture, LEGACY_BLUE_LIGHT_ON).expect("write linked helper target");
    let link_target = format!("{}fixture", "./".repeat(197));
    assert_eq!(link_target.len(), LEGACY_BLUE_LIGHT_ON.len());
    symlink(link_target, &link).expect("link legacy helper");

    assert!(!is_legacy_stock_script(
        &link,
        "scripts/unixnotis-blue-light-on"
    ));
    let _ = fs::remove_dir_all(root);
}
