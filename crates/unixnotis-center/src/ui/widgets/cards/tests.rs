//! Card timing tests

use std::time::Duration;

use gtk::glib;

use super::calendar::next_local_midnight_delay;

#[test]
fn midnight_delay_targets_next_day_boundary() {
    let timezone = glib::TimeZone::local();
    let now = glib::DateTime::new(&timezone, 2026, 2, 6, 23, 59, 30.0).expect("valid datetime");
    let delay = next_local_midnight_delay(&now).expect("delay");
    // A narrow band keeps daylight-saving and rounding mistakes easy to spot
    assert!(delay <= Duration::from_secs(31));
    assert!(delay >= Duration::from_secs(29));
}
