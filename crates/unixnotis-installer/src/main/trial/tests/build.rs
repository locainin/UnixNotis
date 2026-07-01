use super::build::trial_binary_paths;
use super::test_support::temp_dir;

#[test]
fn trial_binary_paths_use_debug_profile_outputs() {
    let root = temp_dir("debug-paths");

    let binaries = trial_binary_paths(&root);

    // Trial mode should never point at installed binaries or release outputs by default
    assert_eq!(
        binaries.daemon,
        root.join("target").join("debug").join("unixnotis-daemon")
    );
    assert_eq!(
        binaries.control,
        root.join("target").join("debug").join("noticenterctl")
    );

    let _ = std::fs::remove_dir_all(root);
}
