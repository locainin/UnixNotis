mod bus;
mod cache;
mod event_loop;
mod events;
mod metadata;
mod policy;
mod runtime;
mod schedule;
mod snapshot;

use std::path::PathBuf;

use tokio::sync::mpsc;
use unixnotis_core::MediaConfig;
use url::Url;
use zbus::Connection;

use crate::dbus::UiEvent;

// MPRIS base identifiers used to discover players on the session bus
pub(super) const MPRIS_PREFIX: &str = "org.mpris.MediaPlayer2.";
pub(super) const MPRIS_PATH: &str = "/org/mpris/MediaPlayer2";
pub(super) const MPRIS_PLAYER: &str = "org.mpris.MediaPlayer2.Player";
pub(super) const MPRIS_APP: &str = "org.mpris.MediaPlayer2";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaInfo {
    pub bus_name: String,
    pub identity: String,
    /// Browser family tag used for grouping browser-backed players
    pub browser_family: Option<String>,
    /// Browser/source PID from MPRIS metadata or the owning bus process
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
            // Paths become a stable key so reloads can spot real art changes
            Self::LocalFile(path) => format!("file:{}", path.to_string_lossy()),
            // Remote urls are already normalized during parsing
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MediaRefreshOrigin {
    // Native player/property traffic can justify one bounded fallback sweep
    Bus,
    // Synthetic retries must never re-arm themselves or they become a poll loop
    Fallback,
}

#[derive(Debug)]
enum MediaSignal {
    PropertiesChanged {
        bus_name: String,
        origin: MediaRefreshOrigin,
    },
}

#[derive(Clone)]
pub struct MediaHandle {
    command_tx: Option<mpsc::Sender<MediaCommand>>,
    // The shared runtime lets overflow work finish off the GTK thread
    runtime: tokio::runtime::Handle,
}

impl MediaHandle {
    pub fn refresh(&self) {
        self.send_command(MediaCommand::Refresh);
    }

    pub fn play_pause(&self, bus_name: &str) {
        self.send_command(MediaCommand::PlayPause {
            bus_name: bus_name.to_string(),
        });
    }

    pub fn next(&self, bus_name: &str) {
        self.send_command(MediaCommand::Next {
            bus_name: bus_name.to_string(),
        });
    }

    pub fn previous(&self, bus_name: &str) {
        self.send_command(MediaCommand::Previous {
            bus_name: bus_name.to_string(),
        });
    }

    fn send_command(&self, command: MediaCommand) {
        let Some(tx) = &self.command_tx else {
            return;
        };
        match tx.try_send(command) {
            Ok(()) => {}
            Err(mpsc::error::TrySendError::Full(command)) => {
                // A tiny async retry is cheaper than blocking button handlers
                let tx = tx.clone();
                let runtime = self.runtime.clone();
                runtime.spawn(async move {
                    let _ = tx.send(command).await;
                });
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {}
        }
    }
}

pub fn start_media_task(
    runtime: &tokio::runtime::Handle,
    connection: Connection,
    config: MediaConfig,
    sender: async_channel::Sender<UiEvent>,
) -> Option<MediaHandle> {
    // The heavy runtime loop lives in its own file so this module can stay type-focused
    runtime::start_media_task(runtime, connection, config, sender)
}

#[cfg(test)]
#[path = "tests/types.rs"]
mod tests;
