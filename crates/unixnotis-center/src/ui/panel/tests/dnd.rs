use chrono::NaiveDate;

use super::{format_dnd_remaining, tomorrow_date};

#[test]
fn remaining_time_is_hidden_after_expiry_and_rounded_up_before_it() {
    assert_eq!(format_dnd_remaining(100, 100), "");
    assert_eq!(format_dnd_remaining(99, 100), "");
    assert_eq!(format_dnd_remaining(101, 100), "· 1m");
    assert_eq!(format_dnd_remaining(100 + 47 * 60, 100), "· 47m");
}

#[test]
fn remaining_time_keeps_hours_compact_without_losing_partial_hour() {
    assert_eq!(format_dnd_remaining(100 + 60 * 60, 100), "· 1h");
    assert_eq!(
        format_dnd_remaining(100 + 2 * 60 * 60 + 5 * 60, 100),
        "· 2h 5m"
    );
}

#[test]
fn morning_choice_uses_the_next_local_eight_oclock() {
    let today = NaiveDate::from_ymd_opt(2026, 7, 18).expect("valid date");

    assert_eq!(tomorrow_date(today), NaiveDate::from_ymd_opt(2026, 7, 19));
}
