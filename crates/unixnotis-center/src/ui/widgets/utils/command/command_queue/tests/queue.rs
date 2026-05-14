use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

use super::coalesced::{insert_coalesced_job, CoalescedRefreshState};
use super::delayed::{
    next_delayed_wake, next_ready_delayed_job_index, try_enqueue_delayed_job, DelayedState,
};
use super::{should_warn_queue_full_from, CommandJob, CommandKind, CommandPlan};

fn job(cmd: &str, kind: CommandKind) -> CommandJob {
    // Keep test jobs small
    CommandJob {
        cmd: cmd.to_string(),
        plan: CommandPlan {
            kind,
            timeout_override: None,
        },
        respond: None,
        queued_at: Instant::now(),
    }
}

#[test]
fn coalesced_insert_replaces_existing_key() {
    let mut state = CoalescedRefreshState {
        pending: HashMap::new(),
        order: VecDeque::new(),
    };
    insert_coalesced_job(&mut state, job("echo a", CommandKind::Fast));
    let outcome = insert_coalesced_job(&mut state, job("echo a", CommandKind::Fast));

    // Same key keeps one slot
    assert_eq!(state.pending.len(), 1);
    assert_eq!(state.order.len(), 1);
    assert!(outcome.replaced_existing);
    assert!(!outcome.evicted_oldest);
}

#[test]
fn coalesced_insert_keeps_distinct_keys() {
    let mut state = CoalescedRefreshState {
        pending: HashMap::new(),
        order: VecDeque::new(),
    };
    let first = insert_coalesced_job(&mut state, job("echo a", CommandKind::Fast));
    let second = insert_coalesced_job(&mut state, job("echo a", CommandKind::Slow));

    // Fast and slow stay separate
    assert_eq!(state.pending.len(), 2);
    assert_eq!(state.order.len(), 2);
    assert!(!first.replaced_existing);
    assert!(!second.replaced_existing);
}

#[test]
fn coalesced_insert_evicts_oldest_key_at_capacity() {
    let mut state = CoalescedRefreshState {
        pending: HashMap::new(),
        order: VecDeque::new(),
    };

    for idx in 0..256 {
        let cmd = format!("echo {idx}");
        let outcome = insert_coalesced_job(&mut state, job(&cmd, CommandKind::Fast));
        assert!(!outcome.evicted_oldest);
    }

    let outcome = insert_coalesced_job(&mut state, job("echo newest", CommandKind::Fast));

    // Oldest key is dropped once capacity is reached
    assert_eq!(state.pending.len(), 256);
    assert_eq!(state.order.len(), 256);
    assert!(outcome.evicted_oldest);
    assert!(!state.pending.values().any(|job| job.cmd == "echo 0"));
    assert!(state.pending.values().any(|job| job.cmd == "echo newest"));
}

#[test]
fn delayed_enqueue_rejects_jobs_once_capacity_is_reached() {
    let now = Instant::now();
    let mut state = DelayedState {
        pending: Vec::new(),
        next_seq: 0,
    };

    assert!(try_enqueue_delayed_job(&mut state, job("echo a", CommandKind::Slow), now, 1).is_ok());
    let rejected = try_enqueue_delayed_job(
        &mut state,
        job("echo b", CommandKind::Slow),
        now + Duration::from_millis(10),
        1,
    );

    // Delayed queue stays bounded
    assert!(rejected.is_err());
    assert_eq!(state.pending.len(), 1);
}

#[test]
fn delayed_ready_index_prefers_earliest_deadline_then_sequence() {
    let now = Instant::now();
    let mut state = DelayedState {
        pending: Vec::new(),
        next_seq: 0,
    };

    assert!(try_enqueue_delayed_job(
        &mut state,
        job("echo third", CommandKind::Slow),
        now + Duration::from_millis(5),
        8,
    )
    .is_ok());
    assert!(
        try_enqueue_delayed_job(&mut state, job("echo first", CommandKind::Slow), now, 8).is_ok()
    );
    assert!(
        try_enqueue_delayed_job(&mut state, job("echo second", CommandKind::Slow), now, 8).is_ok()
    );

    // Same-time jobs keep order
    let index = next_ready_delayed_job_index(&state.pending, now).expect("expected due job");
    assert_eq!(state.pending[index].job.cmd, "echo first");
}

#[test]
fn delayed_wake_reports_zero_for_due_jobs() {
    let now = Instant::now();
    let mut state = DelayedState {
        pending: Vec::new(),
        next_seq: 0,
    };

    assert!(
        try_enqueue_delayed_job(&mut state, job("echo due", CommandKind::Slow), now, 8).is_ok()
    );

    // Due work wakes right away
    let delay = next_delayed_wake(&state.pending, now).expect("expected wake delay");
    assert_eq!(delay, Duration::ZERO);
}

#[test]
fn queue_full_warning_throttle_blocks_repeats_inside_window() {
    let start = Instant::now();
    let mut last_warn = None;

    assert!(should_warn_queue_full_from(&mut last_warn, start));
    // Second warning inside the throttle interval should stay quiet
    assert!(!should_warn_queue_full_from(
        &mut last_warn,
        start + Duration::from_secs(1)
    ));
}

#[test]
fn queue_full_warning_throttle_allows_after_window() {
    let start = Instant::now();
    let mut last_warn = None;

    assert!(should_warn_queue_full_from(&mut last_warn, start));
    // Warning should be allowed once the configured interval has passed
    assert!(should_warn_queue_full_from(
        &mut last_warn,
        start + Duration::from_secs(6)
    ));
}
