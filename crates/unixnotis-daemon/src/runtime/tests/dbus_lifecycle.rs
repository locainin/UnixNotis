use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use clap::Parser;
use unixnotis_core::{ControlProxy, NotificationsProxy, CONTROL_BUS_NAME, NOTIFICATIONS_BUS_NAME};
use zbus::fdo::DBusProxy;
use zbus::names::BusName;
use zbus::{Connection, ConnectionBuilder};

use super::super::run_with_builder;
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
        let listen_address = format!("unix:path={}", socket.display());
        let daemon = unixnotis_core::util::trusted_system_program_path("dbus-daemon")
            .expect("find trusted dbus-daemon");
        let mut child = Command::new(daemon)
            .args([
                "--session",
                "--nofork",
                "--nopidfile",
                "--print-address=1",
                &format!("--address={listen_address}"),
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("start private D-Bus broker");
        let stdout = child.stdout.take().expect("capture broker address");
        let mut address = String::new();
        BufReader::new(stdout)
            .read_line(&mut address)
            .expect("read broker address");
        assert!(
            address.trim().starts_with(&listen_address),
            "broker must listen on the isolated test socket"
        );
        Self {
            child,
            socket,
            address: address.trim().to_string(),
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
