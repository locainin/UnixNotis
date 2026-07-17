use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use super::super::dbus::*;
use crate::doctor::report::DoctorSeverity;
use unixnotis_core::{ControlState, CONTROL_BUS_NAME, CONTROL_OBJECT_PATH, NOTIFICATIONS_BUS_NAME};
use zbus::ConnectionBuilder;

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
        // Other tests may temporarily replace PATH, so resolve the broker from fixed system roots
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

struct TestControl {
    deny_state: bool,
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
            history_count: 4,
            inhibited: false,
            inhibitor_count: 2,
        })
    }
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

async fn connect(address: &str) -> zbus::Connection {
    ConnectionBuilder::address(address)
        .expect("parse private broker address")
        .build()
        .await
        .expect("connect to private broker")
}

async fn control_server(address: &str, deny_state: bool) -> zbus::Connection {
    let connection = ConnectionBuilder::address(address)
        .expect("parse private broker address")
        .name(CONTROL_BUS_NAME)
        .expect("request control bus name")
        .serve_at(CONTROL_OBJECT_PATH, TestControl { deny_state })
        .expect("register test control interface")
        .build()
        .await
        .expect("connect test control service");
    connection
        .request_name(NOTIFICATIONS_BUS_NAME)
        .await
        .expect("request notification bus name");
    connection
}

#[test]
fn unavailable_bus_preserves_an_error_and_dependent_check_context() {
    let result = unavailable_bus_result("connection refused".to_string());

    assert!(!result.control_owned);
    assert_eq!(result.checks.len(), 2);
    assert_eq!(result.checks[0].severity, DoctorSeverity::Error);
    assert_eq!(result.checks[1].severity, DoctorSeverity::Note);
}

#[test]
fn access_denied_state_failure_explains_installed_client_requirements() {
    let error = zbus::Error::FDO(Box::new(zbus::fdo::Error::AccessDenied(
        "caller is not authorized for control operation".to_string(),
    )));

    let check = control_state_failure_check(&error);

    assert_eq!(check.id, "dbus.control-state");
    assert_eq!(check.severity, DoctorSeverity::Error);
    assert_eq!(check.summary, "UnixNotis control access denied");
    assert_eq!(
        check.details.as_deref(),
        Some("The running daemon rejected this client")
    );
    assert!(check
        .hint
        .as_deref()
        .is_some_and(|hint| hint.contains("installed noticenterctl")));
    assert!(!check
        .details
        .as_deref()
        .is_some_and(|details| details.contains("caller is not authorized")));
}

#[test]
fn non_authorization_state_failure_preserves_the_original_error() {
    let error = zbus::Error::Failure("state unavailable".to_string());

    let check = control_state_failure_check(&error);

    assert_eq!(check.summary, "GetState failed");
    assert_eq!(check.details.as_deref(), Some("state unavailable"));
    assert!(check.hint.is_none());
}

#[test]
fn missing_bus_owners_report_both_readiness_failures() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build D-Bus test runtime");
    runtime.block_on(async {
        let broker = PrivateBroker::start();
        let client = connect(&broker.address).await;

        let result = inspect_bus_connection(&client).await;

        assert!(!result.control_owned);
        assert!(result.checks.iter().any(|check| {
            check.id == "dbus.notifications-readiness" && check.severity == DoctorSeverity::Error
        }));
        assert!(result.checks.iter().any(|check| {
            check.id == "dbus.control-state"
                && check.summary == "UnixNotis control service has no owner"
        }));
    });
}

#[test]
fn owned_control_service_runs_proxy_and_state_checks() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build D-Bus test runtime");
    runtime.block_on(async {
        let broker = PrivateBroker::start();
        let _server = control_server(&broker.address, false).await;
        let client = connect(&broker.address).await;

        let result = inspect_bus_connection(&client).await;

        assert!(result.control_owned);
        assert!(result.checks.iter().any(|check| {
            check.id == "dbus.control-proxy" && check.severity == DoctorSeverity::Pass
        }));
        let state = result
            .checks
            .iter()
            .find(|check| check.id == "dbus.control-state")
            .expect("control state check");
        assert_eq!(state.severity, DoctorSeverity::Pass);
        assert_eq!(state.summary, "GetState completed");
        assert!(state
            .details
            .as_deref()
            .is_some_and(|details| details.contains("History entries: 4")));
    });
}

#[test]
fn method_error_access_denial_uses_the_specific_client_guidance() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build D-Bus test runtime");
    runtime.block_on(async {
        let broker = PrivateBroker::start();
        let _server = control_server(&broker.address, true).await;
        let client = connect(&broker.address).await;

        let result = inspect_bus_connection(&client).await;

        assert!(result.control_owned);
        let state = result
            .checks
            .iter()
            .find(|check| check.id == "dbus.control-state")
            .expect("control state check");
        assert_eq!(state.severity, DoctorSeverity::Error);
        assert_eq!(state.summary, "UnixNotis control access denied");
        assert_eq!(
            state.details.as_deref(),
            Some("The running daemon rejected this client")
        );
    });
}
