use std::time::Duration;

use super::{resolve_command_plan, CommandKind};

#[test]
fn slow_command_promotes_refresh_plan_to_slow_lane() {
    let plan = resolve_command_plan("sleep 1", CommandKind::Fast);

    assert_eq!(plan.kind, CommandKind::Slow);
    assert_eq!(plan.timeout(), Duration::from_millis(800));
}

#[test]
fn action_command_keeps_action_lane_even_when_command_is_slow() {
    let plan = resolve_command_plan("sleep 1", CommandKind::Action);

    assert_eq!(plan.kind, CommandKind::Action);
    assert_eq!(plan.timeout(), Duration::from_millis(1_200));
}

#[test]
fn explicit_timeout_overrides_lane_default() {
    let plan =
        resolve_command_plan("true", CommandKind::Fast).with_timeout(Duration::from_millis(25));

    assert_eq!(plan.timeout(), Duration::from_millis(25));
}
