use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::actions::install::service::flow::rollback_failed_activation_with_quiescence;
use crate::actions::releases::{commit_pending_release, pending_release_exists};
use crate::detect::Detection;
use crate::model::ActionMode;
use crate::service_manager::ServiceManager;

use super::super::super::support::test_context;
use super::super::flow_support::{
    enable_service_with_readiness_and_quiescence, flow_env, flow_paths, install_release_generation,
    lock_env, service_flow_root, write_fake_tools, FakeToolMode,
};

#[test]
fn failed_new_generation_readiness_restores_and_rechecks_the_previous_runtime() {
    let _lock = lock_env();
    let root = service_flow_root("install-generation-readiness-rollback");
    let log_path = root.join("calls.log");
    let fake_bin = root.join("fake-bin");
    let _fake_tools = write_fake_tools(&fake_bin, &log_path, FakeToolMode::Default);
    let _env = flow_env(&root);
    let paths = flow_paths(
        &root,
        ServiceManager::systemd_user(
            root.join("home")
                .join(".config")
                .join("systemd")
                .join("user"),
        ),
    );
    let source = root.join("release-source");
    fs::create_dir_all(&source).expect("create release source");
    let binary = "unixnotis-daemon".to_string();
    write_binary(&source.join(&binary), "old generation");
    let old_generation = install_release_generation(
        &paths,
        &source,
        std::slice::from_ref(&binary),
        || Ok(()),
        || Ok(()),
    )
    .expect("install prior generation");
    commit_pending_release(&paths).expect("commit prior generation");
    write_binary(&source.join(&binary), "new generation");
    install_release_generation(
        &paths,
        &source,
        std::slice::from_ref(&binary),
        || Ok(()),
        || Ok(()),
    )
    .expect("activate pending generation");
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let mut ctx = test_context(&detection, &paths, ActionMode::Install);
    let readiness_calls = AtomicUsize::new(0);

    let error = enable_service_with_readiness_and_quiescence(
        &mut ctx,
        |_ctx| {
            if readiness_calls.fetch_add(1, Ordering::AcqRel) == 0 {
                Err(anyhow::anyhow!("new generation failed readiness"))
            } else {
                Ok(())
            }
        },
        |_paths| Ok(()),
    )
    .expect_err("failed new generation must report activation failure after rollback");

    assert!(error
        .to_string()
        .contains("new generation failed readiness"));
    assert_eq!(readiness_calls.load(Ordering::Acquire), 2);
    assert_eq!(
        fs::read_link(paths.installed_current_link().expect("current path"))
            .expect("restored current link"),
        std::path::Path::new("releases").join(old_generation)
    );
    assert_eq!(
        fs::read_to_string(paths.bin_dir.join(binary)).expect("read restored binary"),
        "old generation"
    );
    fs::remove_dir_all(root).expect("remove readiness rollback fixture");
}

#[test]
fn failure_after_binary_activation_restores_and_restarts_the_previous_runtime() {
    let _lock = lock_env();
    let root = service_flow_root("install-generation-later-step-rollback");
    let log_path = root.join("calls.log");
    let fake_bin = root.join("fake-bin");
    let _fake_tools = write_fake_tools(&fake_bin, &log_path, FakeToolMode::Default);
    let _env = flow_env(&root);
    let paths = flow_paths(
        &root,
        ServiceManager::systemd_user(
            root.join("home")
                .join(".config")
                .join("systemd")
                .join("user"),
        ),
    );
    let source = root.join("release-source");
    fs::create_dir_all(&source).expect("create release source");
    let binary = "unixnotis-daemon".to_string();
    write_binary(&source.join(&binary), "old generation");
    let old_generation = install_release_generation(
        &paths,
        &source,
        std::slice::from_ref(&binary),
        || Ok(()),
        || Ok(()),
    )
    .expect("install prior generation");
    commit_pending_release(&paths).expect("commit prior generation");
    write_binary(&source.join(&binary), "new generation");
    install_release_generation(
        &paths,
        &source,
        std::slice::from_ref(&binary),
        || Ok(()),
        || Ok(()),
    )
    .expect("activate pending generation");
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let mut ctx = test_context(&detection, &paths, ActionMode::Install);
    let readiness_calls = AtomicUsize::new(0);

    let error = rollback_failed_activation_with_quiescence(
        &mut ctx,
        &|_ctx| {
            readiness_calls.fetch_add(1, Ordering::AcqRel);
            Ok(())
        },
        anyhow::anyhow!("service artifact installation failed"),
        |_paths| Ok(()),
    )
    .expect_err("a later step failure must remain an installation failure");

    assert!(error
        .to_string()
        .contains("service artifact installation failed"));
    assert_eq!(readiness_calls.load(Ordering::Acquire), 1);
    assert_eq!(
        fs::read_link(paths.installed_current_link().expect("current path"))
            .expect("restored current link"),
        std::path::Path::new("releases").join(old_generation)
    );
    assert_eq!(
        fs::read_to_string(paths.bin_dir.join(binary)).expect("read restored binary"),
        "old generation"
    );
    fs::remove_dir_all(root).expect("remove later-step rollback fixture");
}

#[test]
fn successful_stop_result_cannot_roll_back_while_runtime_remains_live() {
    let _lock = lock_env();
    let root = service_flow_root("install-generation-live-runtime-blocks-rollback");
    let log_path = root.join("calls.log");
    let fake_bin = root.join("fake-bin");
    let _fake_tools = write_fake_tools(&fake_bin, &log_path, FakeToolMode::Default);
    let _env = flow_env(&root);
    let paths = flow_paths(
        &root,
        ServiceManager::systemd_user(
            root.join("home")
                .join(".config")
                .join("systemd")
                .join("user"),
        ),
    );
    let source = root.join("release-source");
    fs::create_dir_all(&source).expect("create release source");
    let binary = "unixnotis-daemon".to_string();
    write_binary(&source.join(&binary), "old generation");
    install_release_generation(
        &paths,
        &source,
        std::slice::from_ref(&binary),
        || Ok(()),
        || Ok(()),
    )
    .expect("install prior generation");
    commit_pending_release(&paths).expect("commit prior generation");
    write_binary(&source.join(&binary), "new generation");
    let new_generation = install_release_generation(
        &paths,
        &source,
        std::slice::from_ref(&binary),
        || Ok(()),
        || Ok(()),
    )
    .expect("activate pending generation");
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let mut ctx = test_context(&detection, &paths, ActionMode::Install);

    let error = rollback_failed_activation_with_quiescence(
        &mut ctx,
        &|_ctx| Ok(()),
        anyhow::anyhow!("new generation failed readiness"),
        |_paths| Err(anyhow::anyhow!("notification owner is still live")),
    )
    .expect_err("a live rejected runtime must block disk rollback");

    assert!(error
        .to_string()
        .contains("service manager reported a successful stop"));
    assert_eq!(
        fs::read_link(paths.installed_current_link().expect("current path"))
            .expect("retain active generation"),
        std::path::Path::new("releases").join(new_generation)
    );
    assert!(pending_release_exists(&paths).expect("retain pending rollback journal"));
    fs::remove_dir_all(root).expect("remove live runtime rollback fixture");
}

fn write_binary(path: &std::path::Path, contents: &str) {
    fs::write(path, contents).expect("write release binary");
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("set release binary mode");
}
