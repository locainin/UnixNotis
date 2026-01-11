//! Notification sound playback and backend selection.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use tokio::process::Command;
use tokio::sync::Semaphore;
use tokio::time::timeout;
use tracing::{debug, info, warn};
use unixnotis_core::{program_in_path, util, Config};
use zbus::zvariant::OwnedValue;

/// Sound handling for notification playback.
pub struct SoundSettings {
    enabled: bool,
    backend: SoundBackend,
    default_name: Option<String>,
    default_file: Option<PathBuf>,
    last_played: Mutex<Option<Instant>>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum SoundBackend {
    Canberra,
    PwPlay,
    PaPlay,
    None,
}

#[derive(Debug, Clone)]
enum SoundSource {
    Name(String),
    File(PathBuf),
}

impl SoundSettings {
    /// Build sound settings from configuration and resolve any custom paths.
    pub fn from_config(config: &Config) -> Self {
        let backend = detect_backend();
        debug!(?backend, "sound backend selected");
        if config.sound.enabled && backend == SoundBackend::None {
            warn!("sound enabled but no playback backend found in PATH");
        }

        let default_file = resolve_default_file(config);
        Self {
            enabled: config.sound.enabled,
            backend,
            default_name: config.sound.default_name.clone(),
            default_file,
            last_played: Mutex::new(None),
        }
    }

    /// Return true when sound playback is enabled and a backend is available.
    pub fn supports_sound(&self) -> bool {
        self.enabled && self.backend != SoundBackend::None
    }

    /// Resolve a sound source from hints or defaults and play if allowed.
    pub fn play_from_hints(&self, hints: &HashMap<String, OwnedValue>, allow_sound: bool) {
        if !self.enabled || !allow_sound {
            return;
        }
        if hint_bool(hints, "suppress-sound").unwrap_or(false) {
            return;
        }
        if !self.should_play_now() {
            return;
        }

        let source = resolve_hint_sound(hints).or_else(|| self.default_source());
        if let Some(source) = source {
            self.play(source);
        }
    }

    fn default_source(&self) -> Option<SoundSource> {
        if let Some(path) = self.default_file.as_ref() {
            return Some(SoundSource::File(path.clone()));
        }
        self.default_name
            .as_ref()
            .map(|name| SoundSource::Name(name.clone()))
    }

    fn play(&self, source: SoundSource) {
        match self.backend {
            SoundBackend::Canberra => play_with_canberra(source),
            SoundBackend::PwPlay => play_with_pw_play(source),
            SoundBackend::PaPlay => play_with_paplay(source),
            SoundBackend::None => {}
        }
    }

    fn should_play_now(&self) -> bool {
        const MIN_INTERVAL: Duration = Duration::from_millis(150);
        let Ok(mut guard) = self.last_played.lock() else {
            return true;
        };
        let now = Instant::now();
        if let Some(last) = *guard {
            if now.duration_since(last) < MIN_INTERVAL {
                return false;
            }
        }
        *guard = Some(now);
        true
    }
}

fn resolve_hint_sound(hints: &HashMap<String, OwnedValue>) -> Option<SoundSource> {
    if let Some(file) = hint_string(hints, "sound-file") {
        return Some(SoundSource::File(resolve_sound_file(&file)));
    }
    if let Some(name) = hint_string(hints, "sound-name") {
        return Some(SoundSource::Name(name));
    }
    None
}

fn resolve_sound_file(value: &str) -> PathBuf {
    let trimmed = value.trim();
    // Prefer decoded file:// URIs for correctness; fall back to raw path strings.
    if let Some(decoded) = decode_file_uri(trimmed) {
        return decoded;
    }
    PathBuf::from(trimmed)
}

fn decode_file_uri(value: &str) -> Option<PathBuf> {
    // Strictly parse file:// URIs to avoid remote hosts and malformed paths.
    let stripped = value.strip_prefix("file://")?;
    let (host, path) = match stripped.split_once('/') {
        Some((host, path)) => (host, format!("/{}", path)),
        None => ("", stripped.to_string()),
    };
    // Only accept local file URIs (empty host or localhost).
    if !host.is_empty() && host != "localhost" {
        return None;
    }
    let decoded = percent_decode_path(&path)?;
    if !decoded.starts_with('/') {
        return None;
    }
    Some(PathBuf::from(decoded))
}

fn percent_decode_path(value: &str) -> Option<String> {
    // Decode percent-escaped bytes; reject malformed sequences and NULs.
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0usize;
    while index < bytes.len() {
        match bytes[index] {
            b'%' => {
                let hi = *bytes.get(index + 1)?;
                let lo = *bytes.get(index + 2)?;
                let hi = char::from(hi).to_digit(16)?;
                let lo = char::from(lo).to_digit(16)?;
                let value = ((hi << 4) | lo) as u8;
                if value == 0 {
                    return None;
                }
                out.push(value);
                index += 3;
            }
            byte => {
                out.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8(out).ok()
}

fn resolve_default_file(config: &Config) -> Option<PathBuf> {
    if let Some(path) = config.sound.default_file.as_ref() {
        return resolve_config_path(path).or_else(|| Some(PathBuf::from(path)));
    }
    if let Some(dir) = config.sound.default_dir.as_ref() {
        if let Some(path) = resolve_config_path(dir).or_else(|| Some(PathBuf::from(dir))) {
            return choose_first_sound_file(&path);
        }
    }
    None
}

fn resolve_config_path(value: &str) -> Option<PathBuf> {
    let path = PathBuf::from(value);
    if path.is_absolute() {
        return Some(path);
    }
    let base = Config::default_config_dir().ok()?;
    Some(base.join(path))
}

fn choose_first_sound_file(dir: &Path) -> Option<PathBuf> {
    let entries = fs::read_dir(dir).ok()?;
    let mut candidates = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() && has_audio_extension(&path) {
            candidates.push(path);
        }
    }
    candidates.sort();
    let selected = candidates.into_iter().next();
    if let Some(path) = selected.as_ref() {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("sound file");
        info!(name, "using default notification sound file");
    }
    selected
}

fn has_audio_extension(path: &Path) -> bool {
    let Some(ext) = path.extension().and_then(|ext| ext.to_str()) else {
        return false;
    };
    matches!(
        ext.to_ascii_lowercase().as_str(),
        "wav" | "ogg" | "oga" | "mp3" | "flac" | "m4a" | "aac"
    )
}

fn hint_string(hints: &HashMap<String, OwnedValue>, key: &str) -> Option<String> {
    hints
        .get(key)
        .and_then(|value| value.try_clone().ok())
        .and_then(|owned| String::try_from(owned).ok())
}

fn hint_bool(hints: &HashMap<String, OwnedValue>, key: &str) -> Option<bool> {
    hints.get(key).and_then(|value| bool::try_from(value).ok())
}

fn detect_backend() -> SoundBackend {
    if program_in_path("canberra-gtk-play") {
        return SoundBackend::Canberra;
    }
    if program_in_path("pw-play") {
        return SoundBackend::PwPlay;
    }
    if program_in_path("paplay") {
        return SoundBackend::PaPlay;
    }
    SoundBackend::None
}

const SOUND_COMMAND_TIMEOUT: Duration = Duration::from_secs(3);
const SOUND_MAX_CONCURRENT: usize = 2;

fn sound_semaphore() -> &'static Arc<Semaphore> {
    static SEMAPHORE: OnceLock<Arc<Semaphore>> = OnceLock::new();
    SEMAPHORE.get_or_init(|| Arc::new(Semaphore::new(SOUND_MAX_CONCURRENT)))
}

fn spawn_sound_command(backend: &'static str, program: &str, args: &[String]) {
    let limiter = sound_semaphore().clone();
    let permit = match limiter.try_acquire_owned() {
        Ok(permit) => permit,
        Err(_) => {
            debug!(backend, "sound command skipped (concurrency limit reached)");
            return;
        }
    };
    let command_str = if args.is_empty() {
        program.to_string()
    } else {
        format!("{program} {}", args.join(" "))
    };
    let command_snip = util::log_snippet(&command_str);
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);
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

async fn reap_sound_child(
    backend: &'static str,
    command_snip: String,
    pid: Option<u32>,
    mut child: tokio::process::Child,
) {
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

fn play_with_canberra(source: SoundSource) {
    let mut args = Vec::new();
    match source {
        SoundSource::Name(name) => {
            args.push("-i".to_string());
            args.push(name);
        }
        SoundSource::File(path) => {
            args.push("-f".to_string());
            args.push(path.to_string_lossy().to_string());
        }
    }
    spawn_sound_command("canberra", "canberra-gtk-play", &args);
}

fn play_with_pw_play(source: SoundSource) {
    let SoundSource::File(path) = source else {
        warn!("pw-play backend does not support sound-name hints");
        return;
    };
    let args = vec![path.to_string_lossy().to_string()];
    spawn_sound_command("pw-play", "pw-play", &args);
}

fn play_with_paplay(source: SoundSource) {
    let SoundSource::File(path) = source else {
        warn!("paplay backend does not support sound-name hints");
        return;
    };
    let args = vec![path.to_string_lossy().to_string()];
    spawn_sound_command("paplay", "paplay", &args);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn reaps_short_lived_command() {
        let mut command = Command::new("true");
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let child = command.spawn().expect("spawn true");
        reap_sound_child("test", "true".to_string(), child.id(), child).await;
    }

    #[test]
    fn decode_file_uri_accepts_localhost() {
        // Uses $HOME to avoid hard-coded absolute paths in test data.
        let Ok(home) = std::env::var("HOME") else {
            return;
        };
        if home.is_empty() {
            return;
        }
        let uri = format!("file://localhost{home}/sound%20file.ogg");
        let expected = PathBuf::from(format!("{home}/sound file.ogg"));
        assert_eq!(decode_file_uri(&uri), Some(expected));
    }

    #[test]
    fn decode_file_uri_rejects_remote_hosts() {
        // Remote file URI hosts should be rejected for local playback.
        let Ok(home) = std::env::var("HOME") else {
            return;
        };
        if home.is_empty() {
            return;
        }
        let uri = format!("file://example.com{home}/sound.ogg");
        assert!(decode_file_uri(&uri).is_none());
    }

    #[test]
    fn percent_decode_path_rejects_nul() {
        // NUL bytes should never appear in decoded filesystem paths.
        assert!(percent_decode_path("/%00.wav").is_none());
    }
}
