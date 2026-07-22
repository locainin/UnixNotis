use super::super::ownership::{owner_name_is_self, owner_state_matches, wait_for_owner_state};
use std::time::Duration;
use zbus::fdo::DBusProxy;

#[test]
fn owner_state_matches_expected_presence_and_release() {
    // Non-empty owner names mean the bus name is currently owned
    assert!(owner_state_matches(Some(":1.42"), true));
    assert!(!owner_state_matches(Some(":1.42"), false));

    // Empty owner names mean the bus name was released
    assert!(owner_state_matches(Some(""), false));
    assert!(!owner_state_matches(Some(""), true));

    // Missing signal data is treated like released ownership
    assert!(owner_state_matches(None, false));
}

#[test]
fn owner_name_is_self_requires_exact_unique_name_match() {
    // D-Bus unique names are exact tokens, so prefix or suffix matches must not pass
    assert!(owner_name_is_self(Some(":1.7"), ":1.7"));
    assert!(!owner_name_is_self(Some(":1.70"), ":1.7"));
    assert!(!owner_name_is_self(None, ":1.7"));
}

#[tokio::test]
async fn wait_for_owner_state_returns_true_when_expected_owner_is_already_present() {
    let connection = zbus::Connection::session().await.expect("session bus");
    let proxy = DBusProxy::new(&connection).await.expect("dbus proxy");
    let unique_name = connection
        .unique_name()
        .expect("connection unique name")
        .to_string();
    let bus_name = zbus::names::BusName::try_from(unique_name.as_str()).expect("bus name");

    let matched = wait_for_owner_state(&proxy, bus_name, true, Duration::from_millis(10))
        .await
        .expect("wait for owned name");

    assert!(matched);
}

#[tokio::test]
async fn wait_for_owner_state_returns_false_when_expected_owner_never_appears() {
    let connection = zbus::Connection::session().await.expect("session bus");
    let proxy = DBusProxy::new(&connection).await.expect("dbus proxy");
    let missing_name = format!("com.unixnotis.TestMissing{}", std::process::id());
    let bus_name = zbus::names::BusName::try_from(missing_name.as_str()).expect("bus name");

    let matched = wait_for_owner_state(&proxy, bus_name, true, Duration::from_millis(10))
        .await
        .expect("wait for missing name");

    assert!(!matched);
}
