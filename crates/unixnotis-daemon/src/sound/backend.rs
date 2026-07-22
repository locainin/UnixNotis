use crate::system_tools;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(super) enum SoundBackend {
    // Event-name and file playback via libcanberra helper
    Canberra,
    // PipeWire file playback fallback
    PwPlay,
    // PulseAudio file playback fallback
    PaPlay,
    // No supported playback command found
    None,
}

pub(super) fn detect_backend() -> SoundBackend {
    // Prefer canberra first because it supports both sound names and files
    if system_tools::program_path("canberra-gtk-play").is_ok() {
        return SoundBackend::Canberra;
    }
    if system_tools::program_path("pw-play").is_ok() {
        return SoundBackend::PwPlay;
    }
    if system_tools::program_path("paplay").is_ok() {
        return SoundBackend::PaPlay;
    }
    SoundBackend::None
}
