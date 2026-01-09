//! D-Bus runtime for center UI events and control commands.

use std::collections::VecDeque;
use std::time::Duration;

use futures_util::StreamExt;
use tokio::sync::mpsc::{self, UnboundedSender};
use tracing::{info, warn};
use unixnotis_core::{
    CloseReason, ControlProxy, ControlState, Margins, NotificationView, PanelDebugLevel,
    PanelRequest,
};
use zbus::{Connection, Result as ZbusResult};

use crate::debug;
use crate::media::MediaInfo;

/// Events delivered to the GTK main loop.
#[derive(Debug, Clone)]
pub enum UiEvent {
    Seed {
        state: ControlState,
        active: Vec<NotificationView>,
        history: Vec<NotificationView>,
    },
    NotificationAdded(NotificationView, bool),
    NotificationUpdated(NotificationView, bool),
    NotificationClosed(u32, CloseReason),
    StateChanged(ControlState),
    PanelRequested(PanelRequest),
    GroupToggled(String),
    /// Updated set of active media players for the widget.
    MediaUpdated(Vec<MediaInfo>),
    MediaCleared,
    /// Hyprland active-window change that may indicate a click-away.
    ClickOutside,
    /// Hyprland reserved work area update for panel sizing.
    WorkAreaUpdated(Option<Margins>),
    RefreshWidgets,
    CssReload,
    ConfigReload,
}

/// Commands sent from GTK handlers to the D-Bus runtime.
#[derive(Debug, Clone)]
pub enum UiCommand {
    Dismiss(u32),
    InvokeAction { id: u32, action_key: String },
    ClearAll,
    SetDnd(bool),
    ClosePanel,
}

pub fn start_dbus_task(
    runtime: &tokio::runtime::Handle,
    connection: Connection,
    sender: async_channel::Sender<UiEvent>,
) -> UnboundedSender<UiCommand> {
    let (command_tx, command_rx) = mpsc::unbounded_channel();
    runtime.spawn(run_dbus_loop(connection, sender, command_rx));
    command_tx
}

async fn run_dbus_loop(
    connection: Connection,
    sender: async_channel::Sender<UiEvent>,
    mut command_rx: mpsc::UnboundedReceiver<UiCommand>,
) {
    // Buffer UI actions during reconnect to avoid losing user intent.
    let mut offline_commands: VecDeque<UiCommand> = VecDeque::new();

    loop {
        let proxy = match ControlProxy::new(&connection).await {
            Ok(proxy) => proxy,
            Err(err) => {
                warn!(?err, "control interface unavailable, retrying");
                stash_offline_commands(&mut command_rx, &mut offline_commands);
                tokio::time::sleep(Duration::from_millis(500)).await;
                continue;
            }
        };
        info!("connected to unixnotis control interface");
        seed_state(&proxy, &sender).await;
        flush_offline_commands(&proxy, &sender, &mut offline_commands).await;

        let mut added_stream = match proxy.receive_notification_added().await {
            Ok(stream) => stream,
            Err(err) => {
                warn!(?err, "failed to subscribe to notification_added");
                tokio::time::sleep(Duration::from_millis(300)).await;
                continue;
            }
        };
        let mut updated_stream = match proxy.receive_notification_updated().await {
            Ok(stream) => stream,
            Err(err) => {
                warn!(?err, "failed to subscribe to notification_updated");
                tokio::time::sleep(Duration::from_millis(300)).await;
                continue;
            }
        };
        let mut closed_stream = match proxy.receive_notification_closed().await {
            Ok(stream) => stream,
            Err(err) => {
                warn!(?err, "failed to subscribe to notification_closed");
                tokio::time::sleep(Duration::from_millis(300)).await;
                continue;
            }
        };
        let mut state_stream = match proxy.receive_state_changed().await {
            Ok(stream) => stream,
            Err(err) => {
                warn!(?err, "failed to subscribe to state_changed");
                tokio::time::sleep(Duration::from_millis(300)).await;
                continue;
            }
        };
        let mut panel_stream = match proxy.receive_panel_requested().await {
            Ok(stream) => stream,
            Err(err) => {
                warn!(?err, "failed to subscribe to panel_requested");
                tokio::time::sleep(Duration::from_millis(300)).await;
                continue;
            }
        };

        loop {
            tokio::select! {
                command = command_rx.recv() => {
                    let Some(command) = command else {
                        break;
                    };
                    if let Err(err) = handle_command(&proxy, &sender, command).await {
                        warn!(?err, "control command failed");
                    }
                }
                signal = added_stream.next() => {
                    let Some(signal) = signal else {
                        warn!("notification_added stream ended");
                        break;
                    };
                    if let Ok(args) = signal.args() {
                        let _ = sender
                            .send(UiEvent::NotificationAdded(
                                args.notification().clone(),
                                *args.show_popup(),
                            ))
                            .await;
                    }
                }
                signal = updated_stream.next() => {
                    let Some(signal) = signal else {
                        warn!("notification_updated stream ended");
                        break;
                    };
                    if let Ok(args) = signal.args() {
                        let _ = sender
                            .send(UiEvent::NotificationUpdated(
                                args.notification().clone(),
                                *args.show_popup(),
                            ))
                            .await;
                    }
                }
                signal = closed_stream.next() => {
                    let Some(signal) = signal else {
                        warn!("notification_closed stream ended");
                        break;
                    };
                    if let Ok(args) = signal.args() {
                        let _ = sender
                            .send(UiEvent::NotificationClosed(
                                *args.id(),
                                *args.reason(),
                            ))
                            .await;
                    }
                }
                signal = state_stream.next() => {
                    let Some(signal) = signal else {
                        warn!("state_changed stream ended");
                        break;
                    };
                    if let Ok(args) = signal.args() {
                        let _ = sender.send(UiEvent::StateChanged(args.state().clone())).await;
                    }
                }
                signal = panel_stream.next() => {
                    let Some(signal) = signal else {
                        warn!("panel_requested stream ended");
                        break;
                    };
                    if let Ok(args) = signal.args() {
                        let _ = sender.send(UiEvent::PanelRequested(*args.request())).await;
                    }
                }
            }
        }
        stash_offline_commands(&mut command_rx, &mut offline_commands);
        tokio::time::sleep(Duration::from_millis(300)).await;
    }
}

async fn seed_state(proxy: &ControlProxy<'_>, sender: &async_channel::Sender<UiEvent>) {
    let state = proxy.get_state().await;
    let active = proxy.list_active().await;
    let history = proxy.list_history().await;

    if let (Ok(state), Ok(active), Ok(history)) = (state, active, history) {
        let _ = sender
            .send(UiEvent::Seed {
                state,
                active,
                history,
            })
            .await;
    }
}

async fn handle_command(
    proxy: &ControlProxy<'_>,
    sender: &async_channel::Sender<UiEvent>,
    command: UiCommand,
) -> ZbusResult<()> {
    match command {
        UiCommand::Dismiss(id) => proxy.dismiss(id).await,
        UiCommand::InvokeAction { id, action_key } => proxy.invoke_action(id, &action_key).await,
        UiCommand::ClearAll => {
            proxy.clear_all().await?;
            seed_state(proxy, sender).await;
            Ok(())
        }
        UiCommand::SetDnd(enabled) => proxy.set_dnd(enabled).await,
        UiCommand::ClosePanel => proxy.close_panel().await,
    }
}

const MAX_OFFLINE_COMMANDS: usize = 128;

fn stash_offline_commands(
    command_rx: &mut mpsc::UnboundedReceiver<UiCommand>,
    offline: &mut VecDeque<UiCommand>,
) {
    let mut drained = 0usize;
    while let Ok(command) = command_rx.try_recv() {
        if offline.len() >= MAX_OFFLINE_COMMANDS {
            offline.pop_front();
            warn!("dropping control command while interface is unavailable");
        }
        offline.push_back(command);
        drained += 1;
    }
    if drained > 0 {
        debug::log(PanelDebugLevel::Info, || {
            format!(
                "buffered {drained} control command(s) while offline (queued={})",
                offline.len()
            )
        });
    }
}

async fn flush_offline_commands(
    proxy: &ControlProxy<'_>,
    sender: &async_channel::Sender<UiEvent>,
    offline: &mut VecDeque<UiCommand>,
) {
    if offline.is_empty() {
        return;
    }
    debug::log(PanelDebugLevel::Info, || {
        format!("replaying {} buffered control command(s)", offline.len())
    });
    while let Some(command) = offline.pop_front() {
        if let Err(err) = handle_command(proxy, sender, command).await {
            warn!(?err, "buffered control command failed");
        }
    }
}
