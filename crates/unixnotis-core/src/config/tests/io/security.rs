use std::fs;

use super::super::reserve_adjacent_temp;
use super::support::test_root;

#[cfg(unix)]
#[test]
fn temp_reservation_skips_planted_symlink_without_touching_its_target() {
    use std::io::Write;
    use std::os::unix::fs::symlink;

    let root = test_root("atomic-temp-symlink");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create test root");
    let outside = root.join("outside.txt");
    let planted = root.join("planted.tmp");
    let fresh = root.join("fresh.tmp");
    fs::write(&outside, "keep").expect("write outside target");
    symlink(&outside, &planted).expect("plant temp symlink");

    let (reserved, mut file) = reserve_adjacent_temp([planted, fresh.clone()])
        .expect("reserve the fresh candidate after the planted path");
    file.write_all(b"new").expect("write reserved file");
    drop(file);

    assert_eq!(reserved, fresh);
    assert_eq!(
        fs::read_to_string(outside).expect("read outside target"),
        "keep"
    );
    assert_eq!(fs::read_to_string(fresh).expect("read fresh temp"), "new");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn temp_reservation_reports_non_collision_error_without_retrying_as_collision() {
    let root = test_root("atomic-temp-error");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create test root");
    let missing_parent = root.join("missing");

    let error = reserve_adjacent_temp([missing_parent.join("first"), root.join("unused")])
        .expect_err("missing parent should stop reservation");

    assert!(!error
        .to_string()
        .contains("unable to reserve a fresh temporary config file"));
    assert!(!root.join("unused").exists());
    let _ = fs::remove_dir_all(root);
}
