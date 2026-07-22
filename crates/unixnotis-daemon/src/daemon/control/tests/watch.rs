use std::time::Duration;

use super::spawn_inhibitor_owner_watch;
use crate::test_support::daemon_state_for_test;

#[tokio::test]
async fn owner_watch_removes_inhibitors_when_the_client_disconnects() {
    let state = daemon_state_for_test(false).await;
    let client = zbus::Connection::session()
        .await
        .expect("connect inhibitor owner to session bus");
    let owner = client
        .unique_name()
        .expect("client should have a unique bus name")
        .to_string();
    {
        let mut store = state.store.lock().await;
        store.add_inhibitor(owner, "test owner lifetime".to_string(), 0);
        assert_eq!(store.inhibitor_count(), 1);
    }

    spawn_inhibitor_owner_watch(state.clone())
        .await
        .expect("start inhibitor owner watch");
    client.close().await.expect("disconnect inhibitor owner");

    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if state.store.lock().await.inhibitor_count() == 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("owner watch should remove disconnected inhibitors");
}
