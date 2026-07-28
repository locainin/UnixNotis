use crate::doctor::report::DoctorSeverity;
use unixnotis_core::CONTROL_BUS_NAME;
use zbus::fdo::DBusProxy;
use zbus::names::BusName;
use zbus::ConnectionBuilder;

use super::super::{control, inspect_bus_connection, owners};
use super::support::{check_ids, connect, control_server, run_async, PrivateBroker};

#[test]
fn same_owner_and_healthy_ui_preserve_the_complete_check_sequence() {
    run_async(async {
        let broker = PrivateBroker::start();
        let _server = control_server(&broker.address, false, true).await;
        let client = connect(&broker.address).await;

        let result = inspect_bus_connection(&client).await;

        assert!(result.control_owned);
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
        let health = result
            .checks
            .iter()
            .find(|check| check.id == "dbus.ui-health")
            .expect("UI health check");
        assert_eq!(health.severity, DoctorSeverity::Pass);
        assert!(health
            .details
            .as_deref()
            .is_some_and(|details| details.contains("Popup D-Bus/GTK client: ready")));
        assert!(!health
            .details
            .as_deref()
            .is_some_and(|details| details.contains("Popup GTK runtime:")));
    });
}

#[test]
fn access_denied_control_state_reports_mismatched_installation_guidance() {
    run_async(async {
        let broker = PrivateBroker::start();
        let _server = control_server(&broker.address, true, true).await;
        let client = connect(&broker.address).await;

        let result = inspect_bus_connection(&client).await;
        let state = result
            .checks
            .iter()
            .find(|check| check.id == "dbus.control-state")
            .expect("control state check");

        assert_eq!(state.severity, DoctorSeverity::Error);
        assert_eq!(state.summary, "UnixNotis control access denied");
    });
}

#[test]
fn control_owner_loss_between_probe_and_get_state_is_reported() {
    run_async(async {
        let broker = PrivateBroker::start();
        let server = control_server(&broker.address, false, true).await;
        let client = connect(&broker.address).await;
        let dbus = DBusProxy::new(&client).await.expect("create daemon proxy");
        let owner = owners::probe_control_owner(&dbus).await;
        assert!(owner.owner().is_some());
        server
            .release_name(CONTROL_BUS_NAME)
            .await
            .expect("release control name");
        drop(server);
        let control_name = BusName::try_from(CONTROL_BUS_NAME).expect("static control name");
        tokio::time::timeout(std::time::Duration::from_secs(1), async {
            while dbus
                .name_has_owner(control_name.clone())
                .await
                .expect("query control owner")
            {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("control owner should disappear");
        // A replacement without the UnixNotis interface prevents host activation from masking
        // the owner-generation race on systems with the control service installed
        let _replacement = ConnectionBuilder::address(broker.address.as_str())
            .expect("parse private broker address")
            .name(CONTROL_BUS_NAME)
            .expect("request replacement control name")
            .build()
            .await
            .expect("connect replacement owner");

        let checks = control::inspect_control(&client).await;

        assert_eq!(
            checks
                .iter()
                .map(|check| check.id.as_str())
                .collect::<Vec<_>>(),
            ["dbus.control-proxy", "dbus.control-state", "dbus.ui-health"]
        );
        assert_eq!(checks[1].severity, DoctorSeverity::Error);
        assert_eq!(checks[2].severity, DoctorSeverity::Error);
    });
}
