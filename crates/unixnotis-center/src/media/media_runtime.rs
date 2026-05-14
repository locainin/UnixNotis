use tokio::sync::mpsc;
use unixnotis_core::MediaConfig;
use zbus::Connection;

use crate::dbus::UiEvent;

use super::media_loop::run_media_loop;
use super::MediaHandle;

pub(super) const MEDIA_COMMAND_CAPACITY: usize = 32;
pub(super) const MEDIA_SIGNAL_CAPACITY: usize = 256;

pub(super) fn start_media_task(
    runtime: &tokio::runtime::Handle,
    connection: Connection,
    config: MediaConfig,
    sender: async_channel::Sender<UiEvent>,
) -> Option<MediaHandle> {
    if !config.enabled {
        // Disabled media means no background work and no command channel
        return None;
    }

    // Lowercase tokens once so the hot path can stay allocation-free
    let config = normalize_media_config(config);
    // The command channel stays small because button presses arrive in short bursts
    let (command_tx, command_rx) = mpsc::channel(MEDIA_COMMAND_CAPACITY);
    // The runtime task owns player state and feeds snapshots back to the UI
    runtime.spawn(run_media_loop(connection, config, sender, command_rx));

    Some(MediaHandle {
        command_tx: Some(command_tx),
        runtime: runtime.clone(),
    })
}

fn normalize_media_config(mut config: MediaConfig) -> MediaConfig {
    // Lowercase these token lists once so the hot path can use plain contains checks
    config.allowlist = config
        .allowlist
        .into_iter()
        .map(|entry| entry.to_lowercase())
        .collect();
    // Browser family matching uses the same lowercase path
    config.browser_tokens = config
        .browser_tokens
        .into_iter()
        .map(|entry| entry.to_lowercase())
        .collect();
    // Denylist entries follow the same normalized form
    config.denylist = config
        .denylist
        .into_iter()
        .map(|entry| entry.to_lowercase())
        .collect();
    config
}

#[cfg(test)]
#[path = "tests/runtime.rs"]
mod tests;
