//! Nonblocking media controls held by GTK widgets

use tokio::sync::mpsc;
use unixnotis_core::MediaConfig;

use crate::control::UiEvent;

use super::MediaCommand;

#[derive(Clone)]
pub struct MediaHandle {
    command_tx: mpsc::Sender<MediaCommand>,
    // Overflow work uses the shared runtime so GTK callbacks never block
    runtime: tokio::runtime::Handle,
}

impl MediaHandle {
    pub const fn connected(
        command_tx: mpsc::Sender<MediaCommand>,
        runtime: tokio::runtime::Handle,
    ) -> Self {
        Self {
            command_tx,
            runtime,
        }
    }

    pub fn refresh(&self) {
        // Refresh follows the same bounded delivery path as player controls
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
        match self.command_tx.try_send(command) {
            Ok(()) => {}
            Err(mpsc::error::TrySendError::Full(command)) => {
                // A bounded async retry preserves input responsiveness during short bursts
                let tx = self.command_tx.clone();
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
    config: MediaConfig,
    sender: async_channel::Sender<UiEvent>,
) -> Option<MediaHandle> {
    // Runtime ownership remains outside GTK so shutdown can retire the task cleanly
    super::super::runtime::start_media_task(runtime, config, sender)
}

#[cfg(test)]
#[path = "../tests/handle.rs"]
mod tests;
