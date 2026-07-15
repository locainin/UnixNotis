use super::super::{ensure_hyprland_autostart, remove_hyprland_autostart};
use super::super::{HYPR_BOOTSTRAP_END, HYPR_BOOTSTRAP_START};
use crate::actions::ActionContext;
use crate::app::events::UiMessage;
use crate::detect::Detection;
use crate::model::ActionMode;
use crate::paths::InstallPaths;
use crate::service_manager::ServiceManager;
use crate::test_support::env::EnvGuard;
use std::fs;
use std::os::unix::fs::symlink;
use std::sync::atomic::AtomicBool;
use std::sync::{mpsc, Arc};
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn hyprland_autostart_supports_config_symlink_to_regular_file_inside_home() {
    let _lock = crate::test_support::env::test_env_lock();
    let root = test_root("hyprland-config-symlink");
    let home = root.join("home");
    let config_home = home.join("xdg");
    let hypr_dir = config_home.join("hypr");
    let config_link = hypr_dir.join("hyprland.lua");
    let target = home.join("dotfiles").join("hyprland.lua");
    fs::create_dir_all(&hypr_dir).expect("hypr dir");
    fs::create_dir_all(target.parent().expect("dotfile parent")).expect("dotfile dir");
    fs::write(&target, "-- retained\n").expect("hypr target");
    symlink(&target, &config_link).expect("config symlink");
    let _home = EnvGuard::set("HOME", &home);
    let _xdg = EnvGuard::set("XDG_CONFIG_HOME", &config_home);
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let paths = InstallPaths {
        repo_root: root.clone(),
        bin_dir: root.join("bin"),
        service: ServiceManager::systemd_user(root.join("systemd")),
    };
    let (tx, rx) = mpsc::sync_channel::<UiMessage>(8);
    let mut ctx = ActionContext {
        detection: &detection,
        paths: &paths,
        install_state: None,
        log_tx: tx,
        action_mode: ActionMode::Install,
        restore_backup: None,
        service_reload_required: Arc::new(AtomicBool::new(false)),
    };

    ensure_hyprland_autostart(&mut ctx);

    let installed = fs::read_to_string(&target).expect("updated dotfile target");
    assert!(installed.starts_with("-- retained\n"));
    assert!(installed.contains(HYPR_BOOTSTRAP_START));
    assert!(fs::symlink_metadata(&config_link)
        .expect("config link remains")
        .file_type()
        .is_symlink());
    remove_hyprland_autostart(&mut ctx);
    assert_eq!(
        fs::read_to_string(&target).expect("cleaned dotfile target"),
        "-- retained\n"
    );
    let logs = rx.try_iter().collect::<Vec<_>>();
    assert!(logs.iter().any(|message| matches!(
        message,
        UiMessage::Worker(crate::app::events::WorkerEvent::LogLine(line))
            if line.contains("Updated Hyprland config")
    )));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn ensure_hyprland_autostart_rejects_config_symlink_outside_home() {
    let _lock = crate::test_support::env::test_env_lock();
    let root = test_root("hyprland-config-outside-home");
    let home = root.join("home");
    let config_home = home.join("xdg");
    let hypr_dir = config_home.join("hypr");
    let config_link = hypr_dir.join("hyprland.lua");
    let outside = root.join("outside.lua");
    fs::create_dir_all(&hypr_dir).expect("hypr dir");
    fs::write(&outside, "-- protected\n").expect("outside config");
    symlink(&outside, &config_link).expect("config symlink");
    let _home = EnvGuard::set("HOME", &home);
    let _xdg = EnvGuard::set("XDG_CONFIG_HOME", &config_home);
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let paths = InstallPaths {
        repo_root: root.clone(),
        bin_dir: root.join("bin"),
        service: ServiceManager::systemd_user(root.join("systemd")),
    };
    let (tx, rx) = mpsc::sync_channel::<UiMessage>(8);
    let mut ctx = ActionContext {
        detection: &detection,
        paths: &paths,
        install_state: None,
        log_tx: tx,
        action_mode: ActionMode::Install,
        restore_backup: None,
        service_reload_required: Arc::new(AtomicBool::new(false)),
    };

    ensure_hyprland_autostart(&mut ctx);

    assert_eq!(
        fs::read_to_string(&outside).expect("outside config remains"),
        "-- protected\n"
    );
    let logs = rx.try_iter().collect::<Vec<_>>();
    assert!(logs.iter().any(|message| matches!(
        message,
        UiMessage::Worker(crate::app::events::WorkerEvent::LogLine(line))
            if line.contains("outside the home directory")
    )));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn remove_hyprland_autostart_strips_managed_block_from_real_config() {
    let _lock = crate::test_support::env::test_env_lock();
    let root = test_root("hyprland-remove-block");
    let config_home = root.join("xdg");
    let hypr_dir = config_home.join("hypr");
    let config_path = hypr_dir.join("hyprland.conf");
    fs::create_dir_all(&hypr_dir).expect("hypr dir");
    fs::write(
        &config_path,
        format!("before\n{HYPR_BOOTSTRAP_START}\nexec-once = foo\n{HYPR_BOOTSTRAP_END}\nafter\n"),
    )
    .expect("hypr config");
    let _xdg = EnvGuard::set("XDG_CONFIG_HOME", &config_home);
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let paths = InstallPaths {
        repo_root: root.clone(),
        bin_dir: root.join("bin"),
        service: ServiceManager::systemd_user(root.join("systemd")),
    };
    let (tx, _rx) = mpsc::sync_channel::<UiMessage>(8);
    let mut ctx = ActionContext {
        detection: &detection,
        paths: &paths,
        install_state: None,
        log_tx: tx,
        action_mode: ActionMode::Uninstall,
        restore_backup: None,
        service_reload_required: Arc::new(AtomicBool::new(false)),
    };

    remove_hyprland_autostart(&mut ctx);

    assert_eq!(
        fs::read_to_string(&config_path).expect("hypr config remains"),
        "before\nafter\n"
    );
    let _ = fs::remove_dir_all(&root);
}

fn test_root(name: &str) -> std::path::PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "unixnotis-installer-hyprland-{name}-{}-{stamp}",
        std::process::id()
    ))
}
