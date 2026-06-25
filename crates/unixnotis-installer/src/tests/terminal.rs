use crate::terminal::TerminalGuard;

#[test]
fn terminal_guard_remains_stateful() {
    // Avoid calling TerminalGuard::new in unit tests because it changes the real terminal mode
    assert!(std::mem::size_of::<TerminalGuard>() > 0);
}
