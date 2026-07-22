use rustix::process::geteuid;
use zbus::names::BusName;
use zbus::Connection;

use super::credentials::{connection_credentials, CallerCredentials};
#[cfg(target_os = "linux")]
use super::process_identity::read_pidfd_process_id;

#[tokio::test]
async fn connection_credentials_match_the_current_bus_process() {
    let connection = Connection::session().await.expect("session bus connection");
    let sender = connection
        .unique_name()
        .expect("bus connection should have a unique name")
        .as_str()
        .to_string();
    let bus_name = BusName::try_from(sender.as_str()).expect("valid unique bus name");

    let credentials = connection_credentials(&connection, bus_name)
        .await
        .expect("message bus should return caller credentials");

    assert_eq!(credentials.unix_user_id(), Some(geteuid().as_raw()));
    assert_eq!(credentials.process_id(), Some(std::process::id()));

    // Linux authorization requires this stable handle from the same credential snapshot
    #[cfg(target_os = "linux")]
    assert_eq!(
        read_pidfd_process_id(
            credentials
                .process_fd()
                .expect("Linux session bus must provide ProcessFD")
        ),
        Some(std::process::id())
    );
}

#[cfg(target_os = "linux")]
#[test]
fn caller_credentials_expose_a_supplied_stable_process_handle() {
    use rustix::process::{pidfd_open, Pid, PidfdFlags};

    let raw_pid = i32::try_from(std::process::id()).expect("process id should fit i32");
    let pid = Pid::from_raw(raw_pid).expect("process id should be positive");
    let pidfd = pidfd_open(pid, PidfdFlags::empty()).expect("current pidfd should open");
    let credentials = CallerCredentials {
        process_fd: Some(pidfd.into()),
        ..CallerCredentials::default()
    };

    let process_fd = credentials
        .process_fd()
        .expect("supplied process handle should remain available");
    assert_eq!(read_pidfd_process_id(process_fd), Some(std::process::id()));
}
