//! Commands sent to admitted MPRIS players

use std::collections::HashMap;

use unixnotis_core::PanelDebugLevel;

use super::PlayerState;
use crate::diagnostics::panel_debug as debug;
use crate::media::MediaCommand;

pub(in crate::media) async fn handle_command(
    players: &HashMap<String, PlayerState>,
    command: MediaCommand,
) -> zbus::Result<Option<String>> {
    match command {
        MediaCommand::Refresh => Ok(None),
        MediaCommand::PlayPause { bus_name } => {
            if let Some(state) = players.get(&bus_name) {
                debug::log(PanelDebugLevel::Info, || {
                    format!("media command: play/pause {bus_name}")
                });
                // The returned bus name triggers a fast refresh for the targeted player
                let _value: () = state.player.call("PlayPause", &()).await?;
                return Ok(Some(bus_name));
            }
            Ok(None)
        }
        MediaCommand::Next { bus_name } => {
            if let Some(state) = players.get(&bus_name) {
                debug::log(PanelDebugLevel::Info, || {
                    format!("media command: next {bus_name}")
                });
                // The returned bus name triggers a fast refresh for the targeted player
                let _value: () = state.player.call("Next", &()).await?;
                return Ok(Some(bus_name));
            }
            Ok(None)
        }
        MediaCommand::Previous { bus_name } => {
            if let Some(state) = players.get(&bus_name) {
                debug::log(PanelDebugLevel::Info, || {
                    format!("media command: previous {bus_name}")
                });
                // The returned bus name triggers a fast refresh for the targeted player
                let _value: () = state.player.call("Previous", &()).await?;
                return Ok(Some(bus_name));
            }
            Ok(None)
        }
    }
}
