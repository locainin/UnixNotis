use std::time::Duration;

use super::ClickCooldown;

#[gtk::test]
fn click_cooldown_rejects_bursts_and_reopens_after_its_timeout() {
    let guard = ClickCooldown::new(Duration::ZERO);

    assert!(guard.try_start());
    assert!(!guard.try_start());

    let context = gtk::glib::MainContext::default();
    while context.pending() {
        context.iteration(false);
    }
    assert!(guard.try_start());
}
