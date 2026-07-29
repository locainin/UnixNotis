use std::collections::HashMap;
use std::time::{Duration, Instant};

use clap::Parser;
use futures_util::StreamExt;
use unixnotis_core::{ControlProxy, NotificationsProxy, CONTROL_BUS_NAME, NOTIFICATIONS_BUS_NAME};
use zbus::fdo::DBusProxy;
use zbus::names::BusName;
use zbus::zvariant::OwnedValue;
use zbus::{Connection, ConnectionBuilder};

use super::super::{run_with_builder, run_with_builder_inner};
use crate::cli::Args;
use unixnotis_core::Config;

#[path = "dbus_lifecycle/private_bus.rs"]
mod private_bus;

use private_bus::PrivateBus;

async fn connect(address: &str) -> Connection {
    ConnectionBuilder::address(address)
        .expect("parse private broker address")
        .build()
        .await
        .expect("connect to private broker")
}

fn spawn_daemon(address: String, run_seconds: u64) -> tokio::task::JoinHandle<anyhow::Result<()>> {
    tokio::spawn(async move {
        let args = Args::try_parse_from([
            "unixnotis-daemon",
            "--run-seconds",
            &run_seconds.to_string(),
        ])
        .expect("parse bounded daemon command");
        let builder = zbus::connection::Builder::address(address.as_str())
            .expect("parse daemon broker address");
        Box::pin(run_with_builder(&args, Config::default(), builder)).await
    })
}

fn spawn_daemon_with_trusted_sender(
    address: String,
    run_seconds: u64,
    trusted_sender: String,
) -> tokio::task::JoinHandle<anyhow::Result<()>> {
    tokio::spawn(async move {
        let args = Args::try_parse_from([
            "unixnotis-daemon",
            "--run-seconds",
            &run_seconds.to_string(),
        ])
        .expect("parse bounded daemon command");
        let builder = zbus::connection::Builder::address(address.as_str())
            .expect("parse daemon broker address");
        Box::pin(run_with_builder_inner(
            &args,
            Config::default(),
            builder,
            Some(trusted_sender),
        ))
        .await
    })
}

async fn owner(dbus: &DBusProxy<'_>, name: &'static str) -> Option<String> {
    let name = BusName::try_from(name).expect("static bus name");
    dbus.get_name_owner(name)
        .await
        .ok()
        .map(|owner| owner.to_string())
}

async fn wait_for_both_owners(connection: &Connection) -> (String, String) {
    let dbus = DBusProxy::new(connection)
        .await
        .expect("create broker proxy");
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if let (Some(notifications), Some(control)) = (
                owner(&dbus, NOTIFICATIONS_BUS_NAME).await,
                owner(&dbus, CONTROL_BUS_NAME).await,
            ) {
                return (notifications, control);
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("daemon should acquire both names")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn startup_publishes_both_names_with_one_ready_owner() {
    let bus = PrivateBus::start();
    let client = connect(&bus.address).await;
    let daemon = spawn_daemon(bus.address.clone(), 1);

    let (notifications_owner, control_owner) = wait_for_both_owners(&client).await;
    assert_eq!(
        notifications_owner, control_owner,
        "both service names must belong to the ready daemon connection"
    );
    let notifications = NotificationsProxy::new(&client)
        .await
        .expect("create notifications proxy");
    let capabilities_started = Instant::now();
    tokio::time::timeout(Duration::from_secs(2), notifications.get_capabilities())
        .await
        .expect("GetCapabilities must be bounded")
        .expect("GetCapabilities must succeed");
    assert!(
        capabilities_started.elapsed() < Duration::from_millis(500),
        "GetCapabilities exceeded the shared-runner latency budget"
    );
    let information_started = Instant::now();
    let server = tokio::time::timeout(
        Duration::from_secs(2),
        notifications.get_server_information(),
    )
    .await
    .expect("GetServerInformation must be bounded")
    .expect("GetServerInformation must succeed");
    assert_eq!(server.0, "UnixNotis");
    assert!(
        information_started.elapsed() < Duration::from_millis(500),
        "GetServerInformation exceeded the shared-runner latency budget"
    );

    let cold_started = Instant::now();
    tokio::time::timeout(
        Duration::from_secs(2),
        notifications.notify(
            "Lifecycle test",
            0,
            "",
            "Cold notification",
            "First attribution lookup",
            Vec::new(),
            HashMap::new(),
            1_000,
        ),
    )
    .await
    .expect("cold Notify must be bounded")
    .expect("cold Notify must succeed");
    assert!(
        cold_started.elapsed() < Duration::from_secs(1),
        "cold Notify exceeded the shared-runner latency budget"
    );
    let warm_started = Instant::now();
    tokio::time::timeout(
        Duration::from_secs(2),
        notifications.notify(
            "Lifecycle test",
            0,
            "",
            "Warm notification",
            "Cached sender metadata",
            Vec::new(),
            HashMap::new(),
            1_000,
        ),
    )
    .await
    .expect("warm Notify must be bounded")
    .expect("warm Notify must succeed");
    assert!(
        warm_started.elapsed() < Duration::from_millis(500),
        "warm Notify exceeded the shared-runner latency budget"
    );

    let control = ControlProxy::new(&client)
        .await
        .expect("create control proxy");
    tokio::time::timeout(Duration::from_secs(2), control.get_state())
        .await
        .expect("GetState must be bounded")
        .expect("GetState must succeed");
    daemon
        .await
        .expect("join daemon task")
        .expect("bounded daemon run");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn private_session_bus_accepts_full_notification_view_after_added_signal() {
    let bus = PrivateBus::start();
    let client = connect(&bus.address).await;
    let trusted_sender = client
        .unique_name()
        .expect("private session bus assigns a unique client name")
        .to_string();
    let daemon = spawn_daemon_with_trusted_sender(bus.address.clone(), 3, trusted_sender);
    let owners_before = wait_for_both_owners(&client).await;
    let control = ControlProxy::new(&client)
        .await
        .expect("create control proxy");
    let mut added = control
        .receive_notification_added()
        .await
        .expect("subscribe before sending notification");
    let notifications = NotificationsProxy::new(&client)
        .await
        .expect("create notifications proxy");

    let id = notifications
        .notify(
            "Private bus wire test",
            0,
            "",
            "Complete notification view",
            "The private bus must accept the nested enum payload",
            Vec::new(),
            HashMap::from([("urgency".to_string(), OwnedValue::from(2_u8))]),
            2_000,
        )
        .await
        .expect("Notify should return an assigned id");
    let signal = tokio::time::timeout(Duration::from_secs(2), added.next())
        .await
        .expect("NotificationAdded must arrive promptly")
        .expect("NotificationAdded stream must remain open");
    let signal_args = signal.args().expect("decode NotificationAdded arguments");
    assert_eq!(*signal_args.id(), id);

    // This is the exact authorized pull that previously exposed an invalid D-Bus body
    let views = control
        .get_active_notification(id)
        .await
        .expect("GetActiveNotification must return a valid D-Bus body");
    assert_eq!(views.len(), 1);
    assert_eq!(views[0].id, id);
    assert_eq!(views[0].summary, "Complete notification view");
    assert_eq!(views[0].urgency, 2);
    let popup_candidates = control
        .list_popup_candidates()
        .await
        .expect("ListPopupCandidates must return a valid D-Bus body");
    assert_eq!(popup_candidates.len(), 1);
    assert_eq!(popup_candidates[0].id, id);
    assert!(
        !daemon.is_finished(),
        "serializing NotificationView must not disconnect the daemon"
    );
    assert_eq!(wait_for_both_owners(&client).await, owners_before);

    daemon
        .await
        .expect("join daemon task")
        .expect("bounded daemon run");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn session_bus_loss_makes_the_daemon_exit_with_failure() {
    let mut bus = PrivateBus::start();
    let client = connect(&bus.address).await;
    let daemon = spawn_daemon(bus.address.clone(), 30);
    let _owners = wait_for_both_owners(&client).await;

    bus.terminate();
    let result = tokio::time::timeout(Duration::from_secs(8), daemon)
        .await
        .expect("daemon must notice session bus loss")
        .expect("join daemon task");
    assert!(
        result.is_err(),
        "session bus loss must return a daemon failure"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn notification_during_health_probing_keeps_daemon_generation_alive() {
    let bus = PrivateBus::start();
    let client = connect(&bus.address).await;
    let daemon = spawn_daemon(bus.address.clone(), 4);
    let owners_before = wait_for_both_owners(&client).await;
    let notifications = NotificationsProxy::new(&client)
        .await
        .expect("create notifications proxy");

    // Cross the first one-second health interval before committing the notification
    tokio::time::sleep(Duration::from_millis(1_100)).await;
    let id = notifications
        .notify(
            "Health overlap test",
            0,
            "",
            "Notification during probe",
            "The daemon generation must remain alive",
            Vec::new(),
            HashMap::new(),
            3_000,
        )
        .await
        .expect("Notify should return an assigned id");
    assert_ne!(id, 0);

    tokio::time::sleep(Duration::from_millis(1_100)).await;
    assert!(
        !daemon.is_finished(),
        "one healthy probe interval must not retire the daemon generation"
    );
    let owners_after = wait_for_both_owners(&client).await;
    assert_eq!(owners_after, owners_before);

    daemon
        .await
        .expect("join daemon task")
        .expect("bounded daemon run");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn competing_notification_owner_prevents_control_publication() {
    let bus = PrivateBus::start();
    let competitor = connect(&bus.address).await;
    competitor
        .request_name(NOTIFICATIONS_BUS_NAME)
        .await
        .expect("competitor owns notification name");
    let observer = connect(&bus.address).await;
    let daemon = spawn_daemon(bus.address.clone(), 5);

    let result = tokio::time::timeout(Duration::from_secs(10), daemon)
        .await
        .expect("competing owner should fail startup promptly")
        .expect("join daemon task");
    let error = result.expect_err("competing notification owner must fail startup");
    assert!(
        error
            .to_string()
            .contains("already owned and unavailable to this process"),
        "unexpected competing-owner error: {error:#}"
    );
    let dbus = DBusProxy::new(&observer)
        .await
        .expect("create observer proxy");
    assert!(
        owner(&dbus, CONTROL_BUS_NAME).await.is_none(),
        "control readiness must never publish after notification ownership fails"
    );
}
