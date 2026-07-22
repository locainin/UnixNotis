//! Notification sound playback and backend selection

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tracing::{debug, warn};
use unixnotis_core::Config;
use zbus::zvariant::OwnedValue;

use super::backend::{detect_backend, SoundBackend};
use super::command::{play_with_canberra, play_with_paplay, play_with_pw_play};
use super::resolve::{
    hint_bool, resolve_allowed_file_hint_dirs, resolve_config_dir, resolve_default_file,
    resolve_hint_sound,
};
use super::SoundSource;

const SOUND_MIN_INTERVAL: Duration = Duration::from_millis(150);

/// Sound handling for notification playback
pub struct SoundSettings {
    // Global on/off from config
    enabled: bool,
    // Detected backend that is safe to call on this machine
    backend: SoundBackend,
    // File hints are an explicit compatibility opt-in
    allow_file_hints: bool,
    // Every accepted file hint must remain beneath one configured directory
    allowed_file_hint_dirs: Vec<PathBuf>,
    // Fallback event name used by canberra-style backends
    default_name: Option<String>,
    // Fallback audio file path when hint does not supply one
    default_file: Option<super::SoundFile>,
    // Last successful play request used for burst throttling
    last_played: Mutex<Option<Instant>>,
}

impl SoundSettings {
    /// Build sound settings from configuration and resolve any custom paths
    pub fn from_config(config: &Config, config_path: Option<&Path>) -> Self {
        // Backend discovery is done once during startup to avoid repeated trusted-path scans
        let backend = detect_backend();
        debug!(?backend, "sound backend selected");
        if Self::should_warn_missing_backend(config.sound.enabled, backend) {
            warn!("sound enabled but no playback backend found in PATH");
        }

        // Resolve config paths once so notification hot paths stay cheap
        let config_dir = resolve_config_dir(config_path);
        let default_file = resolve_default_file(config, config_dir.as_deref());
        let allowed_file_hint_dirs = resolve_allowed_file_hint_dirs(config, config_dir.as_deref());
        Self {
            enabled: config.sound.enabled,
            backend,
            allow_file_hints: config.sound.allow_file_hints,
            allowed_file_hint_dirs,
            default_name: config.sound.default_name.clone(),
            default_file,
            last_played: Mutex::new(None),
        }
    }

    /// Return true when sound playback is enabled and a backend is available
    pub fn supports_sound(&self) -> bool {
        self.enabled && self.backend != SoundBackend::None
    }

    /// Resolve a sound source from hints or defaults and play if allowed
    pub fn play_from_hints(&self, hints: &HashMap<String, OwnedValue>, allow_sound: bool) -> bool {
        // Hard gates first to keep the common no-sound path fast
        if !self.enabled || !allow_sound {
            return false;
        }
        // Per-notification hint can force silence even when daemon sound is enabled
        if hint_bool(hints, "suppress-sound").unwrap_or(false) {
            return false;
        }
        // Small cooldown avoids noisy bursts when apps spam fast updates
        if !self.should_play_now() {
            return false;
        }

        // Hint source wins, then fallback source from config
        let source = resolve_hint_sound(hints, self.allow_file_hints, &self.allowed_file_hint_dirs)
            .or_else(|| self.default_source());
        if let Some(source) = source {
            return self.play(source);
        }
        false
    }

    fn should_warn_missing_backend(sound_enabled: bool, backend: SoundBackend) -> bool {
        sound_enabled && backend == SoundBackend::None
    }

    fn default_source(&self) -> Option<SoundSource> {
        if let Some(path) = self.default_file.as_ref() {
            // File fallback is tried before event-name fallback
            return Some(SoundSource::File(path.clone()));
        }
        self.default_name
            .as_ref()
            .map(|name| SoundSource::Name(name.clone()))
    }

    fn play(&self, source: SoundSource) -> bool {
        // Backend-specific launcher keeps this method tiny and testable
        match self.backend {
            SoundBackend::Canberra => {
                play_with_canberra(source);
                true
            }
            SoundBackend::PwPlay => {
                play_with_pw_play(source);
                true
            }
            SoundBackend::PaPlay => {
                play_with_paplay(source);
                true
            }
            SoundBackend::None => false,
        }
    }

    fn should_play_now(&self) -> bool {
        self.should_play_at(Instant::now())
    }

    fn should_play_at(&self, now: Instant) -> bool {
        let Ok(mut guard) = self.last_played.lock() else {
            // A poisoned lock should not disable alerts forever
            return true;
        };
        if let Some(last) = *guard {
            // Skip playback if requests are too close together
            if now.duration_since(last) < SOUND_MIN_INTERVAL {
                return false;
            }
        }
        // Record now only when the request is accepted
        *guard = Some(now);
        true
    }
}

#[cfg(test)]
#[path = "tests/settings.rs"]
mod tests;
