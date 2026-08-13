//! Bounded MPRIS discovery-name selection

use std::collections::HashSet;

use unixnotis_core::MediaConfig;

use super::constants::{MAX_MPRIS_CANDIDATES_PER_PASS, MPRIS_PREFIX};
use super::is_allowed_player;

pub(super) fn is_discoverable_player(name: &str, config: &MediaConfig) -> bool {
    name.starts_with(MPRIS_PREFIX) && is_allowed_player(name, config)
}

pub(super) fn select_player_names(
    names: HashSet<String>,
    tracked: &HashSet<String>,
    cursor: &mut usize,
) -> Vec<String> {
    let mut names = names.into_iter().collect::<Vec<_>>();
    names.sort_unstable();

    let mut selected = names
        .iter()
        .filter(|name| tracked.contains(*name))
        .cloned()
        .collect::<Vec<_>>();
    let remaining = names
        .into_iter()
        .filter(|name| !tracked.contains(name))
        .collect::<Vec<_>>();
    let room = MAX_MPRIS_CANDIDATES_PER_PASS.saturating_sub(selected.len());
    if room == 0 || remaining.is_empty() {
        return selected;
    }

    let start = *cursor % remaining.len();
    let count = room.min(remaining.len());
    selected.extend((0..count).map(|offset| remaining[(start + offset) % remaining.len()].clone()));
    *cursor = (start + count) % remaining.len();
    selected
}
