use std::collections::HashMap;

use crate::media::MediaInfo;

use super::{
    needs_metadata_fallback, schedule_metadata_fallback, schedule_metadata_fallbacks,
    DelayedRefreshTasks,
};

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

#[tokio::test]
async fn repeated_bus_updates_keep_the_original_bounded_refresh_plan() {
    let bus_name = "org.mpris.MediaPlayer2.spotify";
    let mut cache = HashMap::new();
    cache.insert(bus_name.to_string(), make_info("Playing"));
    let (signal_tx, _signal_rx) = tokio::sync::mpsc::channel(4);
    let mut tasks = DelayedRefreshTasks::new();

    schedule_metadata_fallback(&mut tasks, &cache, signal_tx.clone(), bus_name);
    let original_id = tasks.get(bus_name).expect("initial refresh plan").id();
    schedule_metadata_fallback(&mut tasks, &cache, signal_tx, bus_name);
    let current_id = tasks.get(bus_name).expect("preserved refresh plan").id();

    assert_eq!(current_id, original_id);
    tasks.remove(bus_name).expect("refresh plan").abort();
}

#[tokio::test]
async fn fallback_sweep_schedules_each_playing_player() {
    let mut cache = HashMap::new();
    for bus_name in [
        "org.mpris.MediaPlayer2.alpha",
        "org.mpris.MediaPlayer2.beta",
    ] {
        let mut info = make_info("Playing");
        info.bus_name = bus_name.to_string();
        cache.insert(bus_name.to_string(), info);
    }
    let (signal_tx, _signal_rx) = tokio::sync::mpsc::channel(4);
    let mut tasks = DelayedRefreshTasks::new();

    schedule_metadata_fallbacks(&mut tasks, &cache, signal_tx);

    assert_eq!(tasks.len(), 2);
    for (_, task) in tasks {
        task.abort();
    }
}
