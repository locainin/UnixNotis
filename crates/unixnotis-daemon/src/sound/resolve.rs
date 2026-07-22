use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use tracing::{debug, info};
use unixnotis_core::filesystem::{open_regular_file, ContainedPath};
use unixnotis_core::{util, Config};
use zbus::zvariant::OwnedValue;

use super::{wav::is_safe_pcm_wav, SoundFile, SoundSource};

const MAX_SOUND_FILE_BYTES: u64 = 16 * 1024 * 1024;

pub(super) fn resolve_hint_sound(
    hints: &HashMap<String, OwnedValue>,
    allow_file_hints: bool,
    allowed_dirs: &[PathBuf],
) -> Option<SoundSource> {
    // File hints cross into host decoders and stay disabled unless explicitly allowed
    if allow_file_hints {
        if let Some(file) = hint_string(hints, "sound-file") {
            let path = resolve_sound_file(&file);
            if path_is_allowed(&path, allowed_dirs) {
                if let Some(file) = open_sound_file(&path, true) {
                    return Some(SoundSource::File(file));
                }
            }
            debug!(path = %path.display(), "ignoring invalid sound-file hint");
        }
    }
    // Fall back to event name when file path is missing or invalid
    if let Some(name) = hint_string(hints, "sound-name") {
        return Some(SoundSource::Name(name));
    }
    None
}

pub(super) fn resolve_default_file(
    config: &Config,
    config_dir: Option<&Path>,
) -> Option<SoundFile> {
    // First choice is an explicit default file
    if let Some(path) = config.sound.default_file.as_ref() {
        let resolved = resolve_config_path(path, config_dir);
        return resolved.and_then(|path| open_sound_file(&path, false));
    }
    // Second choice is scanning a configured directory for the first valid audio file
    if let Some(dir) = config.sound.default_dir.as_ref() {
        if let Some(path) = resolve_config_path(dir, config_dir) {
            return choose_first_sound_file(&path);
        }
    }
    None
}

pub(super) fn resolve_config_dir(config_path: Option<&Path>) -> Option<PathBuf> {
    // An explicit daemon path owns relative assets even when the environment selects another file
    let config_path = config_path
        .map(Path::to_path_buf)
        .or_else(|| Config::active_config_path().ok())?;
    config_path.parent().map(Path::to_path_buf)
}

pub(super) fn resolve_allowed_file_hint_dirs(
    config: &Config,
    config_dir: Option<&Path>,
) -> Vec<PathBuf> {
    config
        .sound
        .allowed_file_hint_dirs
        .iter()
        .filter_map(|path| resolve_config_path(path, config_dir))
        .collect()
}

pub(super) fn hint_bool(hints: &HashMap<String, OwnedValue>, key: &str) -> Option<bool> {
    // Borrowed conversion avoids cloning large values
    hints.get(key).and_then(|value| bool::try_from(value).ok())
}

fn resolve_sound_file(value: &str) -> PathBuf {
    let trimmed = value.trim();
    // Decode well-formed file:// URIs first, then fall back to plain filesystem paths
    if let Some(decoded) = decode_file_uri(trimmed) {
        return decoded;
    }
    PathBuf::from(trimmed)
}

fn decode_file_uri(value: &str) -> Option<PathBuf> {
    // Only local file URIs are accepted to avoid accidental remote sources
    let stripped = value.strip_prefix("file://")?;
    let (host, path) = match stripped.split_once('/') {
        Some((host, path)) => (host, format!("/{path}")),
        None => ("", stripped.to_string()),
    };
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
    // Invalid escape sequences and NUL bytes are rejected here
    let mut bytes = value.as_bytes().iter().copied();
    let mut out = Vec::with_capacity(value.len());
    while let Some(byte) = bytes.next() {
        match byte {
            b'%' => {
                let hi = bytes.next()?;
                let lo = bytes.next()?;
                let hi = char::from(hi).to_digit(16)?;
                let lo = char::from(lo).to_digit(16)?;
                let value = (hi * 16 + lo) as u8;
                if value == 0 {
                    return None;
                }
                out.push(value);
            }
            byte => {
                out.push(byte);
            }
        }
    }
    String::from_utf8(out).ok()
}

fn resolve_config_path(value: &str, config_dir: Option<&Path>) -> Option<PathBuf> {
    // Expand "~" so config remains short and portable
    let path = util::expand_tilde(value);
    let path = PathBuf::from(path.as_ref());
    if path.is_absolute() {
        return Some(path);
    }
    let base = config_dir?;
    Some(base.join(path))
}

fn choose_first_sound_file(dir: &Path) -> Option<SoundFile> {
    // Missing directory is treated as no default instead of an error path
    let entries = fs::read_dir(dir).ok()?;
    let mut candidates = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        // Keep only normal files with supported extensions
        if path.is_file() && has_audio_extension(&path) {
            candidates.push(path);
        }
    }
    // Deterministic ordering keeps startup behavior stable between runs
    candidates.sort();
    for path in candidates {
        let Some(selected) = open_sound_file(&path, false) else {
            continue;
        };
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("sound file");
        info!(name, "using default notification sound file");
        return Some(selected);
    }
    None
}

fn has_audio_extension(path: &Path) -> bool {
    // Extension gate is cheap and avoids probing unsupported files
    let Some(ext) = path.extension().and_then(|ext| ext.to_str()) else {
        return false;
    };
    matches!(
        ext.to_ascii_lowercase().as_str(),
        "wav" | "ogg" | "oga" | "mp3" | "flac" | "m4a" | "aac"
    )
}

fn open_sound_file(path: &Path, require_safe_hint_format: bool) -> Option<SoundFile> {
    if !has_audio_extension(path) {
        return None;
    }
    // One descriptor binds all checks and later playback to the same regular file
    let file = open_regular_file(path).ok()?;
    let metadata = file.metadata().ok()?;
    if metadata.len() > MAX_SOUND_FILE_BYTES {
        return None;
    }
    if require_safe_hint_format && !has_safe_hint_format(path, &file, metadata.len()) {
        return None;
    }
    Some(SoundFile::new(path.to_path_buf(), file))
}

fn path_is_allowed(path: &Path, allowed_dirs: &[PathBuf]) -> bool {
    path.is_absolute()
        && allowed_dirs
            .iter()
            .any(|root| ContainedPath::resolve(root, path).is_ok())
}

fn has_safe_hint_format(path: &Path, file: &fs::File, file_len: u64) -> bool {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default();
    extension.eq_ignore_ascii_case("wav") && is_safe_pcm_wav(file, file_len)
}

fn hint_string(hints: &HashMap<String, OwnedValue>, key: &str) -> Option<String> {
    // Clone only the selected hint value so unrelated hint payload is untouched
    hints
        .get(key)
        .and_then(|value| value.try_clone().ok())
        .and_then(|owned| String::try_from(owned).ok())
}

#[cfg(test)]
#[path = "tests/resolve.rs"]
mod tests;
