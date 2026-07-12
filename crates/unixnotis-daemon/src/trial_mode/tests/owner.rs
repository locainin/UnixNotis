use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::system_tools::use_fake_tool_bin;

use super::{is_unit_active, pgrep_exact, read_args, read_comm};

struct TempDirGuard {
    path: std::path::PathBuf,
}

impl TempDirGuard {
    fn new(label: &str) -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock moved backwards")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "unixnotis-trial-owner-{label}-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create temp dir");
        Self { path }
    }

    fn write_executable(&self, name: &str, contents: &str) {
        let path = self.path.join(name);
        fs::write(&path, contents).expect("write fake tool");
        let mut permissions = fs::metadata(&path)
            .expect("fake tool metadata")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).expect("chmod fake tool");
    }
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[tokio::test]
async fn is_unit_active_uses_trusted_systemctl_exit_status() {
    let root = TempDirGuard::new("systemctl-active");
    root.write_executable(
        "systemctl",
        "#!/bin/sh\ncase \"$*\" in *mako.service*) exit 0;; *) exit 3;; esac\n",
    );
    let _tools = use_fake_tool_bin(&root.path);

    assert!(is_unit_active("mako.service").await);
    assert!(!is_unit_active("dunst.service").await);
}

#[tokio::test]
async fn pgrep_exact_parses_only_numeric_pids() {
    let root = TempDirGuard::new("pgrep");
    root.write_executable("pgrep", "#!/bin/sh\nprintf '12\\nnot-a-pid\\n34\\n'\n");
    let _tools = use_fake_tool_bin(&root.path);

    let pids = pgrep_exact("mako").await;

    assert_eq!(pids, [12, 34]);
}

#[tokio::test]
async fn read_comm_uses_trusted_ps_fallback_when_procfs_is_missing() {
    let root = TempDirGuard::new("comm");
    root.write_executable("ps", "#!/bin/sh\nprintf 'mako\\n'\n");
    let _tools = use_fake_tool_bin(&root.path);

    let comm = read_comm(u32::MAX).await;

    assert_eq!(comm.as_deref(), Some("mako"));
}

#[tokio::test]
async fn read_args_uses_trusted_ps_fallback_when_procfs_is_missing() {
    let root = TempDirGuard::new("args");
    root.write_executable(
        "ps",
        "#!/bin/sh\nprintf '/usr/bin/mako --config mako.conf\\n'\n",
    );
    let _tools = use_fake_tool_bin(&root.path);

    let args = read_args(u32::MAX).await.expect("fallback args");

    assert_eq!(args, ["/usr/bin/mako", "--config", "mako.conf"]);
}
