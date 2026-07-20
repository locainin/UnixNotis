use super::*;

#[cfg(unix)]
#[test]
fn sound_command_preserves_non_utf8_argument_bytes() {
    use std::os::unix::ffi::OsStringExt;

    let path = OsString::from_vec(b"/tmp/sound-\xff.ogg".to_vec());
    let command = build_sound_command("true", std::slice::from_ref(&path));
    let args = command.as_std().get_args().collect::<Vec<_>>();

    assert_eq!(args, vec![path.as_os_str()]);
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
