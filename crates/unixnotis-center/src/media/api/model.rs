//! Media snapshots and commands shared between the runtime and UI

use std::path::PathBuf;

use url::Url;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaInfo {
    pub bus_name: String,
    pub identity: String,
    /// Browser family tag used for grouping browser-backed players
    pub browser_family: Option<String>,
    /// Browser or source PID from MPRIS metadata or the owning bus process
    pub owner_pid: Option<u32>,
    pub title: String,
    pub artist: String,
    pub playback_status: String,
    pub art_source: Option<MediaArtSource>,
    pub can_play: bool,
    pub can_pause: bool,
    pub can_next: bool,
    pub can_prev: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaArtSource {
    LocalFile(PathBuf),
    RemoteHttps(Url),
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum MediaArtKey {
    Local(PathBuf),
    Remote(Url),
}

impl MediaArtSource {
    pub(crate) fn stable_key(&self) -> MediaArtKey {
        match self {
            // Native paths retain every platform byte instead of using a display conversion
            Self::LocalFile(path) => MediaArtKey::Local(path.clone()),
            // URL keeps its parsed normalized identity and cannot overlap the local variant
            Self::RemoteHttps(url) => MediaArtKey::Remote(url.clone()),
        }
    }
}

#[derive(Debug, Clone)]
pub enum MediaCommand {
    Refresh,
    PlayPause { bus_name: String },
    Next { bus_name: String },
    Previous { bus_name: String },
}

#[cfg(test)]
#[path = "../tests/model.rs"]
mod tests;
