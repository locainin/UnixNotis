//! Player-state construction and inventory commits

use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::pin::Pin;

use tokio::sync::mpsc::Sender;
use unixnotis_core::{MediaConfig, PanelDebugLevel};
use zbus::Connection;

use super::player::{build_player_state_for_owner, OwnerProbe};
use super::{spawn_properties_listener, MprisFairnessState, PlayerState};
use crate::diagnostics::panel_debug as debug;
use crate::media::runtime::MediaSignal;

pub(in crate::media) type PlayerStateBuildFuture<'a> =
    Pin<Box<dyn Future<Output = zbus::Result<PlayerState>> + Send + 'a>>;
pub(in crate::media) type PlayerStateBuilder =
    for<'a> fn(&'a Connection, &'a str, &'a MediaConfig, OwnerProbe) -> PlayerStateBuildFuture<'a>;

pub(super) fn build_dbus_player_state<'a>(
    connection: &'a Connection,
    name: &'a str,
    config: &'a MediaConfig,
    owner: OwnerProbe,
) -> PlayerStateBuildFuture<'a> {
    Box::pin(build_player_state_for_owner(
        connection, name, config, owner,
    ))
}

pub(super) fn insert_player_state(
    players: &mut HashMap<String, PlayerState>,
    signal_tx: &Sender<MediaSignal>,
    name: String,
    state: PlayerState,
) {
    let properties = state.properties.clone();
    let listener_cancel = state.listener_cancel.subscribe();
    players.insert(name.clone(), state);
    spawn_properties_listener(properties, name.clone(), signal_tx.clone(), listener_cancel);
    debug::log(PanelDebugLevel::Info, || {
        format!("media player added: {name}")
    });
}

pub(super) enum FairnessAdmission {
    Admitted {
        victim_name: String,
        candidate_name: String,
    },
    BuildFailed {
        candidate_name: String,
        error: zbus::Error,
    },
    NoVictim,
}

pub(super) async fn admit_fairness_candidate(
    connection: &Connection,
    config: &MediaConfig,
    signal_tx: &Sender<MediaSignal>,
    players: &mut HashMap<String, PlayerState>,
    fairness: &mut MprisFairnessState,
    candidate: (String, OwnerProbe),
    build_player: PlayerStateBuilder,
) -> FairnessAdmission {
    let (candidate_name, owner) = candidate;
    let state = match build_player(connection, &candidate_name, config, owner).await {
        Ok(state) => state,
        Err(error) => {
            return FairnessAdmission::BuildFailed {
                candidate_name,
                error,
            };
        }
    };

    // Victim selection occurs only after the replacement is fully constructible
    let tracked_names = players.keys().cloned().collect::<HashSet<_>>();
    let Some(victim_name) = fairness.select_victim(&tracked_names) else {
        return FairnessAdmission::NoVictim;
    };
    let Some(victim) = players.remove(&victim_name) else {
        return FairnessAdmission::NoVictim;
    };

    // No await separates removal and insertion, so capacity never exposes a partial commit
    let _ = victim.listener_cancel.send(true);
    insert_player_state(players, signal_tx, candidate_name.clone(), state);
    FairnessAdmission::Admitted {
        victim_name,
        candidate_name,
    }
}
