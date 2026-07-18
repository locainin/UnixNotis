use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use futures_util::StreamExt;
use zbus::fdo::DBusProxy;
use zbus::ConnectionBuilder;

use crate::test_support::broker::read_broker_address;

static NEXT_BROKER: AtomicUsize = AtomicUsize::new(0);

struct PrivateBroker {
    child: Child,
    socket: PathBuf,
    address: String,
}

impl PrivateBroker {
    fn start(socket: PathBuf) -> Self {
        let listen_address = format!("unix:path={}", socket.display());
        // Parallel tests can alter PATH, so the private broker uses fixed trusted roots
        let daemon = unixnotis_core::util::trusted_system_program_path("dbus-daemon")
            .expect("find dbus-daemon in a trusted system directory");
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
            .expect("start private dbus-daemon");
        let stdout = child.stdout.take().expect("capture broker address");
        let address = read_broker_address(&mut child, stdout, &listen_address)
            .expect("read private broker address promptly");
        Self {
            child,
            socket,
            address,
        }
    }

    fn stop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for PrivateBroker {
    fn drop(&mut self) {
        self.stop();
        let _ = std::fs::remove_file(&self.socket);
        if let Some(parent) = self.socket.parent() {
            let _ = std::fs::remove_dir(parent);
        }
    }
}

fn broker_socket() -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    let serial = NEXT_BROKER.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "unixnotis-dbus-reconnect-{}-{stamp}-{serial}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("create broker directory");
    root.join("bus.sock")
}

async fn connect(address: &str) -> zbus::Result<zbus::Connection> {
    ConnectionBuilder::address(address)?.build().await
}

async fn wait_for_replacement(address: &str) -> zbus::Connection {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    loop {
        if let Ok(connection) = connect(address).await {
            return connection;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "replacement broker did not accept a new connection"
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

#[test]
fn closed_command_channel_stops_before_attempting_a_bus_connection() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build test runtime");
    runtime.block_on(async {
        let (event_tx, _event_rx) = async_channel::bounded(1);
        let (command_tx, command_rx) = tokio::sync::mpsc::channel(1);
        drop(command_tx);

        tokio::time::timeout(
            Duration::from_millis(100),
            super::run_control_loop(event_tx, command_rx),
        )
        .await
        .expect("closed control loop should stop without waiting for D-Bus");
    });
}

#[test]
fn new_connection_recovers_after_private_session_bus_restart() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build test runtime");
    runtime.block_on(async {
        let socket = broker_socket();
        let mut first_broker = PrivateBroker::start(socket.clone());
        let first = connect(&first_broker.address)
            .await
            .expect("connect first broker");
        let proxy = DBusProxy::new(&first).await.expect("create first proxy");
        let mut owner_changes = proxy
            .receive_name_owner_changed()
            .await
            .expect("subscribe first broker");

        first_broker.stop();
        let ended = tokio::time::timeout(Duration::from_secs(2), owner_changes.next())
            .await
            .expect("dead broker should terminate its signal stream");
        assert!(ended.is_none());

        let second_broker = PrivateBroker::start(socket);
        let replacement = wait_for_replacement(&second_broker.address).await;
        let replacement_proxy = DBusProxy::new(&replacement)
            .await
            .expect("create replacement proxy");
        let names = replacement_proxy
            .list_names()
            .await
            .expect("query replacement broker");
        assert!(!names.is_empty());
    });
}

#[test]
fn broker_socket_is_scoped_to_a_unique_temporary_directory() {
    let first = broker_socket();
    let second = broker_socket();

    assert_ne!(first, second);
    assert_eq!(first.file_name(), Some(std::ffi::OsStr::new("bus.sock")));
    let _ = std::fs::remove_dir_all(first.parent().expect("temporary socket has a parent"));
    let _ = std::fs::remove_dir_all(second.parent().expect("temporary socket has a parent"));
}
