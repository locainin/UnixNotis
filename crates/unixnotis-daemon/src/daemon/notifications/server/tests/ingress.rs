use std::collections::HashMap;
use std::os::fd::AsFd;
use std::time::Duration;

use zbus::zvariant::{OwnedValue, SerializeValue, Structure, Value};
use zbus::{Connection, Message};

use super::{
    notify_body_is_oversized, notify_has_unix_fds, NotificationIngress, MAX_NOTIFY_WIRE_BODY_BYTES,
};
use crate::daemon::{NotificationServer, NOTIFICATIONS_OBJECT_PATH};
use crate::expire::ExpirationScheduler;
use crate::test_support::daemon_state_for_test;

const NOTIFICATIONS_INTERFACE: &str = "org.freedesktop.Notifications";
// Four-megabyte D-Bus fixtures need headroom when the full test binary runs in parallel
const TEST_NOTIFY_TIMEOUT: Duration = Duration::from_secs(10);

#[test]
fn notify_wire_limit_applies_only_to_oversized_notify_calls() {
    assert_eq!(MAX_NOTIFY_WIRE_BODY_BYTES, 4_325_376);
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

#[test]
fn unix_file_descriptors_are_rejected_only_for_notify_calls() {
    assert!(!notify_has_unix_fds("Notify", None));
    assert!(!notify_has_unix_fds("Notify", Some(0)));
    assert!(notify_has_unix_fds("Notify", Some(1)));
    assert!(!notify_has_unix_fds("CloseNotification", Some(1)));
}

#[test]
fn raw_message_header_exposes_attached_unix_file_descriptor_count() {
    let file = std::fs::File::open("/dev/null").expect("open descriptor fixture");
    let descriptor = zbus::zvariant::Fd::from(file.as_fd());
    let message = zbus::Message::method(NOTIFICATIONS_OBJECT_PATH, "Notify")
        .expect("method builder")
        .interface(NOTIFICATIONS_INTERFACE)
        .expect("notification interface")
        .build(&(descriptor,))
        .expect("descriptor-bearing message");

    assert_eq!(message.header().unix_fds(), Some(1));
    assert!(notify_has_unix_fds("Notify", message.header().unix_fds()));
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
    let error = send_image_notification(&state, &client, 1, 1, 4, MAX_NOTIFY_WIRE_BODY_BYTES + 1)
        .await
        .expect_err("image above the wire limit must fail");

    assert!(
        error.to_string().contains("LimitsExceeded"),
        "unexpected D-Bus error: {error}"
    );
    assert!(state.store.lock().await.list_active().is_empty());
}

#[tokio::test]
async fn native_image_above_retained_limit_keeps_the_text_notification() {
    let (state, client) = notification_ingress().await;
    let reply = send_image_notification(&state, &client, 1_024, 1_024, 4_096, 1_024 * 1_024 * 4)
        .await
        .expect("normal native image must not discard the text notification");
    let id = reply.body().deserialize::<u32>().expect("notification id");
    let active = state
        .store
        .lock()
        .await
        .active_notification_view(id)
        .expect("notification should be retained");

    assert_eq!(active.summary, "summary");
    assert!(!active.image.has_image_data);
}

#[tokio::test]
async fn native_image_within_retained_limit_reaches_the_notification_model() {
    let (state, client) = notification_ingress().await;
    let reply = send_image_notification(&state, &client, 128, 128, 512, 128 * 128 * 4)
        .await
        .expect("bounded native image should reach the typed interface");
    let id = reply.body().deserialize::<u32>().expect("notification id");
    let active = state
        .store
        .lock()
        .await
        .active_notification_view(id)
        .expect("notification should be retained");

    assert!(active.image.has_image_data);
    assert_eq!(active.image.image_data.data.len(), 128 * 128 * 4);
}

#[tokio::test]
async fn bounded_unknown_variant_does_not_break_notification_delivery() {
    let (state, client) = notification_ingress().await;
    let hints = HashMap::from([("sender-pid".to_string(), OwnedValue::from(42_u32))]);
    let reply = send_owned_hints_notification(&state, &client, hints)
        .await
        .expect("bounded unknown hint should be ignored");
    let id = reply.body().deserialize::<u32>().expect("notification id");

    assert_eq!(id, 1);
    assert_eq!(state.store.lock().await.list_active().len(), 1);
}

#[tokio::test]
async fn supported_wire_hints_keep_text_boolean_and_both_urgency_types() {
    let (state, client) = notification_ingress().await;
    let category =
        OwnedValue::try_from(Value::from("im.received")).expect("owned category hint string");
    let first_hints = HashMap::from([
        ("category".to_string(), category),
        ("transient".to_string(), OwnedValue::from(true)),
        ("urgency".to_string(), OwnedValue::from(2_u8)),
    ]);
    let first_reply = send_owned_hints_notification(&state, &client, first_hints)
        .await
        .expect("supported byte urgency hints should reach the typed interface");
    let first_id = first_reply
        .body()
        .deserialize::<u32>()
        .expect("first notification id");
    let second_hints = HashMap::from([("urgency".to_string(), OwnedValue::from(1_u32))]);
    let second_reply = send_owned_hints_notification(&state, &client, second_hints)
        .await
        .expect("supported integer urgency hints should reach the typed interface");
    let second_id = second_reply
        .body()
        .deserialize::<u32>()
        .expect("second notification id");
    let store = state.store.lock().await;
    let first = store
        .active_notification_view(first_id)
        .expect("first notification should be retained");
    let second = store
        .active_notification_view(second_id)
        .expect("second notification should be retained");

    assert_eq!(first.category, "im.received");
    assert!(first.is_transient);
    assert_eq!(first.urgency, 2);
    assert_eq!(second.urgency, 1);
}

#[tokio::test]
async fn image_hint_aliases_follow_standard_precedence_independent_of_wire_order() {
    let (state, client) = notification_ingress().await;
    let all_aliases = HashMap::from([
        ("icon_data".to_string(), owned_rgba_pixel([3, 0, 0, 255])),
        ("image_data".to_string(), owned_rgba_pixel([2, 0, 0, 255])),
        ("image-data".to_string(), owned_rgba_pixel([1, 0, 0, 255])),
    ]);
    let standard_id = send_owned_hints_notification(&state, &client, all_aliases)
        .await
        .expect("standard image alias should decode")
        .body()
        .deserialize::<u32>()
        .expect("standard image notification id");
    let legacy_aliases = HashMap::from([
        ("icon_data".to_string(), owned_rgba_pixel([3, 0, 0, 255])),
        ("image_data".to_string(), owned_rgba_pixel([2, 0, 0, 255])),
    ]);
    let legacy_id = send_owned_hints_notification(&state, &client, legacy_aliases)
        .await
        .expect("legacy image alias should decode")
        .body()
        .deserialize::<u32>()
        .expect("legacy image notification id");
    let icon_only = HashMap::from([("icon_data".to_string(), owned_rgba_pixel([3, 0, 0, 255]))]);
    let icon_id = send_owned_hints_notification(&state, &client, icon_only)
        .await
        .expect("legacy icon alias should decode")
        .body()
        .deserialize::<u32>()
        .expect("legacy icon notification id");
    let store = state.store.lock().await;

    assert_eq!(
        store
            .active_notification_view(standard_id)
            .expect("standard image notification")
            .image
            .image_data
            .data,
        [1, 0, 0, 255]
    );
    assert_eq!(
        store
            .active_notification_view(legacy_id)
            .expect("legacy image notification")
            .image
            .image_data
            .data,
        [2, 0, 0, 255]
    );
    assert_eq!(
        store
            .active_notification_view(icon_id)
            .expect("legacy icon notification")
            .image
            .image_data
            .data,
        [3, 0, 0, 255]
    );
}

#[tokio::test]
async fn supported_hint_with_wrong_signature_is_rejected_without_daemon_failure() {
    let (state, client) = notification_ingress().await;
    let invalid_hints = [
        HashMap::from([("category".to_string(), OwnedValue::from(true))]),
        HashMap::from([(
            "transient".to_string(),
            OwnedValue::try_from(Value::from("yes")).expect("owned boolean mismatch"),
        )]),
        HashMap::from([(
            "urgency".to_string(),
            OwnedValue::try_from(Value::from("high")).expect("owned urgency mismatch"),
        )]),
        HashMap::from([(
            "image-data".to_string(),
            OwnedValue::try_from(Value::from("pixels")).expect("owned image mismatch"),
        )]),
    ];

    for hints in invalid_hints {
        let error = send_owned_hints_notification(&state, &client, hints)
            .await
            .expect_err("known hint with wrong signature must fail");
        assert!(
            error
                .to_string()
                .contains("notification hint has an unexpected D-Bus signature"),
            "unexpected mismatched-hint error: {error}"
        );
    }

    assert!(state.store.lock().await.list_active().is_empty());
    let recovery = send_owned_hints_notification(&state, &client, HashMap::new())
        .await
        .expect("valid notification should still work after rejected hints");
    assert_eq!(
        recovery
            .body()
            .deserialize::<u32>()
            .expect("recovery notification id"),
        1
    );
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

async fn send_image_notification(
    state: &crate::daemon::DaemonState,
    client: &Connection,
    width: i32,
    height: i32,
    rowstride: i32,
    image_bytes: usize,
) -> zbus::Result<Message> {
    let destination = state
        .connection()
        .unique_name()
        .expect("daemon unique name")
        .clone();
    let image = (
        width,
        height,
        rowstride,
        true,
        8_i32,
        4_i32,
        vec![0_u8; image_bytes],
    );
    let hints = HashMap::from([("image-data", SerializeValue(&image))]);
    let payload = (
        "app",
        0_u32,
        "",
        "summary",
        "body",
        Vec::<String>::new(),
        hints,
        0_i32,
    );

    tokio::time::timeout(
        TEST_NOTIFY_TIMEOUT,
        client.call_method(
            Some(destination),
            NOTIFICATIONS_OBJECT_PATH,
            Some(NOTIFICATIONS_INTERFACE),
            "Notify",
            &payload,
        ),
    )
    .await
    .expect("Notify response timed out")
}

async fn send_owned_hints_notification(
    state: &crate::daemon::DaemonState,
    client: &Connection,
    hints: HashMap<String, OwnedValue>,
) -> zbus::Result<Message> {
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
        "body",
        Vec::<String>::new(),
        hints,
        0_i32,
    );

    tokio::time::timeout(
        TEST_NOTIFY_TIMEOUT,
        client.call_method(
            Some(destination),
            NOTIFICATIONS_OBJECT_PATH,
            Some(NOTIFICATIONS_INTERFACE),
            "Notify",
            &payload,
        ),
    )
    .await
    .expect("Notify response timed out")
}

fn owned_rgba_pixel(data: [u8; 4]) -> OwnedValue {
    let image = Structure::from((1_i32, 1_i32, 4_i32, true, 8_i32, 4_i32, data.to_vec()));
    OwnedValue::try_from(Value::from(image)).expect("owned one-pixel image hint")
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
