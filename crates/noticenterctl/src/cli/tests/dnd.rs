use std::str::FromStr;

use super::super::dnd::{future_timestamp, DndClockTime, DndDuration};

#[test]
fn duration_parser_accepts_supported_units_and_rejects_invalid_bounds() {
    assert!(DndDuration::from_str("30m").is_ok());
    assert!(DndDuration::from_str("1h").is_ok());
    assert!(DndDuration::from_str("2d").is_ok());
    assert!(DndDuration::from_str("0m").is_err());
    assert!(DndDuration::from_str("30").is_err());
    assert!(DndDuration::from_str("366d").is_err());
}

#[test]
fn clock_parser_requires_exact_twenty_four_hour_time() {
    assert!(DndClockTime::from_str("08:00").is_ok());
    assert!(DndClockTime::from_str("23:59").is_ok());
    assert!(DndClockTime::from_str("24:00").is_err());
    assert!(DndClockTime::from_str("8:00").is_err());
    assert!(DndClockTime::from_str("08:0").is_err());
    assert!(DndClockTime::from_str("8am").is_err());
}

#[test]
fn clock_deadline_resolves_to_a_future_occurrence() {
    let now = chrono::Utc::now().timestamp();
    let deadline = DndClockTime::from_str("08:00")
        .expect("valid clock")
        .deadline()
        .expect("next local occurrence");

    assert!(deadline > now);
    assert!(deadline <= now + 2 * 24 * 60 * 60);
}

#[test]
fn future_timestamp_selects_the_next_absolute_occurrence() {
    assert_eq!(future_timestamp(100, Some(200), None), Some(200));
    assert_eq!(future_timestamp(150, Some(100), Some(200)), Some(200));
    assert_eq!(future_timestamp(50, Some(200), Some(100)), Some(100));
    assert_eq!(future_timestamp(200, Some(100), Some(200)), None);
}
