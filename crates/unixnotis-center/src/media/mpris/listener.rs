//! Per-player MPRIS property listeners

use std::collections::HashMap;

use futures_util::StreamExt;
use tokio::sync::{mpsc::Sender, watch};
use tracing::warn;
use unixnotis_core::PanelDebugLevel;
use zbus::fdo::PropertiesProxy;

use super::constants::MPRIS_PLAYER;
use crate::diagnostics::panel_debug as debug;
use crate::media::runtime::{MediaRefreshOrigin, MediaSignal};

pub(in crate::media) fn spawn_properties_listener(
    properties: PropertiesProxy<'static>,
    bus_name: String,
    signal_tx: Sender<MediaSignal>,
    mut cancel_rx: watch::Receiver<bool>,
) {
    tokio::spawn(async move {
        let mut stream = match properties.receive_properties_changed().await {
            Ok(stream) => stream,
            Err(err) => {
                warn!(?err, "failed to subscribe to media properties");
                return;
            }
        };
        loop {
            tokio::select! {
                result = cancel_rx.changed() => {
                    // Exit promptly when the player is removed or cancellation is requested
                    if result.is_err() || *cancel_rx.borrow() {
                        break;
                    }
                }
                update = stream.next() => {
                    let Some(update) = update else {
                        break;
                    };
                    let Ok(args) = update.args() else {
                        continue;
                    };
                    if args.interface_name != MPRIS_PLAYER {
                        continue;
                    }
                    if !is_relevant_media_change(&args.changed_properties, &args.invalidated_properties) {
                        continue;
                    }
                    debug::log(PanelDebugLevel::Verbose, || {
                        format!("media properties changed: {bus_name}")
                    });
                    if signal_tx
                        .send(MediaSignal::PropertiesChanged {
                            bus_name: bus_name.clone(),
                            origin: MediaRefreshOrigin::Bus,
                        })
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
            }
        }
    });
}

pub(super) fn is_relevant_media_change(
    changed: &HashMap<&str, zbus::zvariant::Value<'_>>,
    invalidated: &[&str],
) -> bool {
    const KEYS: [&str; 8] = [
        "Metadata",
        "PlaybackStatus",
        "LoopStatus",
        "Shuffle",
        "CanPlay",
        "CanPause",
        "CanGoNext",
        "CanGoPrevious",
    ];

    // Ignore unrelated property churn so browser players do not wake the panel constantly
    if changed.keys().any(|key| KEYS.contains(key)) {
        return true;
    }
    invalidated.iter().any(|key| KEYS.contains(key))
}
