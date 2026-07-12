use super::super::{ensure_hyprland_autostart, remove_hyprland_autostart};
use super::super::{HYPR_BOOTSTRAP_END, HYPR_BOOTSTRAP_START};
use crate::actions::ActionContext;
use crate::detect::Detection;
use crate::events::UiMessage;
use crate::model::ActionMode;
use crate::paths::InstallPaths;
use crate::service_manager::ServiceManager;
use std::fs;
use std::os::unix::fs::symlink;
use std::sync::atomic::AtomicBool;
use std::sync::{mpsc, Arc};
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn ensure_hyprland_autostart_rejects_config_symlink_without_touching_target() {
    let _lock = crate::tests::env::test_env_lock();
    let root = test_root("hyprland-config-symlink");
    let config_home = root.join("xdg");
    let hypr_dir = config_home.join("hypr");
    let config_link = hypr_dir.join("hyprland.lua");
    let protected = root.join("protected.lua");
    fs::create_dir_all(&hypr_dir).expect("hypr dir");
    fs::write(&protected, "-- protected\n").expect("protected");
    symlink(&protected, &config_link).expect("config symlink");
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
        fs::read_to_string(&protected).expect("protected remains"),
        "-- protected\n"
    );
    let logs = rx.try_iter().collect::<Vec<_>>();
    assert!(logs.iter().any(|message| matches!(
        message,
        UiMessage::Worker(crate::events::WorkerEvent::LogLine(line))
            if line.contains("refusing to write through symlink")
    )));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn remove_hyprland_autostart_strips_managed_block_from_real_config() {
    let _lock = crate::tests::env::test_env_lock();
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

struct EnvGuard {
    name: &'static str,
    previous: Option<std::ffi::OsString>,
}

impl EnvGuard {
    fn set(name: &'static str, value: impl AsRef<std::ffi::OsStr>) -> Self {
        let previous = std::env::var_os(name);
        std::env::set_var(name, value);
        Self { name, previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        match &self.previous {
            Some(value) => std::env::set_var(self.name, value),
            None => std::env::remove_var(self.name),
        }
    }
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
