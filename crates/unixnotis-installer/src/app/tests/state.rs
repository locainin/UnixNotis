use std::collections::VecDeque;
use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::time::{SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};

use crate::actions::{
    check_install_state, BuildAccelConfigStatus, BuildAccelDetection, InstallationDisposition,
};
use crate::app::{App, BuildAccelMenuMode, BuildAccelState, MenuItem, ProgressState, Screen};
use crate::checks::{CheckItem, CheckState, Checks};
use crate::detect::Detection;
use crate::model::{ActionMode, ResetAction};
use crate::paths::InstallPaths;
use crate::service_manager::{ServiceArtifactKind, ServiceManager};

#[test]
fn menu_items_keep_expected_order() {
    let items = App::menu_items();

    // The menu order is muscle memory in the TUI, so accidental reordering should fail loudly
    assert_eq!(items[0], MenuItem::Action(ActionMode::Test));
    assert_eq!(items[1], MenuItem::Action(ActionMode::Install));
    assert_eq!(items[2], MenuItem::Action(ActionMode::Reset));
    assert_eq!(items[3], MenuItem::Action(ActionMode::Uninstall));
    assert_eq!(items[4], MenuItem::Quit);
}

#[test]
fn selected_menu_clamps_out_of_range_index_to_last_item() {
    let mut app = app_with_build_accel(None);
    app.menu_index = usize::MAX;

    // Clamping prevents stale indices from panicking after menu length changes
    assert_eq!(app.selected_menu(), MenuItem::Quit);
}

#[test]
fn refresh_reloads_environment_checks_instead_of_retaining_stale_state() {
    let _lock = crate::test_support::env::test_env_lock();
    let root = test_root("app-refresh-checks");
    let _session = crate::test_support::env::EnvGuard::set("XDG_SESSION_TYPE", "x11");
    let _display = crate::test_support::env::EnvGuard::set("WAYLAND_DISPLAY", "");
    let _runtime = crate::test_support::env::EnvGuard::set("XDG_RUNTIME_DIR", root.join("run"));
    let _home = crate::test_support::env::EnvGuard::set("HOME", root.join("home"));
    let _config = crate::test_support::env::EnvGuard::set("XDG_CONFIG_HOME", root.join("config"));
    let fake_bin = root.join("fake-bin");
    fs::create_dir_all(&fake_bin).expect("fake bin dir");
    let _fake_tools = crate::system_tools::routing::use_fake_tool_bin(&fake_bin);
    let mut app = app_with_build_accel(None);

    assert_eq!(app.checks.wayland.state, CheckState::Ok);
    app.refresh();

    assert_eq!(app.checks.wayland.state, CheckState::Fail);
    fs::remove_dir_all(root).expect("remove refresh fixture");
}

#[test]
fn refresh_backups_replaces_stale_rows_and_resets_selection() {
    let _lock = crate::test_support::env::test_env_lock();
    let root = test_root("app-refresh-backups");
    let config_home = root.join("config");
    let backup = config_home.join("unixnotis").join("Backup-2026-08-08");
    fs::create_dir_all(&backup).expect("backup fixture");
    let _config = crate::test_support::env::EnvGuard::set("XDG_CONFIG_HOME", &config_home);
    let mut app = app_with_build_accel(None);
    app.restore_backups = vec![root.join("stale-backup")];
    app.restore_menu_index = 9;

    app.refresh_backups();

    assert_eq!(app.restore_backups, [backup]);
    assert_eq!(app.restore_menu_index, 0);
    fs::remove_dir_all(root).expect("remove backup refresh fixture");
}

#[test]
fn build_accel_menu_mode_returns_only_when_no_prompt_state_exists() {
    let app = app_with_build_accel(None);

    // No prompt state means the only valid action is returning to the main menu
    assert_eq!(app.build_accel_menu_mode(), BuildAccelMenuMode::ReturnOnly);
    assert_eq!(app.build_accel_menu_len(), 1);
}

#[test]
fn build_accel_menu_mode_allows_enable_when_tools_exist_and_config_is_missing() {
    let app = app_with_build_accel(Some(BuildAccelDetection {
        sccache_installed: true,
        mold_installed: false,
        config_status: BuildAccelConfigStatus::Missing,
    }));

    // Missing config plus at least one tool gives the user a real enable choice
    assert_eq!(
        app.build_accel_menu_mode(),
        BuildAccelMenuMode::EnableOrSkip
    );
    assert_eq!(app.build_accel_menu_len(), 2);
}

#[test]
fn build_accel_menu_mode_allows_reinstall_for_managed_config() {
    let app = app_with_build_accel(Some(BuildAccelDetection {
        sccache_installed: false,
        mold_installed: false,
        config_status: BuildAccelConfigStatus::Managed {
            wrapper_present: true,
        },
    }));

    // Managed configs can be refreshed even when tool detection changed since the first install
    assert_eq!(app.build_accel_menu_mode(), BuildAccelMenuMode::Reinstall);
    assert_eq!(app.build_accel_menu_len(), 2);
}

#[test]
fn action_label_uses_install_wording_when_state_is_unknown() {
    let app = app_with_build_accel(None);

    // Unknown install state should stay non-destructive in the menu label
    assert_eq!(app.action_label(ActionMode::Install), "Install");
    assert_eq!(app.action_label(ActionMode::Reset), "Reset config");
}

#[test]
fn action_label_distinguishes_healthy_install_from_missing_service_artifact() {
    let _lock = crate::test_support::env::test_env_lock();
    let root = test_root("app-reinstall-label");
    let repo_root = root.join("repo");
    let bin_dir = root.join("bin");
    let systemd_dir = root.join("config").join("systemd").join("user");
    let _home = crate::test_support::env::EnvGuard::set("HOME", root.join("home"));
    let _user = crate::test_support::env::EnvGuard::set("USER", "unixnotis-test");
    let _config_home =
        crate::test_support::env::EnvGuard::set("XDG_CONFIG_HOME", root.join("config"));
    let _runit =
        crate::test_support::env::EnvGuard::set("UNIXNOTIS_RUNIT_SERVICE_DIR", root.join("runit"));
    let _svdir = crate::test_support::env::EnvGuard::set("SVDIR", root.join("runit"));
    let _s6_data =
        crate::test_support::env::EnvGuard::set("UNIXNOTIS_S6_DATA_DIR", root.join("s6"));
    let _s6_live =
        crate::test_support::env::EnvGuard::set("UNIXNOTIS_S6RC_LIVE_DIR", root.join("s6-live"));
    fs::create_dir_all(&repo_root).expect("repo dir");
    fs::create_dir_all(&bin_dir).expect("bin dir");
    let fake_bin = root.join("fake-bin");
    fs::create_dir_all(&fake_bin).expect("fake bin dir");
    crate::test_support::fs::write_executable(
        &fake_bin.join("systemctl"),
        "#!/bin/sh\ncase \"$*\" in\n  *is-enabled*) exit 0 ;;\nesac\nprintf '%s\\n' 'LoadState=loaded' 'ActiveState=inactive'\nexit 0\n",
    );
    let _fake_tools = crate::system_tools::routing::use_fake_tool_bin(&fake_bin);

    // A complete release inventory keeps the install-state check focused on one binary
    fs::write(
        repo_root.join("unixnotis-release.json"),
        r#"{"version":"test","binaries":["unixnotis-daemon"]}"#,
    )
    .expect("release metadata");
    fs::create_dir_all(repo_root.join("bin")).expect("release source directory");
    fs::write(repo_root.join("bin/unixnotis-daemon"), "release source")
        .expect("release source binary");
    let binary_contents = b"#!/bin/sh\n";
    let digest = format!("{:x}", Sha256::digest(binary_contents));
    let size = u64::try_from(binary_contents.len()).expect("test binary size fits u64");
    let build_id = release_build_id("test", "unixnotis-daemon", size, &digest);
    let generation = format!("test-{build_id}");
    let release_root = root
        .join("lib")
        .join("unixnotis")
        .join("releases")
        .join(&generation);
    let release_binary = release_root.join("bin").join("unixnotis-daemon");
    fs::create_dir_all(release_binary.parent().expect("release binary parent"))
        .expect("release binary directory");
    fs::write(&release_binary, binary_contents).expect("installed release binary");
    fs::set_permissions(&release_binary, fs::Permissions::from_mode(0o755))
        .expect("installed release binary mode");
    fs::write(
        release_root.join("manifest.json"),
        format!(
            "{{\"schema_version\":1,\"package_version\":\"test\",\"build_id\":\"{build_id}\",\"binaries\":{{\"unixnotis-daemon\":{{\"size\":{size},\"sha256\":\"{digest}\"}}}}}}"
        ),
    )
    .expect("installed release manifest");
    symlink(
        std::path::Path::new("releases").join(generation),
        root.join("lib/unixnotis/current"),
    )
    .expect("current release link");
    symlink(
        "../lib/unixnotis/current/bin/unixnotis-daemon",
        bin_dir.join("unixnotis-daemon"),
    )
    .expect("installed binary entrypoint");

    let service = ServiceManager::systemd_user(systemd_dir);
    for artifact in service.artifacts(&bin_dir) {
        if matches!(artifact.kind, ServiceArtifactKind::File) {
            if let Some(parent) = artifact.path.parent() {
                fs::create_dir_all(parent).expect("artifact parent");
            }
            let contents = artifact
                .contents
                .as_deref()
                .expect("systemd unit artifact contents");

            // The artifact writer is tested elsewhere; this case only needs a
            // safe existing unit so the app label follows the real state check
            fs::write(&artifact.path, contents).expect("service artifact");
        }
    }

    let paths = InstallPaths {
        repo_root,
        bin_dir,
        service,
    };
    let mut app = app_with_build_accel(None);
    app.install_state = Some(check_install_state(&paths));

    // Installed binaries plus a safe service artifact should turn the primary
    // install action into a reinstall action in the TUI
    assert_eq!(app.action_label(ActionMode::Install), "Reinstall");
    assert_eq!(
        app.installation_disposition(),
        InstallationDisposition::InstalledHealthy
    );

    fs::remove_file(paths.service.primary_artifact_path()).expect("remove primary artifact");
    app.install_state = Some(check_install_state(&paths));

    // Existing verified binaries with an incomplete service install need repair, not a fresh install
    assert_eq!(app.action_label(ActionMode::Install), "Repair");
    assert_eq!(
        app.installation_disposition(),
        InstallationDisposition::RepairRequired
    );

    let _ = fs::remove_dir_all(root);
}

fn release_build_id(package_version: &str, binary_name: &str, size: u64, digest: &str) -> String {
    let mut release_digest = Sha256::new();
    release_digest.update(package_version.as_bytes());
    release_digest.update(binary_name.as_bytes());
    release_digest.update(size.to_le_bytes());
    release_digest.update(digest.as_bytes());
    format!("{:x}", release_digest.finalize())
}

fn app_with_build_accel(detection: Option<BuildAccelDetection>) -> App {
    App {
        checks: passing_checks(),
        detection: Detection {
            owner: None,
            daemons: Vec::new(),
        },
        menu_index: 0,
        screen: Screen::Welcome,
        logs: VecDeque::new(),
        steps: Vec::new(),
        progress_state: ProgressState::Idle,
        last_error: None,
        install_state: None,
        progress_ready_at: None,
        build_accel: detection.map(|detection| BuildAccelState {
            detection,
            outcome: None,
        }),
        build_accel_menu_index: 0,
        reset_menu_index: 0,
        reset_action: ResetAction::ResetDefaults,
        restore_backups: Vec::new(),
        restore_menu_index: 0,
        service_manager: None,
        release_status: crate::release::ReleaseStatus::current_only(),
    }
}

fn test_root(name: &str) -> std::path::PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    std::env::temp_dir().join(format!("unixnotis-{name}-{unique}"))
}

fn passing_checks() -> Checks {
    let item = CheckItem {
        label: "test",
        state: CheckState::Ok,
        detail: "ok".to_string(),
    };

    Checks {
        release_archive: false,
        wayland: item.clone(),
        hyprland: item.clone(),
        service_manager: item.clone(),
        cargo: item.clone(),
        pkg_config: item.clone(),
        gtk4_css_features: item.clone(),
        gtk4_layer_shell: item.clone(),
        busctl: item.clone(),
        dbus_update_env: item.clone(),
        install_paths: item.clone(),
        path_contains_bin: item,
    }
}
