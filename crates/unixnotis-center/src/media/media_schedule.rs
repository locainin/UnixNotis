//! Scheduling helpers for delayed media refreshes.
//!
//! Handles retry timing for players that emit late metadata.

use std::collections::HashMap;
use std::time::Duration;

use tokio::sync::mpsc::UnboundedSender;

use super::{MediaInfo, MediaSignal};

pub(super) fn schedule_delayed_refresh(
    signal_tx: UnboundedSender<MediaSignal>,
    bus_name: String,
    delay: Duration,
) {
    tokio::spawn(async move {
        tokio::time::sleep(delay).await;
        let _ = signal_tx.send(MediaSignal::PropertiesChanged(bus_name));
    });
}

pub(super) fn schedule_metadata_fallback(
    cache: &HashMap<String, MediaInfo>,
    signal_tx: UnboundedSender<MediaSignal>,
    bus_name: &str,
) {
    let Some(info) = cache.get(bus_name) else {
        return;
    };
    if info.playback_status != "Playing" {
        return;
    }
    if !info.title.is_empty() {
        return;
    }
    // Some players delay metadata updates during ads; retry briefly to catch late updates.
    for delay_ms in [1200_u64, 2400_u64, 3600_u64] {
        schedule_delayed_refresh(
            signal_tx.clone(),
            bus_name.to_string(),
            Duration::from_millis(delay_ms),
        );
    }
}

pub(super) fn schedule_metadata_fallbacks(
    cache: &HashMap<String, MediaInfo>,
    signal_tx: UnboundedSender<MediaSignal>,
) {
    for (bus_name, info) in cache {
        if info.playback_status == "Playing" && info.title.is_empty() {
            schedule_metadata_fallback(cache, signal_tx.clone(), bus_name);
        }
    }
}
