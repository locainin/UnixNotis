use std::time::Duration;

use futures_util::TryStreamExt;
use unixnotis_core::{CloseReason, Config, ControlState, PopupGateState, CONTROL_OBJECT_PATH};
use zbus::message::Type;
use zbus::{Connection, MatchRule, Message, MessageStream};

use crate::daemon::NOTIFICATIONS_OBJECT_PATH;
use crate::store::NotificationStore;
use crate::test_support::daemon_state_for_test;

use super::super::publisher::record_first_error;
use super::super::state::{popup_gate_from_state, should_publish_any_state_signal};

async fn signal_stream(
    state: &crate::daemon::DaemonState,
    path: &str,
    interface: &str,
    member: &str,
) -> MessageStream {
    let receiver = Connection::session().await.expect("receiver session bus");
    let sender = state
        .connection()
        .unique_name()
        .expect("daemon connection has unique name")
        .to_string();
    let rule = MatchRule::builder()
        .msg_type(Type::Signal)
        .sender(sender.as_str())
        .expect("sender")
        .path(path)
        .expect("path")
        .interface(interface)
        .expect("interface")
        .member(member)
        .expect("member")
        .build();
    MessageStream::for_match_rule(rule, &receiver, Some(8))
        .await
        .expect("signal stream")
}

async fn control_signal_stream(state: &crate::daemon::DaemonState, member: &str) -> MessageStream {
    signal_stream(state, CONTROL_OBJECT_PATH, "com.unixnotis.Control", member).await
}

async fn notifications_signal_stream(
    state: &crate::daemon::DaemonState,
    member: &str,
) -> MessageStream {
    signal_stream(
        state,
        NOTIFICATIONS_OBJECT_PATH,
        "org.freedesktop.Notifications",
        member,
    )
    .await
}

async fn next_signal(stream: &mut MessageStream) -> Message {
    tokio::time::timeout(Duration::from_millis(500), stream.try_next())
        .await
        .expect("signal should arrive before timeout")
        .expect("signal stream should stay open")
        .expect("signal message")
}

async fn assert_no_signal(stream: &mut MessageStream) {
    assert!(
        tokio::time::timeout(Duration::from_millis(100), stream.try_next())
            .await
            .is_err(),
        "signal should not be emitted"
    );
}

#[test]
fn popup_gate_from_state_ignores_history_and_inhibitor_counts() {
    let state = ControlState {
        dnd_enabled: true,
        dnd_expires_at: 0,
        history_count: 99,
        inhibited: false,
        inhibitor_count: 12,
    };

    let gate = popup_gate_from_state(&state);

    assert!(gate.dnd_enabled);
    assert!(!gate.inhibited);
}

#[test]
fn notification_store_control_state_reads_dnd_history_and_inhibitors() {
    let mut store = NotificationStore::new(Config::default());

    store.set_dnd_until(500);
    store.add_inhibitor(":1.test".to_string(), "focus".to_string(), 0);

    let state = store.control_state();

    assert!(state.dnd_enabled);
    assert_eq!(state.dnd_expires_at, 500);
    assert!(state.inhibited);
    assert_eq!(state.inhibitor_count, 1);
    assert_eq!(state.history_count, 0);
}

#[test]
fn should_publish_any_state_signal_is_false_when_both_cached_values_match() {
    assert!(!should_publish_any_state_signal(false, false));
}

#[test]
fn should_publish_any_state_signal_is_true_when_control_state_changed() {
    assert!(should_publish_any_state_signal(true, false));
}

#[test]
fn should_publish_any_state_signal_is_true_when_popup_gate_changed() {
    assert!(should_publish_any_state_signal(false, true));
}

#[test]
fn should_publish_any_state_signal_is_true_when_both_values_changed() {
    assert!(should_publish_any_state_signal(true, true));
}

#[test]
fn record_first_error_stores_first_error() {
    let mut first_error = None;

    record_first_error(&mut first_error, zbus::Error::Failure("first".to_string()));

    assert_eq!(first_error, Some(zbus::Error::Failure("first".to_string())));
}

#[test]
fn record_first_error_keeps_existing_error() {
    let mut first_error = Some(zbus::Error::Failure("first".to_string()));

    record_first_error(&mut first_error, zbus::Error::Failure("second".to_string()));

    assert_eq!(first_error, Some(zbus::Error::Failure("first".to_string())));
}

#[tokio::test]
async fn publish_notification_closed_sends_freedesktop_and_control_close_signals() {
    let state = daemon_state_for_test(false).await;
    let mut freedesktop_stream = notifications_signal_stream(&state, "NotificationClosed").await;
    let mut control_stream = control_signal_stream(&state, "NotificationClosed").await;

    state
        .publish_notification_closed(7, CloseReason::ClosedByCall)
        .await
        .expect("close fanout should emit");

    let freedesktop_signal = next_signal(&mut freedesktop_stream).await;
    let (freedesktop_id, freedesktop_reason) = freedesktop_signal
        .body()
        .deserialize::<(u32, u32)>()
        .expect("freedesktop close body");
    assert_eq!(freedesktop_id, 7);
    assert_eq!(freedesktop_reason, CloseReason::ClosedByCall as u32);

    let control_signal = next_signal(&mut control_stream).await;
    let (control_id, control_reason) = control_signal
        .body()
        .deserialize::<(u32, CloseReason)>()
        .expect("control close body");
    assert_eq!(control_id, 7);
    assert_eq!(control_reason as u32, CloseReason::ClosedByCall as u32);
}

#[tokio::test]
async fn publish_notification_dismissed_sends_control_close_signal() {
    let state = daemon_state_for_test(false).await;
    let mut control_stream = control_signal_stream(&state, "NotificationClosed").await;

    state
        .publish_notification_dismissed(8, false)
        .await
        .expect("dismiss fanout should emit");

    let control_signal = next_signal(&mut control_stream).await;
    let (control_id, control_reason) = control_signal
        .body()
        .deserialize::<(u32, CloseReason)>()
        .expect("control close body");
    assert_eq!(control_id, 8);
    assert_eq!(control_reason as u32, CloseReason::DismissedByUser as u32);
}

#[tokio::test]
async fn publish_state_changed_sends_initial_state_and_suppresses_duplicate() {
    let state = daemon_state_for_test(false).await;
    let mut state_stream = control_signal_stream(&state, "StateChanged").await;
    let mut gate_stream = control_signal_stream(&state, "PopupGateChanged").await;

    state
        .publish_state_changed()
        .await
        .expect("state changed should emit");

    let state_signal = next_signal(&mut state_stream).await;
    let emitted_state = state_signal
        .body()
        .deserialize::<ControlState>()
        .expect("state body");
    assert!(!emitted_state.dnd_enabled);
    assert!(!emitted_state.inhibited);
    assert_eq!(emitted_state.history_count, 0);
    assert_eq!(emitted_state.inhibitor_count, 0);

    let gate_signal = next_signal(&mut gate_stream).await;
    let emitted_gate = gate_signal
        .body()
        .deserialize::<PopupGateState>()
        .expect("popup gate body");
    assert!(!emitted_gate.dnd_enabled);
    assert!(!emitted_gate.inhibited);

    state
        .publish_state_changed()
        .await
        .expect("duplicate state should not fail");
    assert_no_signal(&mut state_stream).await;
    assert_no_signal(&mut gate_stream).await;
}

#[tokio::test]
async fn publish_snapshot_invalidated_sends_snapshot_signal() {
    let state = daemon_state_for_test(false).await;
    let mut stream = control_signal_stream(&state, "SnapshotInvalidated").await;

    state
        .publish_snapshot_invalidated()
        .await
        .expect("snapshot invalidation should emit");

    let signal = next_signal(&mut stream).await;
    signal.body().deserialize::<()>().expect("empty body");
}
