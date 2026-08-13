//! Media runtime startup and one-time configuration normalization

use tokio::sync::mpsc;
use unixnotis_core::MediaConfig;

use crate::control::UiEvent;

use super::super::api::MediaHandle;

const MEDIA_COMMAND_CAPACITY: usize = 32;

pub(in crate::media) fn start_media_task(
    runtime: &tokio::runtime::Handle,
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
    runtime.spawn(super::r#loop::run_event_loop(config, sender, command_rx));

    Some(MediaHandle::connected(command_tx, runtime.clone()))
}

pub(super) fn normalize_media_config(mut config: MediaConfig) -> MediaConfig {
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
