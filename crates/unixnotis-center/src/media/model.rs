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

impl MediaArtSource {
    pub fn stable_key(&self) -> String {
        match self {
            // Source prefixes prevent a local path from colliding with a remote URL
            Self::LocalFile(path) => format!("file:{}", path.to_string_lossy()),
            // Remote URLs are normalized during policy validation
            Self::RemoteHttps(url) => format!("https:{}", url.as_str()),
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
#[path = "tests/model.rs"]
mod tests;
