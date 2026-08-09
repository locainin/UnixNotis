use zbus::fdo::DBusProxy;
use zbus::names::BusName;
use zbus::Connection;

use super::{detect_owner, ensure_owner_is_current};
use crate::trial_mode::state::NotificationOwnerState;

#[tokio::test]
async fn broker_failure_does_not_become_an_unowned_notification_name() {
    let connection = Connection::session().await.expect("session bus connection");
    let proxy = DBusProxy::new(&connection).await.expect("D-Bus proxy");
    connection.close().await.expect("close test bus connection");
    let notifications =
        BusName::try_from(unixnotis_core::NOTIFICATIONS_BUS_NAME).expect("Notifications bus name");

    assert!(
        detect_owner(&proxy, notifications).await.is_err(),
        "broker failure must remain an error"
    );
}

#[tokio::test]
async fn owner_handoff_after_inspection_blocks_the_stop_precondition() {
    let owner_a = Connection::session().await.expect("first owner connection");
    let owner_b = Connection::session()
        .await
        .expect("second owner connection");
    let observer = Connection::session().await.expect("observer connection");
    let name = format!("com.unixnotis.TrialOwner.p{}", std::process::id());
    owner_a
        .request_name(name.as_str())
        .await
        .expect("first owner acquires test name");
    let proxy = DBusProxy::new(&observer)
        .await
        .expect("observer D-Bus proxy");
    let inspected = match detect_owner(
        &proxy,
        BusName::try_from(name.as_str()).expect("test bus name"),
    )
    .await
    .expect("inspect first owner")
    {
        NotificationOwnerState::Owned(owner) => owner,
        NotificationOwnerState::Unowned => panic!("test name must be owned"),
    };
    owner_a
        .release_name(name.as_str())
        .await
        .expect("first owner releases test name");
    owner_b
        .request_name(name.as_str())
        .await
        .expect("second owner acquires test name");

    let error = ensure_owner_is_current(
        &proxy,
        BusName::try_from(name.as_str()).expect("test bus name"),
        &inspected,
    )
    .await
    .expect_err("owner handoff must block process stopping");

    assert!(error.to_string().contains("owner changed"));
    owner_b
        .release_name(name.as_str())
        .await
        .expect("release second test owner");
}
