use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

use super::super::worker::CommandJob;
use super::{
    next_delayed_wake, next_ready_delayed_job_index, try_enqueue_delayed_job, DelayedSlowQueue,
    DelayedState,
};
use crate::ui::widgets::utils::command::{CommandKind, CommandPlan};

fn job(cmd: &str) -> CommandJob {
    CommandJob {
        cmd: cmd.to_string(),
        plan: CommandPlan {
            kind: CommandKind::Slow,
            timeout_override: None,
        },
        respond: None,
        queued_at: Instant::now(),
    }
}

#[test]
fn delayed_queue_rejects_jobs_at_capacity() {
    let now = Instant::now();
    let mut state = DelayedState {
        pending: Vec::new(),
        next_seq: 0,
    };

    assert!(try_enqueue_delayed_job(&mut state, job("echo a"), now, 1).is_ok());
    assert!(try_enqueue_delayed_job(&mut state, job("echo b"), now, 1).is_err());
    assert_eq!(state.pending.len(), 1);
}

#[test]
fn delayed_submit_returns_job_when_queue_lock_is_poisoned() {
    let queue = DelayedSlowQueue {
        state: Mutex::new(DelayedState {
            pending: Vec::new(),
            next_seq: 0,
        }),
        wake: Condvar::new(),
    };
    let poison = catch_unwind(AssertUnwindSafe(|| {
        let _guard = queue.state.lock().expect("delayed test queue lock");
        panic!("poison delayed test queue");
    }));
    assert!(poison.is_err());

    assert!(queue.submit(job("echo rejected"), Duration::ZERO).is_err());
}

#[test]
fn due_job_selection_prefers_deadline_then_sequence() {
    let now = Instant::now();
    let mut state = DelayedState {
        pending: Vec::new(),
        next_seq: 0,
    };
    assert!(try_enqueue_delayed_job(
        &mut state,
        job("echo later"),
        now + Duration::from_millis(5),
        8,
    )
    .is_ok());
    assert!(try_enqueue_delayed_job(&mut state, job("echo first"), now, 8).is_ok());
    assert!(try_enqueue_delayed_job(&mut state, job("echo second"), now, 8).is_ok());

    let index = next_ready_delayed_job_index(&state.pending, now).expect("expected due job");

    assert_eq!(state.pending[index].job.cmd, "echo first");
    assert_eq!(next_delayed_wake(&state.pending, now), Some(Duration::ZERO));
}
