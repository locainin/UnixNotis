//! Media snapshots and commands shared between the runtime and UI

use crate::media::art::MediaArtSource;

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

#[derive(Debug, Clone)]
pub enum MediaCommand {
    Refresh,
    PlayPause { bus_name: String },
    Next { bus_name: String },
    Previous { bus_name: String },
}
