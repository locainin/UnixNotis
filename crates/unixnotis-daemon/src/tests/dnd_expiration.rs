use std::time::Duration;

use super::{delay_until_recheck, DndExpirationScheduler, MAX_CLOCK_RECHECK};
use crate::test_support::daemon_state_for_test;

#[test]
fn delay_until_recheck_returns_zero_for_due_and_past_deadlines() {
    assert_eq!(delay_until_recheck(100, 100), Duration::ZERO);
    assert_eq!(delay_until_recheck(101, 100), Duration::ZERO);
}

#[test]
fn delay_until_recheck_caps_long_waits_for_wall_clock_changes() {
    assert_eq!(delay_until_recheck(100, 110), Duration::from_secs(10));
    assert_eq!(delay_until_recheck(100, 10_000), MAX_CLOCK_RECHECK);
}

#[tokio::test]
async fn scheduler_disables_dnd_when_the_current_deadline_is_due() {
    let state = daemon_state_for_test(false).await;
    let expires_at = chrono::Utc::now().timestamp();
    {
        let mut store = state.store.lock().await;
        store.set_dnd_until(expires_at);
    }
    let scheduler = DndExpirationScheduler::start(state.clone());
    state.set_dnd_scheduler(scheduler.clone());

    scheduler.schedule(Some(expires_at));

    tokio::time::timeout(Duration::from_millis(500), async {
        loop {
            if !state.store.lock().await.dnd_enabled() {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("due DND deadline should be processed promptly");
    assert_eq!(state.store.lock().await.dnd_expires_at(), None);
}
