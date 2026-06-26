use std::env;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::Mutex;

// These tests execute the real installer service phases against fake manager binaries
// That catches ordering bugs that pure CommandSpec tests cannot see
use crate::detect::Detection;
use crate::model::ActionMode;
use crate::paths::InstallPaths;
use crate::service_manager::ServiceManager;

use super::super::super::service::{enable_service, install_service};
use super::super::support::{test_context, test_root};

static ENV_LOCK: Mutex<()> = Mutex::new(());

struct EnvGuard {
    // Tests mutate process-wide env, so each guard owns one variable restoration
    key: &'static str,
    old: Option<String>,
}

impl EnvGuard {
    fn set(key: &'static str, value: impl AsRef<str>) -> Self {
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

#[test]
fn systemd_install_flow_runs_reload_env_import_and_enable() {
    let _lock = ENV_LOCK.lock().expect("test env lock");
    // The lock keeps process-wide environment edits serialized across this test module
    let root = test_root("install-flow-systemd");
    let log_path = root.join("calls.log");
    let fake_bin = root.join("fake-bin");
    // Fake binaries let the real installer flow run without depending on host systemd state
    write_fake_tools(&fake_bin, &log_path, None);
    let _env = flow_env(&root, &fake_bin);
    let paths = flow_paths(
        &root,
        ServiceManager::systemd_user(
            root.join("home")
                .join(".config")
                .join("systemd")
                .join("user"),
        ),
    );

    run_install_and_enable(&paths).expect("systemd flow should complete");

    let calls = read_calls(&log_path);
    // systemd still owns reload, D-Bus import, systemd import, and enable --now
    assert_call_order(
        &calls,
        &[
            "program=systemctl argv=[--user][daemon-reload]",
            "program=dbus-update-activation-environment argv=[WAYLAND_DISPLAY]",
            "program=systemctl argv=[--user][--no-pager][import-environment][WAYLAND_DISPLAY]",
            "program=systemctl argv=[--user][enable][--now][unixnotis-daemon.service]",
        ],
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn dinit_install_flow_sets_environment_from_env_and_starts_without_reload() {
    let _lock = ENV_LOCK.lock().expect("test env lock");
    // dinit is command-backed for env sync, so this test checks argv and env separately
    let root = test_root("install-flow-dinit");
    let log_path = root.join("calls.log");
    let fake_bin = root.join("fake-bin");
    write_fake_tools(&fake_bin, &log_path, None);
    let _env = flow_env(&root, &fake_bin);
    let paths = flow_paths(
        &root,
        ServiceManager::dinit_user(root.join("home").join(".config").join("dinit.d")),
    );

    run_install_and_enable(&paths).expect("dinit flow should complete");

    let calls = read_calls(&log_path);
    // dinit should receive variable names in argv and sensitive values through env overrides
    assert!(calls
        .iter()
        .any(|call| call.contains("program=dinitctl argv=[--user][setenv][WAYLAND_DISPLAY]")));
    assert!(calls
        .iter()
        .any(|call| call.contains("WAYLAND_DISPLAY=wayland-test")));
    assert!(
        !calls
            .iter()
            .any(|call| call.contains("WAYLAND_DISPLAY=wayland-test]")),
        "dinit env values should stay out of argv"
    );
    assert_call_order(
        &calls,
        &[
            // dinit intentionally has no first-install reload command
            "program=dinitctl argv=[--user][setenv][WAYLAND_DISPLAY]",
            "program=dinitctl argv=[--user][start][unixnotis-daemon]",
        ],
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn runit_install_flow_syncs_envdir_before_removing_down_and_starting() {
    let _lock = ENV_LOCK.lock().expect("test env lock");
    // runit is the race-sensitive backend because runsvdir can start as soon as ./run exists
    let root = test_root("install-flow-runit");
    let log_path = root.join("calls.log");
    let fake_bin = root.join("fake-bin");
    // The fake sv command fails if start runs before envdir exists or while down remains
    write_fake_tools(&fake_bin, &log_path, Some(FakeToolMode::RunitSv));
    let _env = flow_env(&root, &fake_bin);
    let service_root = root.join("home").join(".config").join("service");
    let paths = flow_paths(&root, ServiceManager::runit_user(service_root));

    run_install_and_enable(&paths).expect("runit flow should complete");

    let service_dir = paths.service.primary_artifact_path();
    // Envdir sync must happen before the temporary down gate is removed
    assert!(service_dir.join("env").join("WAYLAND_DISPLAY").is_file());
    assert!(
        fs::symlink_metadata(service_dir.join("down")).is_err(),
        "runit down gate should be removed immediately before sv start"
    );
    let calls = read_calls(&log_path);
    assert!(
        calls.iter().any(
            |call| call.contains("program=sv argv=[start]") && call.contains("runit_ready=yes")
        ),
        "fake sv should see synced envdir and no down gate at start"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn s6_install_flow_reloads_database_then_changes_service() {
    let _lock = ENV_LOCK.lock().expect("test env lock");
    // s6 needs database reload before the live service change can see the new source tree
    let root = test_root("install-flow-s6");
    let log_path = root.join("calls.log");
    let fake_bin = root.join("fake-bin");
    write_fake_tools(&fake_bin, &log_path, None);
    let _env = flow_env(&root, &fake_bin);
    let paths = flow_paths(
        &root,
        ServiceManager::s6_user(
            root.join("home").join(".local").join("share").join("s6"),
            root.join("run").join("s6-rc"),
        ),
    );

    run_install_and_enable(&paths).expect("s6 flow should complete");

    // s6 is envdir-backed, so environment sync should produce files before s6-rc change
    assert!(paths
        .service
        .primary_artifact_path()
        .join("env")
        .join("WAYLAND_DISPLAY")
        .is_file());
    let calls = read_calls(&log_path);
    assert_call_order(
        &calls,
        &["program=s6-db-reload argv=[-u]", "program=s6-rc argv=[-l]"],
    );
    assert!(calls
        .iter()
        .any(|call| call.contains("[change][unixnotis-daemon]")));
    let _ = fs::remove_dir_all(&root);
}

#[derive(Clone, Copy)]
enum FakeToolMode {
    RunitSv,
}

fn run_install_and_enable(paths: &InstallPaths) -> anyhow::Result<()> {
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let mut ctx = test_context(&detection, paths, ActionMode::Install);
    // Run the same two public install phases used by the TUI worker
    install_service(&mut ctx)?;
    enable_service(&mut ctx)
}

fn flow_paths(root: &Path, service: ServiceManager) -> InstallPaths {
    // Only the service manager varies; binaries and repo roots stay under the temp home
    InstallPaths {
        repo_root: root.join("repo"),
        bin_dir: root.join("home").join(".local").join("bin"),
        service,
    }
}

fn flow_env(root: &Path, fake_bin: &Path) -> Vec<EnvGuard> {
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

fn write_fake_tools(fake_bin: &Path, log_path: &Path, mode: Option<FakeToolMode>) {
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
            ("sv", Some(FakeToolMode::RunitSv)) => fake_runit_sv_script(tool, log_path),
            _ => fake_tool_script(tool, log_path, ""),
        };
        let path = fake_bin.join(tool);
        fs::write(&path, script).expect("write fake tool");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("chmod fake tool");
    }
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
         }} >> {}\n",
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
         [ \"$runit_ready\" != no ]\n",
        sh_quote(log_path)
    )
}

fn read_calls(log_path: &Path) -> Vec<String> {
    // Each fake process appends one line, making order assertions deterministic
    fs::read_to_string(log_path)
        .expect("read fake command log")
        .lines()
        .map(ToString::to_string)
        .collect()
}

fn assert_call_order(calls: &[String], expected: &[&str]) {
    let mut index = 0;
    for needle in expected {
        // Search from the last match forward so unrelated helper commands may appear between steps
        let Some(found) = calls[index..].iter().position(|call| call.contains(needle)) else {
            panic!("missing call containing {needle:?}; calls were {calls:#?}");
        };
        index += found + 1;
    }
}

fn sh_quote(path: &Path) -> String {
    let raw = path.display().to_string();
    // Single-quote escaping is enough because fake scripts only embed local temp paths
    format!("'{}'", raw.replace('\'', "'\\''"))
}
