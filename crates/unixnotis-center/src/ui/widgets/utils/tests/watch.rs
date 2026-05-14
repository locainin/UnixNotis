use super::should_emit_watch_event;

#[test]
fn pactl_watch_filters_to_sink_and_server() {
    let cmd = "pactl subscribe";
    assert!(should_emit_watch_event(cmd, "Event 'change' on sink #1"));
    assert!(should_emit_watch_event(cmd, "Event 'new' on server"));
    assert!(!should_emit_watch_event(cmd, "Event 'change' on source #2"));
    assert!(!should_emit_watch_event(
        cmd,
        "Event 'change' on sink-input #3"
    ));
}

#[test]
fn non_pactl_commands_always_emit() {
    assert!(should_emit_watch_event("echo test", "anything"));
}
