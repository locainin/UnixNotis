use std::fs;
use std::os::unix::fs::symlink;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::detect::Detection;
use crate::model::ActionMode;
use crate::service_manager::ServiceManager;

use super::super::super::service::{install_service, uninstall_service};
use super::super::support::{test_context, test_root};
use super::flow_support::{flow_env, flow_paths, lock_env, write_fake_tools, FakeToolMode};

#[test]
fn every_backend_reinstall_without_changes_clears_reload_flag() {
    for (name, paths) in backend_cases("idempotent-install") {
        let detection = Detection {
            owner: None,
            daemons: Vec::new(),
        };
        let mut ctx = test_context(&detection, &paths, ActionMode::Install);
        // Reload state is the user-visible sign that steady service artifacts changed
        let reload_required = Arc::new(AtomicBool::new(false));
        ctx.service_reload_required = reload_required.clone();

        install_service(&mut ctx)
            .unwrap_or_else(|err| panic!("{name} first install failed: {err}"));
        assert!(
            reload_required.load(Ordering::Acquire),
            "{name} first install should mark steady artifacts changed"
        );

        // Start true so the second install must actively clear a stale reload decision
        reload_required.store(true, Ordering::Release);
        install_service(&mut ctx)
            .unwrap_or_else(|err| panic!("{name} unchanged reinstall failed: {err}"));

        assert!(
            !reload_required.load(Ordering::Acquire),
            "{name} unchanged reinstall should not mark steady artifacts changed"
        );

        let _ = fs::remove_dir_all(paths.repo_root.parent().expect("case root"));
    }
}

#[test]
fn every_backend_uninstall_twice_is_safe() {
    let _lock = lock_env();
    for (name, paths) in backend_cases("idempotent-uninstall") {
        let root = paths.repo_root.parent().expect("case root").to_path_buf();
        let fake_bin = root.join("fake-bin");
        let log_path = root.join("calls.log");
        // Disable/refresh commands still execute during uninstall, so route them to fake tools
        let _fake_tools = write_fake_tools(&fake_bin, &log_path, FakeToolMode::Default);
        let _env = flow_env(&root);
        // s6 uninstall refresh checks the live root, even though no real supervisor is running
        fs::create_dir_all(root.join("run").join("s6-rc")).expect("s6 live dir");
        let detection = Detection {
            owner: None,
            daemons: Vec::new(),
        };
        let mut install_ctx = test_context(&detection, &paths, ActionMode::Install);

        install_service(&mut install_ctx)
            .unwrap_or_else(|err| panic!("{name} setup install failed: {err}"));

        let mut uninstall_ctx = test_context(&detection, &paths, ActionMode::Uninstall);
        uninstall_service(&mut uninstall_ctx)
            .unwrap_or_else(|err| panic!("{name} first uninstall failed: {err}"));
        uninstall_service(&mut uninstall_ctx)
            .unwrap_or_else(|err| panic!("{name} second uninstall failed: {err}"));

        let _ = fs::remove_dir_all(root);
    }
}

#[test]
fn every_backend_wrong_primary_artifact_shape_fails_without_mutation() {
    for (name, paths) in backend_cases("wrong-shape") {
        let primary = paths.service.primary_artifact_path();
        let parent = primary.parent().expect("primary parent");
        fs::create_dir_all(parent).expect("primary parent");
        let foreign = paths
            .repo_root
            .parent()
            .expect("case root")
            .join("foreign-target");
        fs::write(&foreign, "foreign").expect("foreign file");
        // A symlink at the owned path is the common unsafe shape across all backend types
        symlink(&foreign, &primary).expect("foreign symlink at primary artifact");
        let detection = Detection {
            owner: None,
            daemons: Vec::new(),
        };
        let mut ctx = test_context(&detection, &paths, ActionMode::Install);

        let err =
            install_service(&mut ctx).expect_err("install should reject wrong artifact shape");

        // The install must fail closed and leave the foreign path untouched
        assert!(
            err.to_string().contains("symlink")
                || err.to_string().contains("unsafe")
                || err.to_string().contains("not managed"),
            "{name} error should explain the unsafe shape: {err}"
        );
        assert_eq!(
            fs::read_link(&primary).expect("foreign symlink remains"),
            foreign
        );

        let _ = fs::remove_dir_all(paths.repo_root.parent().expect("case root"));
    }
}

fn backend_cases(label: &str) -> Vec<(&'static str, crate::paths::InstallPaths)> {
    // One table keeps the same regression assertions applied to every supported backend
    ["systemd", "dinit", "runit", "s6"]
        .into_iter()
        .map(|backend| {
            let root = test_root(&format!("{label}-{backend}"));
            fs::create_dir_all(root.join("repo")).expect("repo dir");
            let service = match backend {
                "systemd" => ServiceManager::systemd_user(
                    root.join("home")
                        .join(".config")
                        .join("systemd")
                        .join("user"),
                ),
                "dinit" => {
                    ServiceManager::dinit_user(root.join("home").join(".config").join("dinit.d"))
                }
                "runit" => {
                    ServiceManager::runit_user(root.join("home").join(".config").join("service"))
                }
                "s6" => ServiceManager::s6_user(
                    root.join("home").join(".local").join("share").join("s6"),
                    root.join("run").join("s6-rc"),
                ),
                _ => unreachable!("backend list is fixed"),
            };
            (backend, flow_paths(&root, service))
        })
        .collect()
}
