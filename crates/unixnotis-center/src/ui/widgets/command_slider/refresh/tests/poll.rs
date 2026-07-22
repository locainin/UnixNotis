use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use super::{needs_polling, next_poll_in};
use crate::ui::widgets::command_runtime::backoff::{RefreshBackoff, INFLIGHT_REFRESH_RECHECK};
use crate::ui::widgets::command_runtime::watch::{start_command_watch, CommandWatch};
use crate::ui::widgets::command_slider::refresh::SliderRefreshGate;
use unixnotis_core::CommandSpec;

#[test]
fn polling_without_a_watch_starts_at_the_minimum_deadline() {
    let watch = RefCell::<Option<CommandWatch>>::new(None);
    let gate = SliderRefreshGate::new();
    let backoff = Rc::new(RefCell::new(RefreshBackoff::default()));

    assert_eq!(
        next_poll_in(
            &watch,
            &gate,
            &backoff,
            Instant::now(),
            Duration::from_secs(5),
        ),
        Some(Duration::from_millis(1))
    );
}

#[test]
fn polling_uses_the_inflight_health_check_while_refresh_is_running() {
    let watch = RefCell::<Option<CommandWatch>>::new(None);
    let gate = SliderRefreshGate::new();
    let backoff = Rc::new(RefCell::new(RefreshBackoff::default()));
    assert!(gate.begin_or_queue());

    assert_eq!(
        next_poll_in(
            &watch,
            &gate,
            &backoff,
            Instant::now(),
            Duration::from_secs(5),
        ),
        Some(INFLIGHT_REFRESH_RECHECK)
    );
}

#[test]
fn polling_uses_the_recorded_backoff_deadline() {
    let watch = RefCell::<Option<CommandWatch>>::new(None);
    let gate = SliderRefreshGate::new();
    let backoff = Rc::new(RefCell::new(RefreshBackoff::default()));
    let now = Instant::now();
    backoff
        .borrow_mut()
        .note_success(now, Duration::from_secs(5), true);

    assert_eq!(
        next_poll_in(&watch, &gate, &backoff, now, Duration::from_secs(1)),
        Some(Duration::from_secs(5))
    );
}

#[gtk::test]
fn active_watch_suppresses_polling_until_it_exits() {
    let watch = start_command_watch(&CommandSpec::direct("sleep", ["2"]), || {})
        .expect("watch should start");
    let watch = RefCell::new(Some(watch));
    let gate = SliderRefreshGate::new();
    let backoff = Rc::new(RefCell::new(RefreshBackoff::default()));

    assert!(!needs_polling(&watch));
    assert_eq!(
        next_poll_in(
            &watch,
            &gate,
            &backoff,
            Instant::now(),
            Duration::from_secs(5),
        ),
        None
    );
}

#[gtk::test]
fn exited_watch_is_removed_before_polling_resumes() {
    let watch = start_command_watch(&CommandSpec::direct("true", [] as [&str; 0]), || {})
        .expect("watch should start");
    let watch = RefCell::new(Some(watch));
    let deadline = Instant::now() + Duration::from_secs(2);
    while watch.borrow().as_ref().is_some_and(CommandWatch::is_active) && Instant::now() < deadline
    {
        std::thread::yield_now();
    }

    assert!(needs_polling(&watch));
    assert!(watch.borrow().is_none());
}
