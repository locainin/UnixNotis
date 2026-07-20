use std::ffi::OsString;
use std::process::Stdio;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use tokio::process::Command;
use tokio::sync::Semaphore;
use tokio::time::timeout;
use tracing::{debug, warn};
use unixnotis_core::util;

use super::SoundSource;

const SOUND_COMMAND_TIMEOUT: Duration = Duration::from_secs(3);
// Small cap prevents unbounded process fanout during notification bursts
const SOUND_MAX_CONCURRENT: usize = 2;

pub(super) fn play_with_canberra(source: SoundSource) {
    // canberra supports both symbolic names and direct files
    let mut args = Vec::new();
    match source {
        SoundSource::Name(name) => {
            args.push(OsString::from("-i"));
            args.push(OsString::from(name));
        }
        SoundSource::File(path) => {
            args.push(OsString::from("-f"));
            args.push(path.into_os_string());
        }
    }
    spawn_sound_command("canberra", "canberra-gtk-play", &args);
}

pub(super) fn play_with_pw_play(source: SoundSource) {
    // pw-play accepts only direct file playback
    let SoundSource::File(path) = source else {
        warn!("pw-play backend does not support sound-name hints");
        return;
    };
    let args = vec![path.into_os_string()];
    spawn_sound_command("pw-play", "pw-play", &args);
}

pub(super) fn play_with_paplay(source: SoundSource) {
    // paplay accepts only direct file playback
    let SoundSource::File(path) = source else {
        warn!("paplay backend does not support sound-name hints");
        return;
    };
    let args = vec![path.into_os_string()];
    spawn_sound_command("paplay", "paplay", &args);
}

fn sound_semaphore() -> &'static Arc<Semaphore> {
    static SEMAPHORE: OnceLock<Arc<Semaphore>> = OnceLock::new();
    // Process-wide limiter shared by all sound playback requests
    SEMAPHORE.get_or_init(|| Arc::new(Semaphore::new(SOUND_MAX_CONCURRENT)))
}

fn spawn_sound_command(backend: &'static str, program: &str, args: &[OsString]) {
    let limiter = sound_semaphore().clone();
    // try_acquire keeps this call non-blocking on hot paths
    let permit = if let Ok(permit) = limiter.try_acquire_owned() {
        permit
    } else {
        debug!(backend, "sound command skipped (concurrency limit reached)");
        return;
    };
    let command_str = sound_command_display(program, args);
    let command_snip = util::log_snippet(&command_str);
    let mut command = build_sound_command(program, args);
    match command.spawn() {
        Ok(child) => {
            let pid = child.id();
            debug!(
                backend,
                pid,
                command = %command_snip,
                "sound command spawned"
            );
            tokio::spawn(async move {
                // Keep the permit owned until this child exits or gets killed
                let _permit = permit;
                reap_sound_child(backend, command_snip, pid, child).await;
            });
        }
        Err(err) => {
            warn!(
                backend,
                command = %command_snip,
                ?err,
                "failed to spawn sound command"
            );
        }
    }
}

fn build_sound_command(program: &str, args: &[OsString]) -> Command {
    let mut command = Command::new(program);
    command
        // OsString keeps every valid Unix path byte intact
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        // Dropped tasks must not leave playback children behind
        .kill_on_drop(true);
    command
}

fn sound_command_display(program: &str, args: &[OsString]) -> String {
    let mut display = program.to_string();
    for argument in args {
        // Lossy text is restricted to bounded diagnostics, never execution
        display.push(' ');
        display.push_str(&argument.to_string_lossy());
    }
    display
}

async fn reap_sound_child(
    backend: &'static str,
    command_snip: String,
    pid: Option<u32>,
    mut child: tokio::process::Child,
) {
    // Duration in logs helps distinguish slow backend issues from spawn issues
    let started = Instant::now();
    match timeout(SOUND_COMMAND_TIMEOUT, child.wait()).await {
        Ok(Ok(status)) => {
            let elapsed_ms = started.elapsed().as_millis();
            if status.success() {
                debug!(
                    backend,
                    pid,
                    command = %command_snip,
                    status = ?status.code(),
                    elapsed_ms,
                    "sound command completed"
                );
            } else {
                warn!(
                    backend,
                    pid,
                    command = %command_snip,
                    status = ?status.code(),
                    elapsed_ms,
                    "sound command exited with error"
                );
            }
        }
        Ok(Err(err)) => {
            warn!(
                backend,
                pid,
                command = %command_snip,
                ?err,
                "sound command wait failed"
            );
        }
        Err(_) => {
            warn!(
                backend,
                pid,
                command = %command_snip,
                "sound command timed out"
            );
            // Timeout path sends kill and then waits to avoid zombie children
            if let Err(err) = child.kill().await {
                warn!(
                    backend,
                    pid,
                    command = %command_snip,
                    ?err,
                    "sound command kill failed"
                );
            }
            let _ = child.wait().await;
        }
    }
}

#[cfg(test)]
#[path = "tests/command.rs"]
mod tests;
