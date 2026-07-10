use super::*;
use chrono::Utc;
use std::collections::HashMap;
use std::time::Duration;
use unixnotis_core::{Notification, NotificationImage, Urgency};
use zbus::zvariant::OwnedValue;

#[test]
fn expiration_heap_orders_by_deadline() {
    let now = Instant::now();
    let mut heap = BinaryHeap::new();
    heap.push(ExpirationItem {
        id: 1,
        deadline: now + Duration::from_secs(2),
    });
    heap.push(ExpirationItem {
        id: 2,
        deadline: now + Duration::from_secs(1),
    });

    let first = heap.pop().expect("first item");
    assert_eq!(first.id, 2);
}

#[test]
fn apply_command_tracks_latest_schedule() {
    let now = Instant::now();
    let mut heap = BinaryHeap::new();
    let mut scheduled = HashMap::new();

    apply_command(
        ExpirationCommand::Schedule {
            id: 7,
            deadline: now + Duration::from_secs(5),
        },
        &mut heap,
        &mut scheduled,
    );
    apply_command(
        ExpirationCommand::Schedule {
            id: 7,
            deadline: now + Duration::from_secs(3),
        },
        &mut heap,
        &mut scheduled,
    );

    assert_eq!(scheduled.len(), 1);
    assert_eq!(scheduled.get(&7), Some(&(now + Duration::from_secs(3))));
    assert_eq!(heap.len(), 2);
}

#[test]
fn apply_command_cancel_removes_schedule() {
    let now = Instant::now();
    let mut heap = BinaryHeap::new();
    let mut scheduled = HashMap::new();

    apply_command(
        ExpirationCommand::Schedule {
            id: 9,
            deadline: now + Duration::from_secs(2),
        },
        &mut heap,
        &mut scheduled,
    );
    apply_command(
        ExpirationCommand::Cancel { id: 9 },
        &mut heap,
        &mut scheduled,
    );

    assert!(scheduled.is_empty());
}

#[test]
fn maybe_compact_rebuilds_from_scheduled() {
    let now = Instant::now();
    let mut heap = BinaryHeap::new();
    let mut scheduled = HashMap::new();

    scheduled.insert(1_u32, now + Duration::from_secs(1));
    for id in 0..129_u32 {
        heap.push(ExpirationItem {
            id,
            deadline: now + Duration::from_secs(id as u64 + 1),
        });
    }

    maybe_compact(&mut heap, &scheduled);
    assert_eq!(heap.len(), scheduled.len());
    let item = heap.pop().expect("rebuilt item");
    assert_eq!(item.id, 1);
}

#[tokio::test]
async fn scheduler_closes_notification_at_scheduled_deadline() {
    let state = crate::test_support::daemon_state_for_test(false).await;
    let scheduler = ExpirationScheduler::start(state.clone());
    state.set_scheduler(scheduler.clone());
    let deadline = Instant::now() + Duration::from_millis(20);

    let id = {
        let mut store = state.store.lock().await;
        let outcome = store.insert(make_notification("expires"), 0);
        let id = outcome.notification.id;
        store.set_expiration(id, Some(deadline));
        id
    };

    scheduler.schedule(id, Some(deadline));

    let expired = tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            let is_active = {
                let store = state.store.lock().await;
                store.active_notification_view(id).is_some()
            };
            if !is_active {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await;

    assert!(expired.is_ok());
    let store = state.store.lock().await;
    assert_eq!(store.expiration_for(id), None);
}

fn make_notification(summary: &str) -> Notification {
    Notification {
        id: 0,
        app_name: "TestApp".to_string(),
        app_icon: String::new(),
        summary: summary.to_string(),
        body: String::new(),
        actions: Vec::new(),
        hints: HashMap::<String, OwnedValue>::new(),
        urgency: Urgency::Normal,
        category: None,
        is_transient: false,
        is_resident: false,
        suppress_popup: false,
        suppress_sound: false,
        image: NotificationImage::default(),
        expire_timeout: 0,
        received_at: Utc::now(),
        sender_name: Some(":1.test".to_string()),
        sender_pid: Some(1234),
        sender_start_time: Some(555),
        sender_executable: Some("/usr/bin/test-app".to_string()),
    }
}
