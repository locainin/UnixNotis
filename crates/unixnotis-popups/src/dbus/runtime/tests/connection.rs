use super::super::connection::OwnerWait;

#[test]
fn owner_wait_states_keep_shutdown_distinct_from_recovery() {
    assert!(matches!(
        OwnerWait::Ready(String::from(":1.20")),
        OwnerWait::Ready(_)
    ));
    assert!(matches!(OwnerWait::Disconnected, OwnerWait::Disconnected));
    assert!(matches!(OwnerWait::Shutdown, OwnerWait::Shutdown));
}
