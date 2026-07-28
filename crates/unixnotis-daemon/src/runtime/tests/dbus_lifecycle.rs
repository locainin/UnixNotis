use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

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

static NEXT_BROKER: AtomicUsize = AtomicUsize::new(0);

struct PrivateBroker {
    child: Child,
    socket: PathBuf,
    address: String,
}

impl PrivateBroker {
    fn start() -> Self {
        let socket = broker_socket();
        let address = format!("unix:path={}", socket.display());
        let socket_activate =
            unixnotis_core::util::trusted_system_program_path("systemd-socket-activate")
                .expect("find trusted systemd-socket-activate");
        let broker = unixnotis_core::util::trusted_system_program_path("dbus-broker-launch")
            .expect("find trusted dbus-broker-launch");
        let runtime_dir = std::env::var("XDG_RUNTIME_DIR")
            .expect("private dbus-broker tests require XDG_RUNTIME_DIR");
        let mut command = Command::new(socket_activate);
        command
            .arg("--now")
            .arg("--setenv")
            .arg(format!("XDG_RUNTIME_DIR={runtime_dir}"));
        if let Ok(bus_address) = std::env::var("DBUS_SESSION_BUS_ADDRESS") {
            // The launcher uses the existing user bus only for systemd activation control
            command
                .arg("--setenv")
                .arg(format!("DBUS_SESSION_BUS_ADDRESS={bus_address}"));
        }
        let mut child = command
            .arg("--listen")
            .arg(&socket)
            .arg("--fdname=dbus.socket")
            .arg(broker)
            .args(["--scope", "user"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("start private dbus-broker");

        // Socket activation creates the isolated listener before clients are allowed to connect
        let deadline = Instant::now() + Duration::from_secs(2);
        while !socket.exists() && Instant::now() < deadline {
            assert!(
                child
                    .try_wait()
                    .expect("query private broker process")
                    .is_none(),
                "private dbus-broker exited before creating its socket"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(
            socket.exists(),
            "private dbus-broker must create its isolated socket"
        );
        Self {
            child,
            socket,
            address,
        }
    }

    fn terminate(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for PrivateBroker {
    fn drop(&mut self) {
        self.terminate();
        let _ = std::fs::remove_file(&self.socket);
        if let Some(parent) = self.socket.parent() {
            let _ = std::fs::remove_dir(parent);
        }
    }
}

fn broker_socket() -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock must be after the Unix epoch")
        .as_nanos();
    let serial = NEXT_BROKER.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "unixnotis-runtime-dbus-{}-{stamp}-{serial}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("create private broker directory");
    root.join("bus.sock")
}

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
    let broker = PrivateBroker::start();
    let client = connect(&broker.address).await;
    let daemon = spawn_daemon(broker.address.clone(), 1);

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
async fn strict_broker_accepts_full_notification_view_after_added_signal() {
    let broker = PrivateBroker::start();
    let client = connect(&broker.address).await;
    let trusted_sender = client
        .unique_name()
        .expect("private broker assigns a unique client name")
        .to_string();
    let daemon = spawn_daemon_with_trusted_sender(broker.address.clone(), 3, trusted_sender);
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
            "Strict broker wire test",
            0,
            "",
            "Complete notification view",
            "The strict broker must accept the nested enum payload",
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

    // This is the exact authorized pull that previously made dbus-broker reject the body
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
async fn broker_loss_makes_the_daemon_exit_with_failure() {
    let mut broker = PrivateBroker::start();
    let client = connect(&broker.address).await;
    let daemon = spawn_daemon(broker.address.clone(), 30);
    let _owners = wait_for_both_owners(&client).await;

    broker.terminate();
    let result = tokio::time::timeout(Duration::from_secs(8), daemon)
        .await
        .expect("daemon must notice broker loss")
        .expect("join daemon task");
    assert!(result.is_err(), "broker loss must return a daemon failure");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn notification_during_health_probing_keeps_daemon_generation_alive() {
    let broker = PrivateBroker::start();
    let client = connect(&broker.address).await;
    let daemon = spawn_daemon(broker.address.clone(), 4);
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
    let broker = PrivateBroker::start();
    let competitor = connect(&broker.address).await;
    competitor
        .request_name(NOTIFICATIONS_BUS_NAME)
        .await
        .expect("competitor owns notification name");
    let observer = connect(&broker.address).await;
    let daemon = spawn_daemon(broker.address.clone(), 5);

    let result = tokio::time::timeout(Duration::from_secs(10), daemon)
        .await
        .expect("competing owner should fail startup promptly")
        .expect("join daemon task");
    assert!(
        result.is_err(),
        "competing notification owner must fail startup"
    );
    let dbus = DBusProxy::new(&observer)
        .await
        .expect("create observer proxy");
    assert!(
        owner(&dbus, CONTROL_BUS_NAME).await.is_none(),
        "control readiness must never publish after notification ownership fails"
    );
}
