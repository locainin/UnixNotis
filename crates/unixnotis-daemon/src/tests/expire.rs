use super::*;

impl ExpirationScheduler {
    pub(crate) fn channel_for_test() -> (Self, mpsc::UnboundedReceiver<ExpirationCommand>) {
        let (sender, receiver) = mpsc::unbounded_channel();
        (Self { sender }, receiver)
    }
}
use chrono::Utc;
use futures_util::TryStreamExt;
use std::collections::HashMap;
use std::time::Duration;
use unixnotis_core::{
    Notification, NotificationImage, Urgency, CONTROL_INTERFACE, CONTROL_OBJECT_PATH,
};
use zbus::message::Type;
use zbus::zvariant::OwnedValue;
use zbus::{Connection, MatchRule, MessageStream};

fn ticket(id: u32, generation: u64, deadline: Instant) -> ExpirationTicket {
    ExpirationTicket {
        id,
        generation,
        deadline,
    }
}

#[test]
fn expiration_heap_orders_by_deadline() {
    let now = Instant::now();
    let mut heap = BinaryHeap::new();
    heap.push(ExpirationItem {
        ticket: ticket(1, 1, now + Duration::from_secs(2)),
    });
    heap.push(ExpirationItem {
        ticket: ticket(2, 2, now + Duration::from_secs(1)),
    });

    let first = heap.pop().expect("first item");
    assert_eq!(first.ticket.id, 2);
}

#[test]
fn apply_command_tracks_latest_schedule() {
    let now = Instant::now();
    let mut heap = BinaryHeap::new();
    let mut scheduled = HashMap::new();

    apply_command(
        ExpirationCommand::Schedule {
            ticket: ticket(7, 1, now + Duration::from_secs(5)),
        },
        &mut heap,
        &mut scheduled,
    );
    apply_command(
        ExpirationCommand::Schedule {
            ticket: ticket(7, 2, now + Duration::from_secs(3)),
        },
        &mut heap,
        &mut scheduled,
    );

    assert_eq!(scheduled.len(), 1);
    assert_eq!(
        scheduled.get(&7),
        Some(&ticket(7, 2, now + Duration::from_secs(3)))
    );
    assert_eq!(heap.len(), 2);
}

#[test]
fn late_older_schedule_cannot_replace_newer_generation() {
    let now = Instant::now();
    let mut heap = BinaryHeap::new();
    let mut scheduled = HashMap::new();
    let newer = ticket(7, 2, now + Duration::from_secs(10));

    // This order reproduces delivery after two store commits were reversed
    apply_command(
        ExpirationCommand::Schedule { ticket: newer },
        &mut heap,
        &mut scheduled,
    );
    apply_command(
        ExpirationCommand::Schedule {
            ticket: ticket(7, 1, now + Duration::from_secs(1)),
        },
        &mut heap,
        &mut scheduled,
    );

    assert_eq!(scheduled.get(&7), Some(&newer));
    assert_eq!(heap.len(), 1);
}

#[test]
fn apply_command_cancel_removes_schedule() {
    let now = Instant::now();
    let mut heap = BinaryHeap::new();
    let mut scheduled = HashMap::new();

    apply_command(
        ExpirationCommand::Schedule {
            ticket: ticket(9, 4, now + Duration::from_secs(2)),
        },
        &mut heap,
        &mut scheduled,
    );
    apply_command(
        ExpirationCommand::Cancel {
            id: 9,
            generation: 4,
        },
        &mut heap,
        &mut scheduled,
    );

    assert!(scheduled.is_empty());
}

#[test]
fn late_older_cancel_preserves_newer_generation() {
    let now = Instant::now();
    let mut heap = BinaryHeap::new();
    let mut scheduled = HashMap::new();
    let newer = ticket(9, 5, now + Duration::from_secs(2));
    apply_command(
        ExpirationCommand::Schedule { ticket: newer },
        &mut heap,
        &mut scheduled,
    );

    apply_command(
        ExpirationCommand::Cancel {
            id: 9,
            generation: 4,
        },
        &mut heap,
        &mut scheduled,
    );

    assert_eq!(scheduled.get(&9), Some(&newer));
}

#[test]
fn maybe_compact_rebuilds_from_scheduled() {
    let now = Instant::now();
    let mut heap = BinaryHeap::new();
    let mut scheduled = HashMap::new();

    scheduled.insert(1_u32, ticket(1, 1, now + Duration::from_secs(1)));
    for id in 0..129_u32 {
        heap.push(ExpirationItem {
            ticket: ticket(
                id,
                u64::from(id) + 1,
                now + Duration::from_secs(u64::from(id) + 1),
            ),
        });
    }

    maybe_compact(&mut heap, &scheduled);
    assert_eq!(heap.len(), scheduled.len());
    let item = heap.pop().expect("rebuilt item");
    assert_eq!(item.ticket.id, 1);
}

#[tokio::test]
async fn scheduler_closes_notification_at_scheduled_deadline() {
    let state = crate::test_support::daemon_state_for_test(false).await;
    let scheduler = ExpirationScheduler::start(state.clone());
    state.set_scheduler(scheduler.clone());
    let deadline = Instant::now() + Duration::from_millis(20);

    let key = {
        let mut store = state.store.lock().await;
        let outcome = store.insert(make_notification("expires"), 0);
        let key = outcome.notification.key();
        store.set_expiration(&outcome.notification, Some(deadline));
        key
    };

    scheduler.schedule(key.id, key.generation, Some(deadline));

    let expired = tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            let is_active = {
                let store = state.store.lock().await;
                store.active_notification_view(key.id).is_some()
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
    assert_eq!(store.expiration_for(key.id), None);
}

#[tokio::test]
async fn old_timer_never_closes_or_signals_for_same_id_replacement() {
    let state = crate::test_support::daemon_state_for_test(false).await;
    let scheduler = ExpirationScheduler::start(state.clone());
    state.set_scheduler(scheduler.clone());
    let mut closed_signals = control_closed_stream(&state).await;
    let old_deadline = Instant::now() + Duration::from_millis(30);

    // Holding the store lock forces the expired worker to wait at its commit point
    let mut store = state.store.lock().await;
    let original = store.insert(make_notification("original"), 0).notification;
    store.set_expiration(&original, Some(old_deadline));
    scheduler.schedule(original.id, original.generation, Some(old_deadline));
    tokio::time::sleep(Duration::from_millis(80)).await;

    let replacement = store
        .insert(make_notification("replacement"), original.id)
        .notification;
    let replacement_deadline = Instant::now() + Duration::from_millis(250);
    store.set_expiration(&replacement, Some(replacement_deadline));
    scheduler.schedule(
        replacement.id,
        replacement.generation,
        Some(replacement_deadline),
    );
    drop(store);

    // The stale timer now resumes but cannot remove the replacement generation
    tokio::time::sleep(Duration::from_millis(60)).await;
    let active = state
        .store
        .lock()
        .await
        .active_notification_view(replacement.id)
        .expect("replacement should remain active after the old deadline");
    assert_eq!(active.generation, replacement.generation);
    assert_eq!(active.summary, "replacement");
    assert!(
        tokio::time::timeout(Duration::from_millis(60), closed_signals.try_next())
            .await
            .is_err(),
        "old generation must not emit a close signal"
    );

    // The replacement keeps its own schedule and expires normally
    let signal = tokio::time::timeout(Duration::from_millis(500), closed_signals.try_next())
        .await
        .expect("replacement close signal should arrive")
        .expect("close signal stream should remain healthy")
        .expect("replacement close signal");
    let (closed_id, closed_generation, reason) = signal
        .body()
        .deserialize::<(u32, u64, CloseReason)>()
        .expect("notification close signal body");
    assert_eq!(closed_id, replacement.id);
    assert_eq!(closed_generation, replacement.generation);
    assert_eq!(reason as u32, CloseReason::Expired as u32);
    assert!(state
        .store
        .lock()
        .await
        .active_notification_view(replacement.id)
        .is_none());
}

async fn control_closed_stream(state: &DaemonState) -> MessageStream {
    let receiver = Connection::session()
        .await
        .expect("receiver should connect to the test session bus");
    let sender = state
        .connection()
        .unique_name()
        .expect("daemon connection should have a unique name")
        .to_string();
    let rule = MatchRule::builder()
        .msg_type(Type::Signal)
        .sender(sender.as_str())
        .expect("daemon sender should be a valid bus name")
        .path(CONTROL_OBJECT_PATH)
        .expect("control object path should be valid")
        .interface(CONTROL_INTERFACE)
        .expect("control interface should be valid")
        .member("NotificationClosed")
        .expect("close member should be valid")
        .build();
    MessageStream::for_match_rule(rule, &receiver, Some(8))
        .await
        .expect("close signal subscription should succeed")
}

fn make_notification(summary: &str) -> Notification {
    Notification {
        id: 0,
        generation: 0,
        app_name: "TestApp".to_string(),
        app_icon: String::new(),
        attribution: unixnotis_core::NotificationAttribution::default(),
        summary: summary.to_string(),
        body: String::new(),
        actions: Vec::new(),
        inline_reply: unixnotis_core::InlineReply::default(),
        inline_reply_policy: unixnotis_core::InlineReplyPolicy::Allow,
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
