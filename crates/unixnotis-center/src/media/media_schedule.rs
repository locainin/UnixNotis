use std::collections::HashMap;
use std::time::Duration;

use tokio::sync::mpsc::Sender;
use tokio::task::JoinHandle;

use super::{MediaInfo, MediaRefreshOrigin, MediaSignal};

pub(super) type DelayedRefreshTasks = HashMap<String, JoinHandle<()>>;

// Short retries catch button-triggered state changes quickly
const COMMAND_REFRESH_DELAYS_MS: [u64; 2] = [150, 650];
// Passive track changes need an early retry so late art does not feel stuck
const METADATA_REFRESH_DELAYS_MS: [u64; 4] = [250, 900, 1800, 3200];
// Command path keeps the quick button retries and then falls into the same late-art sweep
const COMMAND_METADATA_REFRESH_DELAYS_MS: [u64; 6] = [150, 450, 900, 1800, 3200, 4800];

pub(super) fn cancel_delayed_refresh(tasks: &mut DelayedRefreshTasks, bus_name: &str) {
    // Player-specific cancellation keeps delayed refresh growth bounded.
    if let Some(task) = tasks.remove(bus_name) {
        task.abort();
    }
}

pub(super) fn prune_delayed_refreshes(tasks: &mut DelayedRefreshTasks) {
    // Drop completed handles so the task map tracks only active delayed work.
    tasks.retain(|_, task| !task.is_finished());
}

pub(super) fn schedule_command_refresh(
    tasks: &mut DelayedRefreshTasks,
    cache: &HashMap<String, MediaInfo>,
    signal_tx: Sender<MediaSignal>,
    bus_name: &str,
) {
    // Command-triggered updates should refresh quickly, then keep metadata fallback.
    if needs_metadata_fallback(cache, bus_name) {
        schedule_refresh_sequence(
            tasks,
            signal_tx,
            bus_name,
            &COMMAND_METADATA_REFRESH_DELAYS_MS,
        );
        return;
    }
    schedule_refresh_sequence(tasks, signal_tx, bus_name, &COMMAND_REFRESH_DELAYS_MS);
}

pub(super) fn schedule_metadata_fallback(
    tasks: &mut DelayedRefreshTasks,
    cache: &HashMap<String, MediaInfo>,
    signal_tx: Sender<MediaSignal>,
    bus_name: &str,
) {
    // Replace older delayed work so each player has at most one active retry plan.
    if !needs_metadata_fallback(cache, bus_name) {
        cancel_delayed_refresh(tasks, bus_name);
        return;
    }
    schedule_refresh_sequence(tasks, signal_tx, bus_name, &METADATA_REFRESH_DELAYS_MS);
}

pub(super) fn schedule_metadata_fallbacks(
    tasks: &mut DelayedRefreshTasks,
    cache: &HashMap<String, MediaInfo>,
    signal_tx: Sender<MediaSignal>,
) {
    for bus_name in cache.keys() {
        // Per-player scheduling helper handles both scheduling and cancellation.
        schedule_metadata_fallback(tasks, cache, signal_tx.clone(), bus_name);
    }
}

fn schedule_refresh_sequence(
    tasks: &mut DelayedRefreshTasks,
    signal_tx: Sender<MediaSignal>,
    bus_name: &str,
    delays_ms: &'static [u64],
) {
    cancel_delayed_refresh(tasks, bus_name);
    if delays_ms.is_empty() {
        return;
    }
    let key = bus_name.to_string();
    let target_name = key.clone();
    // Retry plans are static arrays, so the task can walk the borrowed slice
    // directly instead of allocating a fresh delay list every time
    // One task per player keeps retries bounded under noisy update streams
    let task = tokio::spawn(async move {
        let mut previous_ms = 0_u64;
        for &delay_ms in delays_ms {
            let step_ms = delay_ms.saturating_sub(previous_ms);
            previous_ms = delay_ms;
            if step_ms != 0 {
                tokio::time::sleep(Duration::from_millis(step_ms)).await;
            }
            if signal_tx
                .send(MediaSignal::PropertiesChanged {
                    bus_name: target_name.clone(),
                    origin: MediaRefreshOrigin::Fallback,
                })
                .await
                .is_err()
            {
                break;
            }
        }
    });
    tasks.insert(key, task);
}

fn needs_metadata_fallback(cache: &HashMap<String, MediaInfo>, bus_name: &str) -> bool {
    let Some(info) = cache.get(bus_name) else {
        return false;
    };
    // Some players publish track text first and artwork a moment later
    // Keep one retry plan active while playback is live so late art updates are not missed
    info.playback_status == "Playing"
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::media::MediaInfo;

    use super::needs_metadata_fallback;

    fn make_info(status: &str) -> MediaInfo {
        MediaInfo {
            bus_name: "org.mpris.MediaPlayer2.spotify".to_string(),
            identity: "Spotify".to_string(),
            browser_family: None,
            owner_pid: None,
            title: "track".to_string(),
            artist: "artist".to_string(),
            playback_status: status.to_string(),
            art_source: None,
            can_play: true,
            can_pause: true,
            can_next: true,
            can_prev: true,
        }
    }

    #[test]
    fn metadata_fallback_stays_on_while_playing() {
        let mut cache = HashMap::new();
        cache.insert(
            "org.mpris.MediaPlayer2.spotify".to_string(),
            make_info("Playing"),
        );

        assert!(needs_metadata_fallback(
            &cache,
            "org.mpris.MediaPlayer2.spotify"
        ));
    }

    #[test]
    fn metadata_fallback_stops_when_not_playing() {
        let mut cache = HashMap::new();
        cache.insert(
            "org.mpris.MediaPlayer2.spotify".to_string(),
            make_info("Paused"),
        );

        assert!(!needs_metadata_fallback(
            &cache,
            "org.mpris.MediaPlayer2.spotify"
        ));
    }
}
