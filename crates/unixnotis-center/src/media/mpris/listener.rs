//! Per-player MPRIS property listeners

use std::collections::HashMap;

use futures_util::StreamExt;
use tokio::sync::{mpsc::Sender, watch};
use tracing::warn;
use unixnotis_core::PanelDebugLevel;
use zbus::fdo::PropertiesProxy;
use zbus::message::Type;
use zbus::names::InterfaceName;
use zbus::zvariant::Value;
use zbus::{MatchRule, Message, MessageStream};

use super::constants::{
    MAX_MPRIS_CHANGED_PROPERTIES, MAX_MPRIS_PROPERTIES_CHANGED_BODY_BYTES, MPRIS_PLAYER,
};
use crate::diagnostics::panel_debug as debug;
use crate::media::runtime::{MediaRefreshOrigin, MediaSignal};

pub(in crate::media) fn spawn_properties_listener(
    properties: PropertiesProxy<'static>,
    bus_name: String,
    signal_tx: Sender<MediaSignal>,
    mut cancel_rx: watch::Receiver<bool>,
) {
    tokio::spawn(async move {
        let connection = properties.inner().connection().clone();
        let destination = properties.inner().destination().to_owned();
        let path = properties.inner().path().to_owned();
        let rule = match MatchRule::builder()
            .msg_type(Type::Signal)
            .sender(destination)
            .and_then(|builder| builder.path(path))
            .and_then(|builder| builder.interface("org.freedesktop.DBus.Properties"))
            .and_then(|builder| builder.member("PropertiesChanged"))
            .map(zbus::MatchRuleBuilder::build)
        {
            Ok(rule) => rule,
            Err(err) => {
                warn!(?err, "failed to build media property signal rule");
                return;
            }
        };
        let mut stream = match MessageStream::for_match_rule(rule, &connection, Some(32)).await {
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
                    let Ok(message) = update else {
                        continue;
                    };
                    let Some(relevant) = relevant_media_change_from_message(&message) else {
                        continue;
                    };
                    if !relevant {
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

pub(super) fn relevant_media_change_from_message(message: &Message) -> Option<bool> {
    // SECURITY: enforce the encoded signal-body budget before deserializing
    // `a{sv}`. Dynamic zvariant values may allocate attacker-controlled memory
    if !properties_changed_body_allowed(message.body().len()) {
        return None;
    }
    let body = message.body();
    let (interface_name, changed, invalidated): (
        InterfaceName<'_>,
        HashMap<&str, Value<'_>>,
        Vec<&str>,
    ) = body.deserialize().ok()?;
    if interface_name.as_str() != MPRIS_PLAYER
        || !changed_property_count_allowed(changed.len(), invalidated.len())
    {
        return None;
    }
    Some(is_relevant_media_change(&changed, &invalidated))
}

pub(super) const fn properties_changed_body_allowed(body_len: usize) -> bool {
    body_len <= MAX_MPRIS_PROPERTIES_CHANGED_BODY_BYTES
}

pub(super) const fn changed_property_count_allowed(
    changed_count: usize,
    invalidated_count: usize,
) -> bool {
    match changed_count.checked_add(invalidated_count) {
        Some(count) => count <= MAX_MPRIS_CHANGED_PROPERTIES,
        None => false,
    }
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
