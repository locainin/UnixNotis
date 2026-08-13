use super::reject_root_install;

#[test]
fn root_effective_uid_is_rejected_with_user_level_guidance() {
    let error = reject_root_install(0).expect_err("root must be rejected");

    assert_eq!(
        error.to_string(),
        "unixnotis-installer is user-level; do not run it as root or through sudo"
    );
}

#[test]
fn normal_user_effective_uid_is_accepted() {
    assert!(reject_root_install(1000).is_ok());
}
