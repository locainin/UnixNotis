use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::detect::Detection;
use crate::model::ActionMode;

use super::super::service::{
    install_service, render_service_unit, service_start_mode_from_enabled, ServiceStartMode,
};
use super::support::{test_context, test_paths, test_root};

#[test]
fn install_service_skips_rewrite_when_unit_is_already_current() {
    let root = test_root("install-service-unchanged");
    let paths = test_paths(&root);
    fs::create_dir_all(paths.service.artifact_dir()).expect("make service artifact dir");
    let expected = render_service_unit(&paths);
    fs::write(paths.service.artifact_path(), &expected).expect("write current service artifact");

    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let mut ctx = test_context(&detection, &paths, ActionMode::Install);
    let reload_required = Arc::new(AtomicBool::new(true));
    ctx.service_reload_required = reload_required.clone();

    install_service(&mut ctx).expect("install service should succeed");

    assert_eq!(
        fs::read_to_string(paths.service.artifact_path()).expect("read service artifact"),
        expected
    );
    assert!(!reload_required.load(Ordering::Acquire));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn install_service_marks_reload_when_unit_changes() {
    let root = test_root("install-service-changed");
    let paths = test_paths(&root);
    fs::create_dir_all(paths.service.artifact_dir()).expect("make service artifact dir");
    fs::write(paths.service.artifact_path(), "[Unit]\nDescription=old\n")
        .expect("write old service artifact");

    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let mut ctx = test_context(&detection, &paths, ActionMode::Install);
    let reload_required = Arc::new(AtomicBool::new(false));
    ctx.service_reload_required = reload_required.clone();

    install_service(&mut ctx).expect("install service should succeed");

    assert!(reload_required.load(Ordering::Acquire));
    assert_eq!(
        fs::read_to_string(paths.service.artifact_path()).expect("read service artifact"),
        render_service_unit(&paths)
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn service_start_mode_uses_start_for_enabled_reinstalls() {
    assert_eq!(
        service_start_mode_from_enabled(Some(true)),
        ServiceStartMode::StartOnly
    );
    assert_eq!(
        service_start_mode_from_enabled(Some(false)),
        ServiceStartMode::EnableAndStart
    );
    assert_eq!(
        service_start_mode_from_enabled(None),
        ServiceStartMode::EnableAndStart
    );
}
