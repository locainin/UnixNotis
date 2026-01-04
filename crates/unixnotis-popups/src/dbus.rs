//! D-Bus runtime for popup UI events and control updates.

use std::thread;
use std::time::Duration;

use futures_util::StreamExt;
use tokio::sync::mpsc::{self, UnboundedSender};
use tracing::{info, warn};
use unixnotis_core::{CloseReason, ControlProxy, ControlState, NotificationView};
use zbus::{Connection, Result as ZbusResult};

/// Events delivered to the GTK main loop.
#[derive(Debug, Clone)]
pub enum UiEvent {
    Seed {
        state: ControlState,
        active: Vec<NotificationView>,
    },
    NotificationAdded(NotificationView, bool),
    NotificationUpdated(NotificationView, bool),
    NotificationClosed(u32, CloseReason),
    StateChanged(ControlState),
    CssReload,
    ConfigReload,
}

/// Commands sent from GTK handlers to the D-Bus runtime.
#[derive(Debug, Clone)]
pub enum UiCommand {
    Dismiss(u32),
    InvokeAction { id: u32, action_key: String },
}

pub fn start_dbus_runtime(sender: async_channel::Sender<UiEvent>) -> UnboundedSender<UiCommand> {
    let (command_tx, mut command_rx) = mpsc::unbounded_channel();

    thread::spawn(move || {
        // Dedicated runtime keeps async D-Bus work off the GTK main thread.
        let runtime = match tokio::runtime::Runtime::new() {
            Ok(runtime) => runtime,
            Err(err) => {
                warn!(?err, "failed to initialize tokio runtime");
                return;
            }
        };
        runtime.block_on(async move {
            let connection = match Connection::session().await {
                Ok(connection) => connection,
                Err(err) => {
                    warn!(?err, "failed to connect to session bus");
                    return;
                }
            };

            loop {
                let proxy = match ControlProxy::new(&connection).await {
                    Ok(proxy) => proxy,
                    Err(err) => {
                        warn!(?err, "control interface unavailable, retrying");
                        drain_offline_commands(&mut command_rx);
                        tokio::time::sleep(Duration::from_millis(500)).await;
                        continue;
                    }
                };
                info!("connected to unixnotis control interface");
                seed_state(&proxy, &sender).await;

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

                loop {
                    tokio::select! {
                        command = command_rx.recv() => {
                            let Some(command) = command else {
                                break;
                            };
                            if let Err(err) = handle_command(&proxy, command).await {
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
                    }
                }
                tokio::time::sleep(Duration::from_millis(300)).await;
            }
        });
    });

    command_tx
}

async fn seed_state(proxy: &ControlProxy<'_>, sender: &async_channel::Sender<UiEvent>) {
    let state = proxy.get_state().await;
    let active = proxy.list_active().await;

    if let (Ok(state), Ok(active)) = (state, active) {
        let _ = sender.send(UiEvent::Seed { state, active }).await;
    }
}

async fn handle_command(proxy: &ControlProxy<'_>, command: UiCommand) -> ZbusResult<()> {
    match command {
        UiCommand::Dismiss(id) => proxy.dismiss(id).await,
        UiCommand::InvokeAction { id, action_key } => proxy.invoke_action(id, &action_key).await,
    }
}

fn drain_offline_commands(command_rx: &mut mpsc::UnboundedReceiver<UiCommand>) {
    while command_rx.try_recv().is_ok() {
        warn!("dropping control command while interface is unavailable");
    }
}
