use super::support::*;

#[test]
fn max_active_zero_archives_immediately() {
    let mut store = make_store_with_limits(0, 10);

    let outcome = store.insert(make_notification("first"), 0);
    assert_eq!(outcome.evicted.len(), 1);
    assert!(store.list_active().is_empty());
    assert_eq!(store.history_len(), 1);

    store.insert(make_notification("second"), 0);
    assert!(store.list_active().is_empty());
    assert_eq!(store.history_len(), 2);
}

#[test]
fn max_active_evicts_oldest_to_history() {
    let mut store = make_store_with_limits(1, 10);
    store.insert(make_notification("first"), 0);

    let outcome = store.insert(make_notification("second"), 0);

    assert_eq!(outcome.evicted.len(), 1);
    let active = store.list_active();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].summary, "second");
    assert_eq!(store.history_len(), 1);
}

#[test]
fn max_active_hard_cap_limits_even_when_config_is_higher() {
    let mut store = make_store_with_limits(32, 64);
    for index in 0..18 {
        store.insert(make_notification(&format!("entry-{index}")), 0);
    }

    let active = store.list_active();
    let history = store.list_history();

    assert_eq!(active.len(), 12);
    assert_eq!(history.len(), 6);
    assert_eq!(active[0].summary, "entry-17");
    assert_eq!(active[11].summary, "entry-6");
}

#[test]
fn noisy_principal_cannot_evict_another_principals_active_notifications() {
    let mut store = make_store_with_limits(5, 128);
    let protected = (0..5)
        .map(|index| {
            store
                .insert(
                    make_notification_with_sender(
                        &format!("protected-{index}"),
                        ":1.protected",
                        10,
                        100,
                    ),
                    0,
                )
                .active_notification()
                .key()
        })
        .collect::<Vec<_>>();

    for index in 0..50 {
        store.insert(
            make_notification_with_sender(&format!("noisy-{index}"), ":1.noisy", 20, 200),
            0,
        );
    }

    let active = store.list_active();
    for key in protected {
        assert!(
            active.iter().any(|notification| notification.key() == key),
            "a different principal must not evict protected active state"
        );
    }
    assert_eq!(active.len(), 10);
}

#[test]
fn distinct_bus_senders_remain_isolated_without_process_metadata() {
    let mut store = make_store_with_limits(5, 128);
    let protected = (0..5)
        .map(|index| {
            let mut notification = make_notification(&format!("protected-bus-{index}"));
            notification.sender_name = Some(":1.100".to_string());
            notification.sender_pid = None;
            notification.sender_start_time = None;
            store.insert(notification, 0).active_notification().key()
        })
        .collect::<Vec<_>>();

    for index in 0..20 {
        let mut notification = make_notification(&format!("noisy-bus-{index}"));
        notification.sender_name = Some(":1.200".to_string());
        notification.sender_pid = None;
        notification.sender_start_time = None;
        store.insert(notification, 0);
    }

    let active = store.list_active();
    assert!(
        protected
            .iter()
            .all(|key| active.iter().any(|notification| notification.key() == *key)),
        "a degraded sender must not evict a different unique bus connection"
    );
    assert_eq!(active.len(), 10);
}

#[test]
fn absolute_active_cap_keeps_exact_capacity_and_evicts_the_admitted_tie() {
    let mut store = make_store_with_limits(12, 256);
    let mut admitted_oldest = None;
    for principal in 0..12_u32 {
        let count = if principal < 8 { 11 } else { 10 };
        for index in 0..count {
            let outcome = store.insert(
                make_notification_with_sender(
                    &format!("principal-{principal}-{index}"),
                    &format!(":1.{principal}"),
                    principal.saturating_add(1),
                    u64::from(principal).saturating_add(100),
                ),
                0,
            );
            if principal == 0 && index == 0 {
                admitted_oldest = Some(outcome.active_notification().key());
            }
            assert!(
                outcome.evicted.is_empty(),
                "the exact global capacity must not evict active state"
            );
        }
    }
    assert_eq!(store.list_active().len(), 128);

    let outcome = store.insert(
        make_notification_with_sender("principal-0-tie", ":1.0", 1, 100),
        0,
    );

    assert_eq!(store.list_active().len(), 128);
    assert_eq!(outcome.evicted, vec![admitted_oldest.expect("oldest key")]);
}

#[test]
fn absolute_active_cap_evicts_a_largest_existing_share_not_the_newcomer() {
    let mut store = make_store_with_limits(12, 256);
    for principal in 0..10_u32 {
        for index in 0..12 {
            store.insert(
                make_notification_with_sender(
                    &format!("incumbent-{principal}-{index}"),
                    &format!(":1.{principal}"),
                    principal.saturating_add(1),
                    u64::from(principal).saturating_add(100),
                ),
                0,
            );
        }
    }
    let mut newcomer_keys = Vec::new();
    let mut final_outcome = None;
    for index in 0..9 {
        let outcome = store.insert(
            make_notification_with_sender(&format!("newcomer-{index}"), ":1.newcomer", 999, 9_999),
            0,
        );
        newcomer_keys.push(outcome.active_notification().key());
        final_outcome = Some(outcome);
    }
    let outcome = final_outcome.expect("newcomer outcome");

    assert_eq!(store.list_active().len(), 128);
    assert_eq!(outcome.evicted.len(), 1);
    assert!(
        !newcomer_keys.contains(&outcome.evicted[0]),
        "a smaller newcomer share must not be selected as the emergency victim"
    );
    let active = store.list_active();
    assert!(
        newcomer_keys
            .iter()
            .all(|key| active.iter().any(|notification| notification.key() == *key)),
        "every newcomer generation must survive when a larger share exists"
    );
}

#[test]
fn zero_history_limit_keeps_active_notifications_and_drops_evictions() {
    let mut active_store = make_store_with_limits(2, 0);
    active_store.insert(make_notification("first"), 0);
    let active = active_store.insert(make_notification("second"), 0);
    assert!(active.evicted.is_empty());
    assert_eq!(active_store.list_active().len(), 2);

    let mut evicting_store = make_store_with_limits(0, 0);
    let evicted = evicting_store.insert(make_notification("first"), 0);
    assert_eq!(evicted.evicted.len(), 1);
    assert!(evicting_store.list_active().is_empty());
    assert_eq!(evicting_store.history_len(), 0);
}

#[test]
fn insert_outcome_reflects_popup_and_sound_policy() {
    let state_dir = make_temp_state_dir("insert-outcome-policy");
    let mut config = Config::default();
    config.general.dnd_default = false;
    let mut store = NotificationStore::new_with_state_dir(config, state_dir.clone());
    let allowed = store.insert(make_notification("normal"), 0);
    assert!(allowed.popup_admission.should_show());
    assert!(allowed.allow_sound);

    let dnd_state_dir = make_temp_state_dir("insert-outcome-dnd");
    let mut dnd_config = Config::default();
    dnd_config.general.dnd_default = true;
    let mut dnd_store = NotificationStore::new_with_state_dir(dnd_config, dnd_state_dir.clone());
    let normal = dnd_store.insert(make_notification("normal dnd"), 0);
    assert!(!normal.popup_admission.should_show());
    assert!(!normal.allow_sound);

    let mut critical = make_notification("critical dnd");
    critical.urgency = unixnotis_core::Urgency::Critical;
    let critical = dnd_store.insert(critical, 0);
    assert!(critical.popup_admission.should_show());
    assert!(critical.allow_sound);

    let mut silent = make_notification("silent");
    silent.suppress_sound = true;
    let silent = store.insert(silent, 0);
    assert!(!silent.allow_sound);

    cleanup_temp_dir(&state_dir);
    cleanup_temp_dir(&dnd_state_dir);
}

#[test]
fn popup_candidates_exclude_notifications_suppressed_by_rules() {
    let mut store = make_store_with_limits(4, 4);
    let mut notification = make_notification("rule-suppressed");
    notification.suppress_popup = true;

    let outcome = store.insert(notification, 0);

    assert!(!outcome.popup_admission.should_show());
    assert_eq!(store.list_active().len(), 1);
    assert!(store.list_popup_candidates().is_empty());
}

#[test]
fn popup_candidates_include_notifications_allowed_by_rules() {
    let mut store = make_store_with_limits(4, 4);
    let outcome = store.insert(make_notification("popup-allowed"), 0);

    let candidates = store.list_popup_candidates();

    assert!(outcome.popup_admission.should_show());
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].summary, "popup-allowed");
}
