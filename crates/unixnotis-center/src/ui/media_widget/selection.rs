use crate::media::MediaInfo;

#[derive(Clone, Default)]
pub(crate) struct MediaSelectionSnapshot {
    // The last player list is kept so a rebuilt shell can paint immediately
    pub(super) players: Vec<MediaInfo>,
    // The current bus name keeps the same player selected after a rebuild
    pub(super) current_bus: Option<String>,
}

impl MediaSelectionSnapshot {
    pub(crate) fn is_empty(&self) -> bool {
        // Empty snapshots mean there is nothing useful to restore
        self.players.is_empty()
    }
}

#[derive(Default)]
pub(super) struct MediaSelection {
    // The live player snapshot mirrors the latest media runtime update
    pub(super) players: Vec<MediaInfo>,
    // The card only needs one active slot at a time
    current_index: usize,
}

impl MediaSelection {
    pub(super) fn set_players(&mut self, players: Vec<MediaInfo>) {
        // Hold onto the current bus so refreshes do not kick the user back to slot one
        self.set_players_from_bus(players, self.current_bus());
    }

    pub(super) fn restore_snapshot(&mut self, snapshot: &MediaSelectionSnapshot) {
        // Config reload rebuilds should keep the same visible player when it still exists
        self.set_players_from_bus(snapshot.players.clone(), snapshot.current_bus.clone());
    }

    pub(super) fn current(&self) -> Option<&MediaInfo> {
        // Out of range indexes simply mean there is no active player
        self.players.get(self.current_index)
    }

    pub(super) fn current_bus(&self) -> Option<String> {
        // The bus name is the stable key used to carry selection across refreshes
        self.current().map(|info| info.bus_name.clone())
    }

    pub(super) fn snapshot(&self) -> MediaSelectionSnapshot {
        // The rebuild path needs both the player list and the active selection
        MediaSelectionSnapshot {
            players: self.players.clone(),
            current_bus: self.current_bus(),
        }
    }

    pub(super) fn next(&mut self) {
        if self.players.len() <= 1 {
            // Single-player snapshots do not need a moving cursor
            return;
        }
        // Wraparound keeps the carousel controls simple
        self.current_index = (self.current_index + 1) % self.players.len();
    }

    pub(super) fn prev(&mut self) {
        if self.players.len() <= 1 {
            // Single-player snapshots do not need a moving cursor
            return;
        }
        if self.current_index == 0 {
            // Reverse navigation wraps to the last card
            self.current_index = self.players.len() - 1;
        } else {
            self.current_index -= 1;
        }
    }

    pub(super) fn has_multiple(&self) -> bool {
        // The nav buttons and counter badge both key off this one check
        self.players.len() > 1
    }

    pub(super) fn position(&self) -> (usize, usize) {
        if self.players.is_empty() {
            return (0, 0);
        }
        // The UI shows one-based positions because that reads better in the pill
        (self.current_index + 1, self.players.len())
    }

    fn set_players_from_bus(&mut self, players: Vec<MediaInfo>, current_bus: Option<String>) {
        // Swap the whole snapshot first so later lookups see the latest list
        self.players = players;
        if self.players.is_empty() {
            // An empty list always resets the cursor back to slot zero
            self.current_index = 0;
            return;
        }
        if let Some(current_bus) = current_bus {
            if let Some(index) = self
                .players
                .iter()
                .position(|info| info.bus_name == current_bus)
            {
                // Keep the same visible player when the snapshot order changes
                self.current_index = index;
                return;
            }
        }
        // If the previous player is gone, start from the first visible entry again
        self.current_index = 0;
    }
}

#[cfg(test)]
#[path = "tests/selection.rs"]
mod tests;
