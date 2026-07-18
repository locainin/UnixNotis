use super::super::{
    resolve_service_manager_paths, runit_user_dir, s6_user_dir, ServiceManagerKind,
};

#[test]
fn non_s6_managers_do_not_resolve_a_live_runtime_root() {
    for kind in [
        ServiceManagerKind::Systemd,
        ServiceManagerKind::Dinit,
        ServiceManagerKind::Runit,
    ] {
        let paths = resolve_service_manager_paths(kind).expect("resolve manager paths");
        assert_eq!(paths.kind, kind);
        assert!(paths.live_root.is_none());
        assert!(paths.artifact_root.is_absolute());
    }
}

#[test]
fn default_runit_and_s6_artifact_roots_are_absolute() {
    assert!(runit_user_dir().expect("runit root").is_absolute());
    assert!(s6_user_dir().expect("s6 root").is_absolute());
}
