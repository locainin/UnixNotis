//! Mutable state owned by one media bus generation

use std::collections::HashMap;

use super::schedule::DelayedRefreshTasks;
use crate::media::mpris::PlayerState;
use crate::media::MediaInfo;

pub(super) struct MediaRuntimeState {
    // Live player proxies keyed by bus name
    pub(super) players: HashMap<String, PlayerState>,
    // Last known media snapshot per player
    pub(super) cache: HashMap<String, MediaInfo>,
    // Last emitted snapshot lets the loop drop duplicate UI updates cheaply
    pub(super) last_snapshot: Vec<MediaInfo>,
    // One delayed retry plan per player
    pub(super) delayed_refreshes: DelayedRefreshTasks,
}

impl MediaRuntimeState {
    pub(super) fn new() -> Self {
        // A fresh loop starts empty and fills from the first refresh pass
        Self {
            players: HashMap::new(),
            cache: HashMap::new(),
            last_snapshot: Vec::new(),
            delayed_refreshes: HashMap::new(),
        }
    }
}

impl Drop for MediaRuntimeState {
    fn drop(&mut self) {
        // Connection teardown must cancel delayed work instead of detaching it
        for task in self.delayed_refreshes.drain().map(|(_, task)| task) {
            task.abort();
        }
        for player in self.players.values() {
            let _ = player.listener_cancel.send(true);
        }
    }
}
