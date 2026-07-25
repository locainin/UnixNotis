use std::collections::HashMap;

use zbus::zvariant::{OwnedValue, Structure, Value};
use zbus::Connection;

use super::{notify_body_is_oversized, NotificationIngress, MAX_NOTIFY_WIRE_BODY_BYTES};
use crate::daemon::{NotificationServer, NOTIFICATIONS_OBJECT_PATH};
use crate::expire::ExpirationScheduler;
use crate::test_support::daemon_state_for_test;

const NOTIFICATIONS_INTERFACE: &str = "org.freedesktop.Notifications";

#[test]
fn notify_wire_limit_applies_only_to_oversized_notify_calls() {
    assert_eq!(MAX_NOTIFY_WIRE_BODY_BYTES, 393_216);
    assert!(!notify_body_is_oversized(
        "Notify",
        MAX_NOTIFY_WIRE_BODY_BYTES
    ));
    assert!(notify_body_is_oversized(
        "Notify",
        MAX_NOTIFY_WIRE_BODY_BYTES + 1
    ));
    assert!(!notify_body_is_oversized(
        "CloseNotification",
        MAX_NOTIFY_WIRE_BODY_BYTES + 1
    ));
}

#[tokio::test]
async fn oversized_body_is_rejected_before_notify_deserialization() {
    let (state, client) = notification_ingress().await;
    let body = "b".repeat(MAX_NOTIFY_WIRE_BODY_BYTES + 1);

    assert_oversized_notify_rejected(&state, &client, Vec::new(), HashMap::new(), body).await;
}

#[tokio::test]
async fn oversized_action_array_is_rejected_before_notify_deserialization() {
    let (state, client) = notification_ingress().await;
    let actions = vec!["a".repeat(MAX_NOTIFY_WIRE_BODY_BYTES + 1)];

    assert_oversized_notify_rejected(&state, &client, actions, HashMap::new(), String::new()).await;
}

#[tokio::test]
async fn under_wire_limit_tiny_action_flood_never_reaches_typed_notify() {
    let (state, client) = notification_ingress().await;
    let actions = (0..20_000).map(|_| "a".to_string()).collect::<Vec<_>>();
    let probe = zbus::Message::method(NOTIFICATIONS_OBJECT_PATH, "Notify")
        .expect("method builder")
        .interface(NOTIFICATIONS_INTERFACE)
        .expect("notification interface")
        .build(&(
            "app",
            0_u32,
            "",
            "summary",
            "",
            &actions,
            HashMap::<String, OwnedValue>::new(),
            0_i32,
        ))
        .expect("action flood probe");
    assert!(probe.body().len() < MAX_NOTIFY_WIRE_BODY_BYTES);

    assert_oversized_notify_rejected(&state, &client, actions, HashMap::new(), String::new()).await;
}

#[tokio::test]
async fn oversized_hint_map_is_rejected_before_notify_deserialization() {
    let (state, client) = notification_ingress().await;
    let mut hints = HashMap::new();
    let value = Value::from("h".repeat(MAX_NOTIFY_WIRE_BODY_BYTES + 1));
    hints.insert(
        "category".to_string(),
        OwnedValue::try_from(value).expect("owned hint string"),
    );

    assert_oversized_notify_rejected(&state, &client, Vec::new(), hints, String::new()).await;
}

#[tokio::test]
async fn oversized_image_array_is_rejected_before_notify_deserialization() {
    let (state, client) = notification_ingress().await;
    let image = Structure::from((
        1_i32,
        1_i32,
        4_i32,
        true,
        8_i32,
        4_i32,
        vec![0_u8; MAX_NOTIFY_WIRE_BODY_BYTES + 1],
    ));
    let mut hints = HashMap::new();
    hints.insert(
        "image-data".to_string(),
        OwnedValue::try_from(Value::from(image)).expect("owned image hint"),
    );

    assert_oversized_notify_rejected(&state, &client, Vec::new(), hints, String::new()).await;
}

#[tokio::test]
async fn under_wire_limit_image_above_its_allowance_never_reaches_typed_notify() {
    let (state, client) = notification_ingress().await;
    let image = Structure::from((
        256_i32,
        256_i32,
        1024_i32,
        true,
        8_i32,
        4_i32,
        vec![0_u8; 256 * 1024 + 1],
    ));
    let mut hints = HashMap::new();
    hints.insert(
        "image-data".to_string(),
        OwnedValue::try_from(Value::from(image)).expect("owned image hint"),
    );

    assert_oversized_notify_rejected(&state, &client, Vec::new(), hints, String::new()).await;
}

#[tokio::test]
async fn bounded_notify_body_reaches_the_typed_interface() {
    let (state, client) = notification_ingress().await;
    let destination = state
        .connection()
        .unique_name()
        .expect("daemon unique name")
        .clone();
    let payload = (
        "app",
        0_u32,
        "",
        "summary",
        "bounded body",
        Vec::<String>::new(),
        HashMap::<String, OwnedValue>::new(),
        0_i32,
    );

    let reply = client
        .call_method(
            Some(destination),
            NOTIFICATIONS_OBJECT_PATH,
            Some(NOTIFICATIONS_INTERFACE),
            "Notify",
            &payload,
        )
        .await
        .expect("bounded Notify should reach typed handler");
    let id = reply.body().deserialize::<u32>().expect("notification id");

    assert_eq!(id, 1);
    assert_eq!(state.store.lock().await.list_active().len(), 1);
}

async fn notification_ingress() -> (std::sync::Arc<crate::daemon::DaemonState>, Connection) {
    let state = daemon_state_for_test(false).await;
    let scheduler = ExpirationScheduler::start(state.clone());
    state
        .connection()
        .object_server()
        .at(
            NOTIFICATIONS_OBJECT_PATH,
            NotificationIngress::new(NotificationServer::new(state.clone(), scheduler)),
        )
        .await
        .expect("register guarded notification interface");
    let client = Connection::session().await.expect("notification client");
    (state, client)
}

async fn assert_oversized_notify_rejected(
    state: &crate::daemon::DaemonState,
    client: &Connection,
    actions: Vec<String>,
    hints: HashMap<String, OwnedValue>,
    body: String,
) {
    let destination = state
        .connection()
        .unique_name()
        .expect("daemon unique name")
        .clone();
    let payload = ("app", 0_u32, "", "summary", body, actions, hints, 0_i32);

    let error = client
        .call_method(
            Some(destination),
            NOTIFICATIONS_OBJECT_PATH,
            Some(NOTIFICATIONS_INTERFACE),
            "Notify",
            &payload,
        )
        .await
        .expect_err("oversized Notify body must fail");

    assert!(
        error.to_string().contains("LimitsExceeded"),
        "unexpected D-Bus error: {error}"
    );
    assert!(state.store.lock().await.list_active().is_empty());
}
