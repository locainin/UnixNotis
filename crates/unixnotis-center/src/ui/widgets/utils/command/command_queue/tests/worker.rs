use std::time::{Duration, Instant};

use super::{
    dispatch_ready_job, should_warn_queue_full_from, CommandJob, CommandKind, CommandPlan,
    CommandWorker,
};

fn job(cmd: &str, kind: CommandKind) -> CommandJob {
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
fn queue_warning_throttle_blocks_repeats_inside_window() {
    let start = Instant::now();
    let mut last_warn = None;

    assert!(should_warn_queue_full_from(&mut last_warn, start));
    assert!(!should_warn_queue_full_from(
        &mut last_warn,
        start + Duration::from_secs(1)
    ));
}

#[test]
fn queue_warning_throttle_allows_warning_after_window() {
    let start = Instant::now();
    let mut last_warn = None;

    assert!(should_warn_queue_full_from(&mut last_warn, start));
    assert!(should_warn_queue_full_from(
        &mut last_warn,
        start + Duration::from_secs(6)
    ));
}

#[test]
fn action_lane_starts_while_refresh_worker_is_blocked() {
    let worker = CommandWorker::new(1);
    let mut blocked_refresh = job("sleep 1", CommandKind::Fast);
    blocked_refresh.plan.timeout_override = Some(Duration::from_secs(2));
    dispatch_ready_job(&worker, blocked_refresh);
    std::thread::sleep(Duration::from_millis(75));

    let (respond, response) = async_channel::bounded(1);
    let mut action = job("true", CommandKind::Action);
    action.respond = Some(respond);
    let started = Instant::now();
    dispatch_ready_job(&worker, action);

    loop {
        match response.try_recv() {
            Ok(result) => {
                assert!(result.expect("action command should run").status.success());
                break;
            }
            Err(async_channel::TryRecvError::Empty) => {
                assert!(
                    started.elapsed() < Duration::from_millis(600),
                    "action waited behind the blocked refresh worker"
                );
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(async_channel::TryRecvError::Closed) => {
                panic!("action response channel closed before completion");
            }
        }
    }
}
