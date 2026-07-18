//! Media task startup and runtime orchestration

mod cache;
mod dispatch;
mod r#loop;
mod owner;
mod refresh;
mod schedule;
mod snapshot;
mod state;

use tokio::sync::mpsc;
use unixnotis_core::MediaConfig;

use crate::control::UiEvent;

use super::api::MediaHandle;

pub(super) const MEDIA_COMMAND_CAPACITY: usize = 32;
pub(super) const MEDIA_SIGNAL_CAPACITY: usize = 256;

pub(super) fn start_media_task(
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
    runtime.spawn(r#loop::run_event_loop(config, sender, command_rx));

    Some(MediaHandle::connected(command_tx, runtime.clone()))
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MediaRefreshOrigin {
    // Native bus traffic can justify one bounded fallback sweep
    Bus,
    // Synthetic retries never re-arm themselves because that would become polling
    Fallback,
}

#[derive(Debug)]
pub(super) enum MediaSignal {
    PropertiesChanged {
        bus_name: String,
        origin: MediaRefreshOrigin,
    },
}

#[cfg(test)]
#[path = "../tests/runtime.rs"]
mod tests;

#[cfg(test)]
#[path = "../tests/signals.rs"]
mod signal_tests;

#[cfg(test)]
use cache::MediaCacheMergeMode;
#[cfg(test)]
use dispatch::{
    merge_mode_for_signal, should_publish_immediate_command_snapshot,
    should_schedule_metadata_fallback,
};
#[cfg(test)]
use owner::{
    owner_is_unchanged, owner_rebuild_outcome, replacement_removal_needs_snapshot,
    OwnerChangeOutcome,
};

#[cfg(test)]
#[path = "../tests/events.rs"]
mod event_tests;
