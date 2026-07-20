use std::collections::{HashMap, VecDeque};
use std::time::Instant;

use super::super::worker::CommandJob;
use super::{insert_coalesced_job, CoalescedRefreshState};
use crate::ui::widgets::utils::command::{CommandKind, CommandPlan};
use unixnotis_core::CommandSpec;

fn job(cmd: CommandSpec, kind: CommandKind) -> CommandJob {
    CommandJob {
        cmd,
        plan: CommandPlan {
            kind,
            timeout_override: None,
        },
        respond: None,
        queued_at: Instant::now(),
    }
}

#[test]
fn same_refresh_key_replaces_existing_job() {
    let mut state = CoalescedRefreshState {
        pending: HashMap::new(),
        order: VecDeque::new(),
    };

    insert_coalesced_job(
        &mut state,
        job(CommandSpec::direct("echo", ["a"]), CommandKind::Fast),
    );
    let outcome = insert_coalesced_job(
        &mut state,
        job(CommandSpec::direct("echo", ["a"]), CommandKind::Fast),
    );

    assert_eq!(state.pending.len(), 1);
    assert_eq!(state.order.len(), 1);
    assert!(outcome.replaced_existing);
    assert!(!outcome.evicted_oldest);
}

#[test]
fn distinct_refresh_kinds_keep_separate_jobs() {
    let mut state = CoalescedRefreshState {
        pending: HashMap::new(),
        order: VecDeque::new(),
    };

    insert_coalesced_job(
        &mut state,
        job(CommandSpec::direct("echo", ["a"]), CommandKind::Fast),
    );
    insert_coalesced_job(
        &mut state,
        job(CommandSpec::direct("echo", ["a"]), CommandKind::Slow),
    );

    assert_eq!(state.pending.len(), 2);
    assert_eq!(state.order.len(), 2);
}

#[test]
fn full_refresh_queue_evicts_oldest_key() {
    let mut state = CoalescedRefreshState {
        pending: HashMap::new(),
        order: VecDeque::new(),
    };
    for index in 0..256 {
        insert_coalesced_job(
            &mut state,
            job(
                CommandSpec::direct("echo", [index.to_string()]),
                CommandKind::Fast,
            ),
        );
    }

    let outcome = insert_coalesced_job(
        &mut state,
        job(CommandSpec::direct("echo", ["newest"]), CommandKind::Fast),
    );

    assert_eq!(state.pending.len(), 256);
    assert!(outcome.evicted_oldest);
    assert!(!state
        .pending
        .values()
        .any(|item| item.cmd == CommandSpec::direct("echo", ["0"])));
    assert!(state
        .pending
        .values()
        .any(|item| item.cmd == CommandSpec::direct("echo", ["newest"])));
}
