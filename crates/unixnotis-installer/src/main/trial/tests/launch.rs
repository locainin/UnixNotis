use std::os::unix::fs::symlink;

use super::launch::{run_trial_with_shim_cleanup, trial_launch_script};
use super::paths::shell_quote;
use crate::tests::fs::write_executable;

#[test]
fn trial_launch_script_guards_cleanup_with_expected_symlink_target() {
    let script = trial_launch_script(
        "'/tmp/unixnotis-daemon'",
        "'/home/user/.local/bin/noticenterctl'",
        "'/tmp/target/debug/noticenterctl'",
    );

    // Signal-time cleanup must not be a blind rm of whatever is at the shim path
    assert!(script.contains("[ -L '/home/user/.local/bin/noticenterctl' ]"));
    assert!(script.contains("readlink -- '/home/user/.local/bin/noticenterctl'"));
    assert!(script.contains("= '/tmp/target/debug/noticenterctl'"));
    assert!(script.contains("rm -f -- '/home/user/.local/bin/noticenterctl'"));
}

#[test]
fn shell_quote_preserves_spaces_and_embedded_single_quotes() {
    let quoted = shell_quote("/tmp/unix notis/it's fine");

    // POSIX single-quote escaping closes, emits a quoted single quote, then reopens
    assert_eq!(quoted, "'/tmp/unix notis/it'\"'\"'s fine'");
}

#[test]
fn trial_launch_ignores_shell_from_inherited_path() {
    let _lock = crate::tests::env::test_env_lock();
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
