//! Notification sound playback and backend selection.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tracing::{info, warn};
use unixnotis_core::Config;
use zbus::zvariant::OwnedValue;

use std::collections::HashMap;

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
    if let Some(path) = trimmed.strip_prefix("file://") {
        PathBuf::from(path)
    } else {
        PathBuf::from(trimmed)
    }
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

fn program_in_path(program: &str) -> bool {
    if program.contains(std::path::MAIN_SEPARATOR) {
        return Path::new(program).is_file();
    }
    let Ok(paths) = env::var("PATH") else {
        return false;
    };
    for dir in env::split_paths(&paths) {
        if dir.join(program).is_file() {
            return true;
        }
    }
    false
}

fn play_with_canberra(source: SoundSource) {
    let mut command = Command::new("canberra-gtk-play");
    match source {
        SoundSource::Name(name) => {
            command.arg("-i").arg(name);
        }
        SoundSource::File(path) => {
            command.arg("-f").arg(path);
        }
    }
    if let Err(err) = command.spawn() {
        warn!(
            ?err,
            "failed to play notification sound with canberra-gtk-play"
        );
    }
}

fn play_with_pw_play(source: SoundSource) {
    let SoundSource::File(path) = source else {
        warn!("pw-play backend does not support sound-name hints");
        return;
    };
    if let Err(err) = Command::new("pw-play").arg(path).spawn() {
        warn!(?err, "failed to play notification sound with pw-play");
    }
}

fn play_with_paplay(source: SoundSource) {
    let SoundSource::File(path) = source else {
        warn!("paplay backend does not support sound-name hints");
        return;
    };
    if let Err(err) = Command::new("paplay").arg(path).spawn() {
        warn!(?err, "failed to play notification sound with paplay");
    }
}
