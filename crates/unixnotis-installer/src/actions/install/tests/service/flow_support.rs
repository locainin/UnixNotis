use std::env;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use crate::detect::Detection;
use crate::model::ActionMode;
use crate::paths::InstallPaths;
use crate::service_manager::ServiceManager;

use super::super::super::service::{enable_service, install_service, uninstall_service};
use super::super::support::{test_context, test_root};

pub(super) static ENV_LOCK: Mutex<()> = Mutex::new(());

pub(super) fn lock_env() -> MutexGuard<'static, ()> {
    // A failed test can poison the lock, but cleanup guards still restore env on drop
    ENV_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub(super) struct EnvGuard {
    // Tests mutate process-wide env, so each guard owns one variable restoration
    key: &'static str,
    old: Option<String>,
}

impl EnvGuard {
    pub(super) fn set(key: &'static str, value: impl AsRef<str>) -> Self {
        // Flow tests replace session variables briefly so child commands see a controlled login
        let old = env::var(key).ok();
        env::set_var(key, value.as_ref());
        Self { key, old }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        // Restore global process env so later tests do not inherit fake session state
        if let Some(old) = &self.old {
            env::set_var(self.key, old);
        } else {
            env::remove_var(self.key);
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum FakeToolMode {
    Default,
    RunitSv,
}

pub(super) fn run_install_and_enable(paths: &InstallPaths) -> anyhow::Result<()> {
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let mut ctx = test_context(&detection, paths, ActionMode::Install);
    // Run the same two public install phases used by the TUI worker
    install_service(&mut ctx)?;
    enable_service(&mut ctx)
}

pub(super) fn run_install_only(paths: &InstallPaths) -> anyhow::Result<()> {
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let mut ctx = test_context(&detection, paths, ActionMode::Install);
    install_service(&mut ctx)
}

pub(super) fn run_enable_only(paths: &InstallPaths) -> anyhow::Result<()> {
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let mut ctx = test_context(&detection, paths, ActionMode::Install);
    enable_service(&mut ctx)
}

pub(super) fn run_uninstall_only(paths: &InstallPaths) -> anyhow::Result<()> {
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let mut ctx = test_context(&detection, paths, ActionMode::Uninstall);
    uninstall_service(&mut ctx)
}

pub(super) fn flow_paths(root: &Path, service: ServiceManager) -> InstallPaths {
    // Only the service manager varies; binaries and repo roots stay under the temp home
    InstallPaths {
        repo_root: root.join("repo"),
        bin_dir: root.join("home").join(".local").join("bin"),
        service,
    }
}

pub(super) fn flow_env(root: &Path, fake_bin: &Path) -> Vec<EnvGuard> {
    let original_path = env::var("PATH").unwrap_or_default();
    let mut path_entries = vec![fake_bin.to_path_buf()];
    // Preserve the real PATH after fake tools so sh and coreutils still resolve normally
    path_entries.extend(env::split_paths(&original_path));
    let fake_path = env::join_paths(path_entries).expect("fake PATH");
    vec![
        EnvGuard::set("HOME", root.join("home").display().to_string()),
        EnvGuard::set("SHELL", "/bin/sh"),
        EnvGuard::set("PATH", fake_path.to_string_lossy()),
        EnvGuard::set("WAYLAND_DISPLAY", "wayland-test"),
        EnvGuard::set("XDG_RUNTIME_DIR", root.join("run").display().to_string()),
        EnvGuard::set("XDG_CURRENT_DESKTOP", "Hyprland"),
        EnvGuard::set("XDG_SESSION_TYPE", "wayland"),
        EnvGuard::set("XDG_SESSION_DESKTOP", "Hyprland"),
        EnvGuard::set("DISPLAY", ":99"),
    ]
}

pub(super) fn write_fake_tools(fake_bin: &Path, log_path: &Path, mode: FakeToolMode) {
    fs::create_dir_all(fake_bin).expect("make fake bin");
    // All tools listed here are backends or helper commands used by the service install flow
    for tool in [
        "systemctl",
        "dbus-update-activation-environment",
        "dinitctl",
        "sv",
        "chpst",
        "s6-db-reload",
        "s6-rc",
        "s6-svstat",
        "s6-envdir",
    ] {
        // Most fake tools only log argv and selected env; runit sv has extra ordering checks
        let script = match (tool, mode) {
            ("sv", FakeToolMode::RunitSv) => fake_runit_sv_script(tool, log_path),
            _ => fake_tool_script(tool, log_path, ""),
        };
        let path = fake_bin.join(tool);
        fs::write(&path, script).expect("write fake tool");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("chmod fake tool");
    }
}

pub(super) fn fake_failure_env(program: &'static str, contains: &'static str) -> [EnvGuard; 2] {
    [
        EnvGuard::set("UNIXNOTIS_FAKE_FAIL_PROGRAM", program),
        EnvGuard::set("UNIXNOTIS_FAKE_FAIL_CONTAINS", contains),
    ]
}

pub(super) fn read_calls(log_path: &Path) -> Vec<String> {
    // Each fake process appends one line, making order assertions deterministic
    fs::read_to_string(log_path)
        .expect("read fake command log")
        .lines()
        .map(ToString::to_string)
        .collect()
}

pub(super) fn assert_call_order(calls: &[String], expected: &[&str]) {
    let mut index = 0;
    for needle in expected {
        // Search from the last match forward so unrelated helper commands may appear between steps
        let Some(found) = calls[index..].iter().position(|call| call.contains(needle)) else {
            panic!("missing call containing {needle:?}; calls were {calls:#?}");
        };
        index += found + 1;
    }
}

pub(super) fn service_flow_root(name: &str) -> std::path::PathBuf {
    test_root(name)
}

fn fake_tool_script(tool: &str, log_path: &Path, extra: &str) -> String {
    // Shell scripts keep the integration harness close to real process execution
    format!(
        "#!/bin/sh\n\
         {extra}\n\
         {{\n\
         printf 'program={tool} argv='\n\
         for arg in \"$@\"; do printf '[%s]' \"$arg\"; done\n\
         printf ' WAYLAND_DISPLAY=%s XDG_RUNTIME_DIR=%s\\n' \"${{WAYLAND_DISPLAY-}}\" \"${{XDG_RUNTIME_DIR-}}\"\n\
         }} >> {}\n\
         if [ \"${{UNIXNOTIS_FAKE_FAIL_PROGRAM-}}\" = '{tool}' ]; then\n\
         case \" $* \" in *\"${{UNIXNOTIS_FAKE_FAIL_CONTAINS-}}\"*) exit 42 ;; esac\n\
         fi\n",
        sh_quote(log_path)
    )
}

fn fake_runit_sv_script(tool: &str, log_path: &Path) -> String {
    // This fake models the important runit race without starting a real supervisor
    format!(
        "#!/bin/sh\n\
         runit_ready=unchecked\n\
         # sv start must happen after envdir sync and after the down gate is removed\n\
         if [ \"${{1-}}\" = start ]; then\n\
         if [ -f \"${{2-}}/env/WAYLAND_DISPLAY\" ] && [ ! -e \"${{2-}}/down\" ]; then\n\
         runit_ready=yes\n\
         else\n\
         runit_ready=no\n\
         fi\n\
         fi\n\
         {{\n\
         printf 'program={tool} argv='\n\
         for arg in \"$@\"; do printf '[%s]' \"$arg\"; done\n\
         printf ' runit_ready=%s WAYLAND_DISPLAY=%s XDG_RUNTIME_DIR=%s\\n' \"$runit_ready\" \"${{WAYLAND_DISPLAY-}}\" \"${{XDG_RUNTIME_DIR-}}\"\n\
         }} >> {}\n\
         if [ \"${{UNIXNOTIS_FAKE_FAIL_PROGRAM-}}\" = '{tool}' ]; then\n\
         case \" $* \" in *\"${{UNIXNOTIS_FAKE_FAIL_CONTAINS-}}\"*) exit 42 ;; esac\n\
         fi\n\
         [ \"$runit_ready\" != no ]\n",
        sh_quote(log_path)
    )
}

fn sh_quote(path: &Path) -> String {
    let raw = path.display().to_string();
    // Single-quote escaping is enough because fake scripts only embed local temp paths
    format!("'{}'", raw.replace('\'', "'\\''"))
}
