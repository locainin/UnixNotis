//! MPRIS discovery, player construction, listeners, and control

mod admission;
mod command;
mod constants;
mod credentials;
mod discovery;
mod listener;
mod metadata;
mod player;
mod process;

pub(in crate::media) use admission::is_allowed_player;
pub(in crate::media) use command::handle_command;
pub(in crate::media) use constants::{MAX_MPRIS_PLAYERS, MPRIS_PREFIX};
pub(in crate::media) use discovery::refresh_players;
pub(in crate::media) use listener::spawn_properties_listener;
pub(in crate::media) use metadata::fetch_media_info;
pub(in crate::media) use player::{build_player_state, PlayerState};

#[cfg(test)]
pub(in crate::media) mod tests;
