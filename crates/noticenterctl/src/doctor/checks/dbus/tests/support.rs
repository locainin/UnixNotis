use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use unixnotis_core::{
    ControlState, UiHealth, CONTROL_BUS_NAME, CONTROL_OBJECT_PATH, NOTIFICATIONS_BUS_NAME,
};
use zbus::ConnectionBuilder;

static NEXT_BROKER: AtomicUsize = AtomicUsize::new(0);

pub(super) struct PrivateBroker {
    child: Child,
    socket: PathBuf,
    pub(super) address: String,
}

impl PrivateBroker {
    pub(super) fn start() -> Self {
        let socket = broker_socket();
        let listen_address = format!("unix:path={}", socket.display());
        // Resolve from protected roots because other tests may temporarily replace PATH
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
            .expect("start private D-Bus broker");
        let stdout = child.stdout.take().expect("capture private broker address");
        let mut address = String::new();
        BufReader::new(stdout)
            .read_line(&mut address)
            .expect("read private broker address");
        assert!(
            address.trim().starts_with(&listen_address),
            "private broker must listen on the requested socket"
        );
        Self {
            child,
            socket,
            address: address.trim().to_string(),
        }
    }
}

impl Drop for PrivateBroker {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_file(&self.socket);
        if let Some(parent) = self.socket.parent() {
            let _ = std::fs::remove_dir(parent);
        }
    }
}

pub(super) struct TestControl {
    pub(super) deny_state: bool,
}

#[zbus::interface(name = "com.unixnotis.Control")]
impl TestControl {
    fn get_state(&self) -> zbus::fdo::Result<ControlState> {
        if self.deny_state {
            return Err(zbus::fdo::Error::AccessDenied(
                "test client denied".to_string(),
            ));
        }
        Ok(ControlState {
            dnd_enabled: true,
            dnd_expires_at: 0,
            history_count: 4,
            inhibited: false,
            inhibitor_count: 2,
        })
    }

    fn get_ui_health(&self) -> zbus::fdo::Result<UiHealth> {
        Ok(UiHealth {
            center_process_running: true,
            center_ready: true,
            popups_process_running: true,
            popups_ready: true,
            revision: 0,
        })
    }
}

pub(super) fn run_async(future: impl std::future::Future<Output = ()>) {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build D-Bus test runtime")
        .block_on(future);
}

pub(super) async fn connect(address: &str) -> zbus::Connection {
    ConnectionBuilder::address(address)
        .expect("parse private broker address")
        .build()
        .await
        .expect("connect to private broker")
}

pub(super) async fn control_server(
    address: &str,
    deny_state: bool,
    own_notifications: bool,
) -> zbus::Connection {
    let connection = ConnectionBuilder::address(address)
        .expect("parse private broker address")
        .name(CONTROL_BUS_NAME)
        .expect("request control bus name")
        .serve_at(CONTROL_OBJECT_PATH, TestControl { deny_state })
        .expect("register test control interface")
        .build()
        .await
        .expect("connect test control service");
    if own_notifications {
        connection
            .request_name(NOTIFICATIONS_BUS_NAME)
            .await
            .expect("request notification bus name");
    }
    connection
}

pub(super) fn check_ids(result: &super::super::DoctorBusResult) -> Vec<&str> {
    result
        .checks
        .iter()
        .map(|check| check.id.as_str())
        .collect()
}

fn broker_socket() -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock must be after the Unix epoch")
        .as_nanos();
    let serial = NEXT_BROKER.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "unixnotis-doctor-dbus-{}-{stamp}-{serial}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("create private broker directory");
    root.join("bus.sock")
}
