use super::*;

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
