use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{mpsc, Arc};

use super::{
    active_unit_metadata, classify_installation_channel, classify_installation_channel_at,
    installed_system_package_paths_at, parse_active_unit_metadata, parse_exec_start_path,
    path_entry_exists, property_value, reject_channel, reject_classified_channel,
    reject_conflicting_installation_channel, validate_systemctl_output, ActiveUnitMetadata,
    InstallationChannel, MAX_SYSTEMCTL_OUTPUT_BYTES, SYSTEM_BINARY_ROOT, SYSTEM_UNIT_ROOT,
};
use crate::actions::ActionContext;
use crate::app::events::{UiMessage, WorkerEvent};
use crate::detect::Detection;
use crate::model::ActionMode;
use crate::paths::InstallPaths;
use crate::service_manager::ServiceManager;
use std::os::unix::process::ExitStatusExt;

struct TestHomeLayout {
    unit_root: PathBuf,
    binary_root: PathBuf,
}

fn test_home_layout(label: &str) -> TestHomeLayout {
    // Each test gets an isolated home layout instead of assuming an account path
    let home = crate::test_support::fs::unique_temp_path(label).join("home");
    TestHomeLayout {
        unit_root: home.join(".config").join("systemd").join("user"),
        binary_root: home.join(".local").join("bin"),
    }
}

fn test_context(root: &Path) -> (Detection, InstallPaths) {
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let paths = InstallPaths {
        repo_root: root.join("repo"),
        bin_dir: root.join("home").join(".local").join("bin"),
        service: ServiceManager::systemd_user(
            root.join("home")
                .join(".config")
                .join("systemd")
                .join("user"),
        ),
    };
    (detection, paths)
}

fn action_context<'a>(_detection: &'a Detection, paths: &'a InstallPaths) -> ActionContext<'a> {
    let (log_tx, _log_rx) = mpsc::sync_channel::<UiMessage>(32);
    ActionContext {
        paths,
        install_state: None,
        log_tx,
        action_mode: ActionMode::Install,
        restore_backup: None,
        service_reload_required: Arc::new(AtomicBool::new(false)),
    }
}

#[test]
fn matching_home_and_system_paths_select_one_installation_channel() {
    let home = test_home_layout("installation-channel-matching");
    let system = test_home_layout("installation-channel-matching-system");
    materialize_channel(&home);
    materialize_channel(&system);
    assert_eq!(
        classify_installation_channel_at(
            &home.unit_root.join("unixnotis-daemon.service"),
            &home.binary_root.join("unixnotis-daemon"),
            &home.unit_root,
            &home.binary_root,
            &system.unit_root,
            &system.binary_root,
        ),
        InstallationChannel::HomeLocal
    );
    assert_eq!(
        classify_installation_channel_at(
            &system.unit_root.join("unixnotis-daemon.service"),
            &system.binary_root.join("unixnotis-daemon"),
            &home.unit_root,
            &home.binary_root,
            &system.unit_root,
            &system.binary_root,
        ),
        InstallationChannel::SystemPackage
    );
}

#[test]
fn one_existing_channel_does_not_require_the_other_policy_root_to_exist() {
    let home = test_home_layout("installation-channel-one-root-home");
    let system = test_home_layout("installation-channel-one-root-system");
    materialize_channel(&system);

    assert_eq!(
        classify_installation_channel_at(
            &system.unit_root.join("unixnotis-daemon.service"),
            &system.binary_root.join("unixnotis-daemon"),
            &home.unit_root,
            &home.binary_root,
            &system.unit_root,
            &system.binary_root,
        ),
        InstallationChannel::SystemPackage
    );

    fs::remove_dir_all(
        system
            .unit_root
            .ancestors()
            .nth(4)
            .expect("system fixture root"),
    )
    .expect("remove system fixture");
    materialize_channel(&home);
    assert_eq!(
        classify_installation_channel_at(
            &home.unit_root.join("unixnotis-daemon.service"),
            &home.binary_root.join("unixnotis-daemon"),
            &home.unit_root,
            &home.binary_root,
            &system.unit_root,
            &system.binary_root,
        ),
        InstallationChannel::HomeLocal
    );
    fs::remove_dir_all(
        home.unit_root
            .ancestors()
            .nth(4)
            .expect("home fixture root"),
    )
    .expect("remove home fixture");
}

#[test]
fn crossed_unit_and_binary_paths_are_always_mixed() {
    let home = test_home_layout("installation-channel-crossed");
    let system = test_home_layout("installation-channel-crossed-system");
    materialize_channel(&home);
    materialize_channel(&system);
    let home_unit = home.unit_root.join("unixnotis-daemon.service");
    let home_binary = home.binary_root.join("unixnotis-daemon");
    let system_unit = system.unit_root.join("unixnotis-daemon.service");
    let system_binary = system.binary_root.join("unixnotis-daemon");
    for (unit, binary) in [(&home_unit, &system_binary), (&system_unit, &home_binary)] {
        assert_eq!(
            classify_installation_channel_at(
                unit,
                binary,
                &home.unit_root,
                &home.binary_root,
                &system.unit_root,
                &system.binary_root,
            ),
            InstallationChannel::Mixed
        );
    }
}

#[test]
fn custom_paths_are_not_silently_treated_as_home_or_package_installs() {
    let root = crate::test_support::fs::unique_temp_path("installation-channel-custom");
    let home = test_home_layout("installation-channel-custom-home");
    assert_eq!(
        classify_installation_channel(
            &root.join("custom-units").join("unixnotis-daemon.service"),
            &root.join("custom-bin").join("unixnotis-daemon"),
            &home.unit_root,
            &home.binary_root,
        ),
        InstallationChannel::Unknown
    );
}

#[test]
fn channel_classification_follows_cross_channel_symlink_targets() {
    use std::os::unix::fs::symlink;

    let home = test_home_layout("installation-channel-link-home");
    let system = test_home_layout("installation-channel-link-system");
    materialize_channel(&home);
    materialize_channel(&system);
    let linked_binary = home.binary_root.join("linked-daemon");
    symlink(system.binary_root.join("unixnotis-daemon"), &linked_binary)
        .expect("create home-to-system binary link");

    assert_eq!(
        classify_installation_channel_at(
            &home.unit_root.join("unixnotis-daemon.service"),
            &linked_binary,
            &home.unit_root,
            &home.binary_root,
            &system.unit_root,
            &system.binary_root,
        ),
        InstallationChannel::Mixed
    );

    let linked_unit = system.unit_root.join("linked.service");
    symlink(
        home.unit_root.join("unixnotis-daemon.service"),
        &linked_unit,
    )
    .expect("create system-to-home unit link");
    assert_eq!(
        classify_installation_channel_at(
            &linked_unit,
            &system.binary_root.join("unixnotis-daemon"),
            &home.unit_root,
            &home.binary_root,
            &system.unit_root,
            &system.binary_root,
        ),
        InstallationChannel::Mixed
    );
}

#[test]
fn dangling_channel_link_is_unknown() {
    use std::os::unix::fs::symlink;

    let home = test_home_layout("installation-channel-dangling-home");
    let system = test_home_layout("installation-channel-dangling-system");
    materialize_channel(&home);
    materialize_channel(&system);
    let dangling = home.binary_root.join("dangling-daemon");
    symlink(home.binary_root.join("missing-daemon"), &dangling).expect("create dangling link");

    assert_eq!(
        classify_installation_channel_at(
            &home.unit_root.join("unixnotis-daemon.service"),
            &dangling,
            &home.unit_root,
            &home.binary_root,
            &system.unit_root,
            &system.binary_root,
        ),
        InstallationChannel::Unknown
    );
}

#[test]
fn unrelated_object_under_the_local_prefix_is_not_a_managed_binary_channel() {
    let home = test_home_layout("installation-channel-local-prefix-home");
    let system = test_home_layout("installation-channel-local-prefix-system");
    materialize_channel(&home);
    materialize_channel(&system);
    let local_root = home.binary_root.parent().expect("local root");
    let unrelated_root = local_root.join("share").join("unrelated");
    fs::create_dir_all(&unrelated_root).expect("create unrelated local directory");
    let unrelated_binary = unrelated_root.join("unixnotis-daemon");
    fs::write(&unrelated_binary, "unrelated binary").expect("write unrelated local binary");

    assert_eq!(
        classify_installation_channel_at(
            &home.unit_root.join("unixnotis-daemon.service"),
            &unrelated_binary,
            &home.unit_root,
            &home.binary_root,
            &system.unit_root,
            &system.binary_root,
        ),
        InstallationChannel::Unknown
    );
}

fn materialize_channel(layout: &TestHomeLayout) {
    fs::create_dir_all(&layout.unit_root).expect("create channel unit root");
    fs::create_dir_all(&layout.binary_root).expect("create channel binary root");
    fs::write(
        layout.unit_root.join("unixnotis-daemon.service"),
        "[Service]\n",
    )
    .expect("write channel unit");
    fs::write(layout.binary_root.join("unixnotis-daemon"), "binary").expect("write channel binary");
}

#[test]
fn systemd_exec_start_parser_reads_only_the_structured_path_field() {
    let home = test_home_layout("exec-start-parser");
    let executable = home.binary_root.join("unixnotis-daemon");
    let executable = executable.to_string_lossy();
    let metadata = format!("{{ path={executable} ; argv[]={executable} ; ignore_errors=no ; }}");
    assert_eq!(parse_exec_start_path(&metadata), Some(executable.as_ref()));
    assert_eq!(parse_exec_start_path("argv[]=/tmp/fake"), None);
}

#[test]
fn systemd_property_parser_requires_an_exact_nonempty_key() {
    let home = test_home_layout("property-parser");
    let fragment = home.unit_root.join("unixnotis-daemon.service");
    let executable = home.binary_root.join("unixnotis-daemon");
    let output = format!(
        "FragmentPath={}\nExecStart={{ path={} ; }}\n",
        fragment.display(),
        executable.display()
    );

    assert_eq!(
        property_value(&output, "FragmentPath"),
        Some(fragment.to_string_lossy().as_ref())
    );
    assert_eq!(
        property_value(&output, "ExecStart"),
        Some(format!("{{ path={} ; }}", executable.display()).as_str())
    );
    assert_eq!(property_value(&output, "Path"), None);
    assert_eq!(property_value("FragmentPath=\n", "FragmentPath"), None);
    assert_eq!(
        property_value("FragmentPathx=/tmp/wrong\n", "FragmentPath"),
        None
    );
}

#[test]
fn systemd_unit_metadata_accepts_loaded_units_and_absent_units() {
    let home = test_home_layout("loaded-unit-metadata");
    let fragment = home.unit_root.join("unixnotis-daemon.service");
    let executable = home.binary_root.join("unixnotis-daemon");
    let loaded = format!(
        "LoadState=loaded\nUnitFileState=enabled\nFragmentPath={}\nExecStart={{ path={} ; }}\n",
        fragment.display(),
        executable.display()
    );
    assert_eq!(
        parse_active_unit_metadata(&loaded).expect("loaded metadata should parse"),
        ActiveUnitMetadata::Paths {
            fragment,
            executable,
        }
    );

    let absent = "LoadState=not-found\nUnitFileState=\nFragmentPath=\nExecStart=\n";
    assert_eq!(
        parse_active_unit_metadata(absent).expect("an absent unit should not be active"),
        ActiveUnitMetadata::Absent
    );
}

#[test]
fn runtime_mask_is_recoverable_during_explicit_installation() {
    let masked = "LoadState=masked\nUnitFileState=masked-runtime\nFragmentPath=\nExecStart=\n";

    assert_eq!(
        parse_active_unit_metadata(masked).expect("runtime mask metadata should parse"),
        ActiveUnitMetadata::RuntimeMasked
    );
}

#[test]
fn persistent_mask_remains_distinct_from_temporary_state() {
    let masked = "LoadState=masked\nUnitFileState=masked\nFragmentPath=\nExecStart=\n";

    assert_eq!(
        parse_active_unit_metadata(masked).expect("persistent mask metadata should parse"),
        ActiveUnitMetadata::PersistentMasked
    );
}

#[test]
fn loaded_unit_still_requires_complete_channel_metadata() {
    let home = test_home_layout("incomplete-unit-metadata");
    let incomplete = format!(
        "LoadState=loaded\nUnitFileState=disabled\nFragmentPath={}\nExecStart=\n",
        home.unit_root.join("unixnotis-daemon.service").display()
    );
    let error = parse_active_unit_metadata(&incomplete)
        .expect_err("loaded units require an executable path");

    assert_eq!(
        error.to_string(),
        "systemctl returned incomplete UnixNotis unit path metadata"
    );
}

#[test]
fn package_artifact_probe_distinguishes_complete_absent_and_partial_installs() {
    let root = crate::test_support::fs::unique_temp_path("package-artifact-probe");
    let unit_root = root.join("units");
    let binary_root = root.join("bin");
    fs::create_dir_all(&unit_root).expect("create test unit root");
    fs::create_dir_all(&binary_root).expect("create test binary root");

    assert_eq!(
        installed_system_package_paths_at(&unit_root, &binary_root)
            .expect("missing package artifacts should be accepted"),
        None
    );

    fs::write(unit_root.join("unixnotis-daemon.service"), "[Service]\n")
        .expect("create package unit fixture");
    let partial = installed_system_package_paths_at(&unit_root, &binary_root)
        .expect_err("partial package artifacts should fail closed");
    assert_eq!(
        partial.to_string(),
        "incomplete system-package UnixNotis artifacts detected; repair or remove the package before installing"
    );

    fs::write(binary_root.join("unixnotis-daemon"), []).expect("create package binary fixture");
    assert_eq!(
        installed_system_package_paths_at(&unit_root, &binary_root)
            .expect("complete package artifacts should be detected"),
        Some((
            unit_root.join("unixnotis-daemon.service"),
            binary_root.join("unixnotis-daemon")
        ))
    );
    fs::remove_dir_all(root).expect("remove package artifact fixture");
}

#[test]
fn path_entry_probe_propagates_errors_other_than_missing_paths() {
    let root = crate::test_support::fs::unique_temp_path("package-artifact-probe-error");
    fs::write(&root, []).expect("create regular file fixture");

    let error = path_entry_exists(&root.join("child"))
        .expect_err("a child below a regular file must report its metadata error");

    assert_ne!(
        error
            .downcast_ref::<std::io::Error>()
            .map(std::io::Error::kind),
        Some(std::io::ErrorKind::NotFound),
        "non-missing metadata errors must remain distinguishable"
    );
    fs::remove_file(root).expect("remove regular file fixture");
}

#[test]
fn systemctl_probe_returns_runtime_mask_metadata_without_dynamic_unit_paths() {
    let _lock = crate::test_support::env::test_env_lock();
    let root = crate::test_support::fs::unique_temp_path("systemctl-runtime-mask");
    let fake_bin = root.join("bin");
    fs::create_dir_all(&fake_bin).expect("create fake command directory");
    crate::test_support::fs::write_executable(
        &fake_bin.join("systemctl"),
        "#!/bin/sh\nprintf '%s\\n' 'LoadState=masked' 'UnitFileState=masked-runtime' 'FragmentPath=' 'ExecStart='\n",
    );
    let _path = crate::system_tools::routing::use_fake_tool_bin(&fake_bin);

    assert_eq!(
        active_unit_metadata().expect("systemctl metadata should be inspected"),
        ActiveUnitMetadata::RuntimeMasked
    );
    fs::remove_dir_all(root).expect("remove fake systemctl fixture");
}

#[test]
fn systemctl_probe_failure_is_not_treated_as_an_absent_unit() {
    let _lock = crate::test_support::env::test_env_lock();
    let root = crate::test_support::fs::unique_temp_path("systemctl-inspection-failure");
    let fake_bin = root.join("bin");
    fs::create_dir_all(&fake_bin).expect("create fake command directory");
    crate::test_support::fs::write_executable(
        &fake_bin.join("systemctl"),
        "#!/bin/sh\nprintf 'user manager unavailable\\n' >&2\nexit 1\n",
    );
    let _path = crate::system_tools::routing::use_fake_tool_bin(&fake_bin);

    let error = active_unit_metadata().expect_err("failed inspection must remain unknown");

    assert!(error
        .to_string()
        .contains("failed to inspect UnixNotis systemd unit"));
    assert!(error.to_string().contains("user manager unavailable"));
    fs::remove_dir_all(root).expect("remove fake systemctl fixture");
}

#[test]
fn systemctl_metadata_budget_accepts_exact_stream_limits_and_rejects_oversize() {
    assert_eq!(MAX_SYSTEMCTL_OUTPUT_BYTES, 32_768);
    let output = |stdout: Vec<u8>, stderr: Vec<u8>, status| std::process::Output {
        status: std::process::ExitStatus::from_raw(status),
        stdout,
        stderr,
    };
    let exact = vec![b'x'; MAX_SYSTEMCTL_OUTPUT_BYTES];

    assert_eq!(
        validate_systemctl_output(output(exact.clone(), Vec::new(), 0))
            .expect("exact stdout budget")
            .len(),
        MAX_SYSTEMCTL_OUTPUT_BYTES
    );
    let exact_stderr = validate_systemctl_output(output(Vec::new(), exact, 1))
        .expect_err("failed systemctl output remains an error");
    assert!(exact_stderr
        .to_string()
        .contains("failed to inspect UnixNotis systemd unit"));
    assert!(validate_systemctl_output(output(
        vec![b'x'; MAX_SYSTEMCTL_OUTPUT_BYTES + 1],
        Vec::new(),
        0,
    ))
    .is_err());
    assert!(validate_systemctl_output(output(
        Vec::new(),
        vec![b'x'; MAX_SYSTEMCTL_OUTPUT_BYTES + 1],
        1,
    ))
    .is_err());
}

#[test]
fn installation_channel_guard_rejects_a_persistent_mask_through_the_real_action_boundary() {
    let _lock = crate::test_support::env::test_env_lock();
    let root = crate::test_support::fs::unique_temp_path("channel-guard-persistent-mask");
    let fake_bin = root.join("bin");
    fs::create_dir_all(&fake_bin).expect("create fake command directory");
    crate::test_support::fs::write_executable(
        &fake_bin.join("systemctl"),
        "#!/bin/sh\nprintf '%s\\n' 'LoadState=masked' 'UnitFileState=masked' 'FragmentPath=' 'ExecStart='\n",
    );
    let _path = crate::system_tools::routing::use_fake_tool_bin(&fake_bin);
    let (detection, paths) = test_context(&root);
    let mut context = action_context(&detection, &paths);

    let error = reject_conflicting_installation_channel(&mut context)
        .expect_err("persistent mask must stop the real install check");

    assert_eq!(
        error.to_string(),
        "UnixNotis systemd unit is persistently masked; run `systemctl --user unmask unixnotis-daemon.service` before installing"
    );
    fs::remove_dir_all(root).expect("remove persistent mask fixture");
}

#[test]
fn conflict_dispatcher_rejects_system_package_paths() {
    let root = crate::test_support::fs::unique_temp_path("channel-dispatch-package");
    let (_detection, paths) = test_context(&root);
    let (log_tx, log_rx) = mpsc::sync_channel::<UiMessage>(8);
    let mut context = ActionContext {
        paths: &paths,
        install_state: None,
        log_tx,
        action_mode: ActionMode::Install,
        restore_backup: None,
        service_reload_required: Arc::new(AtomicBool::new(false)),
    };

    let error = reject_classified_channel(
        &mut context,
        InstallationChannel::SystemPackage,
        &Path::new(SYSTEM_UNIT_ROOT).join("unixnotis-daemon.service"),
        &Path::new(SYSTEM_BINARY_ROOT).join("unixnotis-daemon"),
    )
    .expect_err("system package channel must stop home-local installation");

    assert_eq!(
        error.to_string(),
        "the system-package UnixNotis installation must be removed with its package manager before a home-local install"
    );
    let lines = log_rx
        .try_iter()
        .filter_map(|message| match message {
            UiMessage::Worker(WorkerEvent::LogLine(line)) => Some(line),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(lines
        .iter()
        .any(|line| { line == "Error: system package UnixNotis installation channel" }));
    assert!(lines.iter().any(|line| {
        line == &format!(
            "- unit: {}",
            Path::new(SYSTEM_UNIT_ROOT)
                .join("unixnotis-daemon.service")
                .display()
        )
    }));
    assert!(lines.iter().any(|line| {
        line == &format!(
            "- executable: {}",
            Path::new(SYSTEM_BINARY_ROOT)
                .join("unixnotis-daemon")
                .display()
        )
    }));
}

#[test]
fn resolved_channel_boundary_rejects_objects_outside_managed_roots() {
    let root = crate::test_support::fs::unique_temp_path("channel-boundary-unknown");
    let home = test_home_layout("channel-boundary-unknown-home");
    let system = test_home_layout("channel-boundary-unknown-system");
    materialize_channel(&home);
    materialize_channel(&system);
    let (detection, paths) = test_context(&root);
    let mut context = action_context(&detection, &paths);

    let error = reject_channel(
        &mut context,
        &system.unit_root.join("unixnotis-daemon.service"),
        &system.binary_root.join("unixnotis-daemon"),
    )
    .expect_err("unrecognized resolved objects must reach the channel conflict policy");

    assert!(error
        .to_string()
        .contains("unrecognized installation channel"));
    fs::remove_dir_all(root).ok();
    fs::remove_dir_all(
        home.unit_root
            .ancestors()
            .nth(4)
            .expect("home fixture root"),
    )
    .ok();
    fs::remove_dir_all(
        system
            .unit_root
            .ancestors()
            .nth(4)
            .expect("system fixture root"),
    )
    .ok();
}
