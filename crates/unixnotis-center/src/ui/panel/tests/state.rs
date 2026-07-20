use crate::ui::notifications::NotificationCounts;

use super::format_counts;

#[test]
fn count_text_shows_total_when_search_is_inactive() {
    let counts = NotificationCounts {
        matching: 42,
        total: 42,
        filter_active: false,
    };

    assert_eq!(format_counts(counts), "42");
}

#[test]
fn count_text_shows_matches_over_total_during_search() {
    let matches = NotificationCounts {
        matching: 3,
        total: 42,
        filter_active: true,
    };
    let no_matches = NotificationCounts {
        matching: 0,
        total: 42,
        filter_active: true,
    };

    assert_eq!(format_counts(matches), "3 / 42");
    assert_eq!(format_counts(no_matches), "0 / 42");
}
