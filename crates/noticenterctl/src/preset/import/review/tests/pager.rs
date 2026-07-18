use super::super::pager::page_exec_content_review;
use crate::system_tools::routing::use_fake_tool_bin;
use crate::test_support::{test_env_lock, EnvGuard};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

struct TempDirGuard {
    path: PathBuf,
}

impl TempDirGuard {
    fn new(label: &str) -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock moved backwards")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "unixnotis-review-pager-{label}-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create fake tool directory");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[test]
fn pager_uses_trusted_less_without_evaluating_pager_environment() {
    let _lock = test_env_lock();
    let tools = TempDirGuard::new("trusted");
    let capture = tools.path().join("capture");
    let marker = tools.path().join("pager-was-executed");
    let less = tools.path().join("less");
    fs::write(
        &less,
        "#!/bin/sh\nprintf '%s\\n' \"$LESSSECURE|$LESSHISTFILE|$LESS|$LESSOPEN\" > \"$UNIXNOTIS_TEST_PAGER_CAPTURE.env\"\nprintf '%s\\n' \"$@\" > \"$UNIXNOTIS_TEST_PAGER_CAPTURE.args\"\ncat > \"$UNIXNOTIS_TEST_PAGER_CAPTURE\"\n",
    )
    .expect("write fake less");
    fs::set_permissions(&less, fs::Permissions::from_mode(0o755))
        .expect("make fake less executable");

    let _tools = use_fake_tool_bin(tools.path());
    let _capture = EnvGuard::set("UNIXNOTIS_TEST_PAGER_CAPTURE", capture.as_os_str());
    let _pager = EnvGuard::set("PAGER", format!("touch {}", marker.display()));
    let _less = EnvGuard::set("LESS", "-X");
    let _less_open = EnvGuard::set("LESSOPEN", "|sh -c 'echo unsafe'");

    assert!(page_exec_content_review("review text\n").expect("page review"));
    assert_eq!(
        fs::read(&capture).expect("read captured review"),
        b"review text\n"
    );
    assert_eq!(
        fs::read_to_string(capture.with_extension("env")).expect("read captured environment"),
        "1|-||\n"
    );
    assert_eq!(
        fs::read_to_string(capture.with_extension("args")).expect("read captured arguments"),
        "-R\n--\n"
    );
    assert!(!marker.exists());
}

#[test]
fn pager_reports_unavailable_when_less_is_not_installed() {
    let tools = TempDirGuard::new("missing");
    let _tools = use_fake_tool_bin(tools.path());

    assert!(!page_exec_content_review("review\n").expect("fall back without less"));
}
