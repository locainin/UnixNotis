//! MPRIS discovery, player construction, listeners, and control

mod admission;
mod command;
mod constants;
mod discovery;
mod listener;
mod metadata;
mod player;

pub(in crate::media) use admission::is_allowed_player;
#[cfg(test)]
pub(in crate::media) use admission::{detect_browser_family, remote_art_allowed};
pub(in crate::media) use command::handle_command;
pub(in crate::media) use constants::MPRIS_PREFIX;
pub(in crate::media) use discovery::refresh_players;
pub(in crate::media) use listener::spawn_properties_listener;
pub(in crate::media) use metadata::fetch_media_info;
pub(in crate::media) use player::{build_player_state, PlayerState};

#[cfg(test)]
use listener::is_relevant_media_change;
#[cfg(test)]
use player::owner_probe_is_stable;

#[cfg(test)]
#[path = "../tests/bus.rs"]
mod tests;
