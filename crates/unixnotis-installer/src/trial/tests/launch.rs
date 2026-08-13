use std::os::unix::fs::symlink;

use super::launch::{run_trial_with_shim_cleanup, trial_launch_script};
use super::paths::shell_quote;
use crate::test_support::fs::write_executable;

#[test]
fn trial_launch_script_guards_cleanup_with_expected_symlink_target() {
    let root = crate::test_support::fs::unique_temp_path("trial-launch-script");
    let daemon_path = root.join("unixnotis-daemon");
    let shim_path = root.join("home").join(".local/bin/noticenterctl");
    let target_path = root.join("target/debug/noticenterctl");
    let daemon = shell_quote(&daemon_path.to_string_lossy());
    let shim = shell_quote(&shim_path.to_string_lossy());
    let target = shell_quote(&target_path.to_string_lossy());
    let script = trial_launch_script(&daemon, &shim, &target);

    // Signal-time cleanup must not be a blind rm of whatever is at the shim path
    assert!(script.contains(&format!("[ -L {shim} ]")));
    assert!(script.contains(&format!("readlink -- {shim}")));
    assert!(script.contains(&format!("= {target}")));
    assert!(script.contains(&format!("rm -f -- {shim}")));
}

#[test]
fn shell_quote_preserves_spaces_and_embedded_single_quotes() {
    let quoted = shell_quote("/tmp/unix notis/it's fine");

    // POSIX single-quote escaping closes, emits a quoted single quote, then reopens
    assert_eq!(quoted, "'/tmp/unix notis/it'\"'\"'s fine'");
}

#[test]
fn trial_launch_ignores_shell_from_inherited_path() {
    let _lock = crate::test_support::env::test_env_lock();
    let root =
        std::env::temp_dir().join(format!("unixnotis-trial-shell-path-{}", std::process::id()));
    let path_bin = root.join("path-bin");
    let daemon = root.join("daemon");
    let target = root.join("noticenterctl-target");
    let shim = root.join("noticenterctl-shim");
    let marker = root.join("path-shell-ran");
    std::fs::create_dir_all(&path_bin).expect("path bin");
    write_executable(
        &path_bin.join("sh"),
        &format!("#!/bin/sh\nprintf hit > {}\nexit 0\n", marker.display()),
    );
    write_executable(&daemon, "#!/bin/sh\nexit 0\n");
    std::fs::write(&target, b"control").expect("control target");
    symlink(&target, &shim).expect("control shim");
    let _path = EnvGuard::set("PATH", &path_bin);

    let status = run_trial_with_shim_cleanup(&daemon, &shim, &target)
        .expect("trusted trial shell should run");

    assert!(status.success());
    assert!(!marker.exists());
    assert!(!shim.exists());

    let _ = std::fs::remove_dir_all(root);
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
