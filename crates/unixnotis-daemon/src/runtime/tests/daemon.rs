use clap::Parser;
use zbus::fdo::DBusProxy;

use super::{run_daemon, skip_ui_for_zero_duration};
use crate::cli::Args;
use unixnotis_core::{Config, CONTROL_BUS_NAME, NOTIFICATIONS_BUS_NAME};

#[test]
fn only_zero_duration_runs_skip_ui_startup() {
    assert!(skip_ui_for_zero_duration(Some(0)));
    assert!(!skip_ui_for_zero_duration(Some(1)));
    assert!(!skip_ui_for_zero_duration(None));
}

#[tokio::test]
async fn zero_duration_startup_registers_services_without_ui_children() {
    let args = Args::try_parse_from(["unixnotis-daemon", "--run-seconds", "0"])
        .expect("parse zero-duration daemon command");
    let connection = zbus::Connection::session()
        .await
        .expect("connect daemon to session bus");
    let proxy = DBusProxy::new(&connection)
        .await
        .expect("create daemon bus proxy");
    let notifications_name = zbus::names::BusName::try_from(NOTIFICATIONS_BUS_NAME)
        .expect("notification bus name should be valid");

    run_daemon(
        &args,
        Config::default(),
        &connection,
        &proxy,
        notifications_name.clone(),
    )
    .await
    .expect("zero-duration daemon startup should succeed");

    assert!(proxy
        .name_has_owner(notifications_name)
        .await
        .expect("query notification bus owner"));
    assert!(proxy
        .name_has_owner(
            zbus::names::BusName::try_from(CONTROL_BUS_NAME)
                .expect("control bus name should be valid"),
        )
        .await
        .expect("query control bus owner"));
}
