use crate::doctor::report::DoctorSeverity;
use unixnotis_core::{CONTROL_BUS_NAME, CONTROL_OBJECT_PATH, NOTIFICATIONS_BUS_NAME};
use zbus::ConnectionBuilder;

use super::super::inspect_bus_connection;
use super::support::{check_ids, connect, control_server, run_async, PrivateBroker, TestControl};

#[test]
fn no_bus_owners_preserve_the_complete_readiness_failure_sequence() {
    run_async(async {
        let broker = PrivateBroker::start();
        let client = connect(&broker.address).await;

        let result = inspect_bus_connection(&client).await;

        assert!(!result.control_owned);
        assert_eq!(
            check_ids(&result),
            [
                "dbus.session",
                "dbus.identity",
                "dbus.notifications-owner",
                "dbus.notifications-readiness",
                "dbus.control-owner",
                "dbus.control-state",
            ]
        );
    });
}

#[test]
fn notification_owner_without_control_owner_keeps_control_failure_last() {
    run_async(async {
        let broker = PrivateBroker::start();
        let _notifications = ConnectionBuilder::address(broker.address.as_str())
            .expect("parse private broker address")
            .name(NOTIFICATIONS_BUS_NAME)
            .expect("request notification bus name")
            .build()
            .await
            .expect("connect notification service");
        let client = connect(&broker.address).await;

        let result = inspect_bus_connection(&client).await;

        assert_eq!(
            check_ids(&result),
            [
                "dbus.session",
                "dbus.identity",
                "dbus.notifications-owner",
                "dbus.control-owner",
                "dbus.control-state",
            ]
        );
    });
}

#[test]
fn notification_gap_does_not_hide_healthy_control_checks() {
    run_async(async {
        let broker = PrivateBroker::start();
        let _control = control_server(&broker.address, false, false).await;
        let client = connect(&broker.address).await;

        let result = inspect_bus_connection(&client).await;

        assert_eq!(
            check_ids(&result),
            [
                "dbus.session",
                "dbus.identity",
                "dbus.notifications-owner",
                "dbus.notifications-readiness",
                "dbus.control-owner",
                "dbus.control-proxy",
                "dbus.control-state",
                "dbus.ui-health",
            ]
        );
    });
}

#[test]
fn different_owners_preserve_the_shared_owner_error_and_check_order() {
    run_async(async {
        let broker = PrivateBroker::start();
        let _control = ConnectionBuilder::address(broker.address.as_str())
            .expect("parse private broker address")
            .name(CONTROL_BUS_NAME)
            .expect("request control bus name")
            .serve_at(CONTROL_OBJECT_PATH, TestControl { deny_state: false })
            .expect("register test control interface")
            .build()
            .await
            .expect("connect control service");
        let _notifications = ConnectionBuilder::address(broker.address.as_str())
            .expect("parse private broker address")
            .name(NOTIFICATIONS_BUS_NAME)
            .expect("request notification bus name")
            .build()
            .await
            .expect("connect separate notification service");
        let client = connect(&broker.address).await;

        let result = inspect_bus_connection(&client).await;
        assert_eq!(
            check_ids(&result),
            [
                "dbus.session",
                "dbus.identity",
                "dbus.notifications-owner",
                "dbus.control-owner",
                "dbus.shared-owner",
                "dbus.control-proxy",
                "dbus.control-state",
                "dbus.ui-health",
            ]
        );
        let ownership = result
            .checks
            .iter()
            .find(|check| check.id == "dbus.shared-owner")
            .expect("shared owner check");
        assert_eq!(ownership.severity, DoctorSeverity::Error);
    });
}
