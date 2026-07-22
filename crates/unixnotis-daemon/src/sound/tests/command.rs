use super::*;
use crate::system_tools::routing::use_fake_tool_bin;
use crate::test_support::TempRoot;

fn fake_sound_tool(name: &str) -> (TempRoot, crate::system_tools::routing::FakeToolBinGuard) {
    use std::os::unix::fs::PermissionsExt;

    let root = TempRoot::new("sound-command");
    let path = root.join(name);
    std::fs::write(&path, "#!/bin/sh\nexit 0\n").expect("write fake sound tool");
    let mut permissions = std::fs::metadata(&path)
        .expect("fake sound tool metadata")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&path, permissions).expect("make fake sound tool executable");
    let guard = use_fake_tool_bin(root.path());
    (root, guard)
}

#[cfg(unix)]
#[test]
fn sound_command_preserves_non_utf8_argument_bytes() {
    use std::os::unix::ffi::OsStringExt;

    let (_root, _tools) = fake_sound_tool("sound-player");
    let path = OsString::from_vec(b"/tmp/sound-\xff.ogg".to_vec());
    let command = build_sound_command("sound-player", std::slice::from_ref(&path))
        .expect("build trusted sound command");
    let args = command.as_std().get_args().collect::<Vec<_>>();
    let display = sound_command_display("sound-player", std::slice::from_ref(&path));

    assert_eq!(args, vec![path.as_os_str()]);
    assert_eq!(display, "sound-player /tmp/sound-�.ogg");
    assert!(command
        .as_std()
        .get_envs()
        .any(|(name, value)| name == "PATH" && value.is_some()));
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn reaps_short_lived_command() {
    // Smoke coverage keeps the normal successful child exit on the reap path
    let mut command = Command::new("true");
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let child = command.spawn().expect("spawn true");
    reap_sound_child("test", "true".to_string(), child.id(), child).await;
}
