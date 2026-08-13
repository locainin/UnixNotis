use super::*;
use crate::system_tools::routing::use_fake_tool_bin;
use crate::test_support::TempRoot;

fn install_fake_sound_tool(root: &TempRoot, name: &str) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;

    let path = root.join(name);
    std::fs::write(&path, "#!/bin/sh\n: > \"$0.called\"\n").expect("write fake sound tool");
    let mut permissions = std::fs::metadata(&path)
        .expect("fake sound tool metadata")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&path, permissions).expect("make fake sound tool executable");
    path
}

fn fake_sound_tool(name: &str) -> (TempRoot, crate::system_tools::routing::FakeToolBinGuard) {
    let root = TempRoot::new("sound-command");
    install_fake_sound_tool(&root, name);
    let guard = use_fake_tool_bin(root.path());
    (root, guard)
}

async fn launch_until_marker(path: &std::path::Path, mut launch: impl FnMut()) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while !path.exists() {
            // Other sound tests can briefly occupy the process-wide playback permits
            launch();
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("sound backend should create its marker");
    // The marker is written immediately before exit, so allow the reaper to release its permit
    tokio::time::sleep(Duration::from_millis(20)).await;
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

#[cfg(target_os = "linux")]
#[tokio::test(flavor = "current_thread")]
async fn every_sound_backend_launches_its_trusted_tool() {
    let root = TempRoot::new("sound-backends");
    let canberra = install_fake_sound_tool(&root, "canberra-gtk-play");
    let pw_play = install_fake_sound_tool(&root, "pw-play");
    let paplay = install_fake_sound_tool(&root, "paplay");
    let _tools = use_fake_tool_bin(root.path());
    let sound_path = root.join("sound.wav");
    std::fs::write(&sound_path, b"sound fixture").expect("write sound fixture");

    launch_until_marker(&canberra.with_extension("called"), || {
        play_with_canberra(SoundSource::Name("message-new".to_string()));
    })
    .await;

    let file = std::fs::File::open(&sound_path).expect("open sound fixture for pw-play");
    let pw_source = crate::sound::SoundFile::new(sound_path.clone(), file);
    launch_until_marker(&pw_play.with_extension("called"), || {
        play_with_pw_play(SoundSource::File(pw_source.clone()));
    })
    .await;

    let file = std::fs::File::open(&sound_path).expect("open sound fixture for paplay");
    let paplay_source = crate::sound::SoundFile::new(sound_path, file);
    launch_until_marker(&paplay.with_extension("called"), || {
        play_with_paplay(SoundSource::File(paplay_source.clone()));
    })
    .await;
}
