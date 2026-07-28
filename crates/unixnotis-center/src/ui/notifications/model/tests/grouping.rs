use super::*;
use crate::ui::notifications::test_support as support;

fn normalize_filter_query(query: &str) -> Option<FilterQuery> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.is_ascii() {
        return Some(FilterQuery {
            text: trimmed.to_ascii_lowercase().into_boxed_str(),
            ascii_only: true,
        });
    }
    Some(FilterQuery {
        text: trimmed.to_lowercase().into_boxed_str(),
        ascii_only: false,
    })
}

#[test]
fn normalize_filter_query_marks_ascii_fast_path() {
    let query = normalize_filter_query("  Spotify  ").expect("query");
    assert!(query.ascii_only);
    assert_eq!(query.text.as_ref(), "spotify");
}

#[test]
fn normalize_filter_query_keeps_unicode_lowercasing() {
    let query = normalize_filter_query("  ÄPF  ").expect("query");
    assert!(!query.ascii_only);
    assert_eq!(query.text.as_ref(), "äpf");
}

#[test]
fn ascii_filter_matches_without_allocating_a_lowered_copy() {
    let query = normalize_filter_query("spotify").expect("query");
    assert!(contains_casefold("SPOTIFY", &query));
    assert!(contains_casefold("spotifyd", &query));
    assert!(!contains_casefold("Firefox", &query));
}

#[test]
fn unicode_filter_still_matches_non_ascii_text() {
    let query = normalize_filter_query("äpf").expect("query");
    assert!(contains_casefold("Äpfel und Birnen", &query));
    assert!(!contains_casefold("Cafe", &query));
}

#[gtk::test]
fn normalize_group_key_strips_invisible_chars_and_lowercases_ascii() {
    let list = support::make_list();

    assert_eq!(
        list.normalize_group_key("  Te\u{200B}r\tminal  ").as_ref(),
        "terminal"
    );
    assert_eq!(list.normalize_group_key("\u{200B}\u{200C}").as_ref(), "");
}

#[gtk::test]
fn expected_list_len_tracks_collapsed_expanded_and_filtered_groups() {
    let mut list = support::make_list();
    list.seed(
        vec![
            support::notification(1, "Terminal"),
            support::notification(2, "Terminal"),
            support::notification(3, "Browser"),
        ],
        Vec::new(),
    );
    list.flush_rebuild();
    let terminal = list.entries.get(&2).expect("terminal").app_key.clone();

    assert_eq!(list.expected_list_len(), 3);

    list.group_expanded.insert(terminal, true);
    assert_eq!(list.expected_list_len(), 4);

    assert!(list.set_filter_query("browser"));
    list.flush_rebuild();
    assert_eq!(list.expected_list_len(), 1);
}

#[gtk::test]
fn group_visibility_and_entry_filter_cover_app_summary_and_body() {
    let mut list = support::make_list();
    let mut terminal = support::notification(1, "Terminal");
    terminal.summary = "Build complete".to_string();
    terminal.body = "Package uploaded".to_string();
    list.seed(
        vec![terminal, support::notification(2, "Browser")],
        Vec::new(),
    );
    let terminal_key = list.entries.get(&1).expect("terminal").app_key.clone();
    let terminal_ids = list.grouped_cache.get(&terminal_key).expect("ids").clone();

    assert!(!list.group_has_visible_entries(&[]));
    assert!(list.group_has_visible_entries(&terminal_ids));

    assert!(list.set_filter_query("uploaded"));
    assert!(list.group_has_visible_entries(&terminal_ids));

    assert!(list.set_filter_query("missing"));
    assert!(!list.group_has_visible_entries(&terminal_ids));
}

#[gtk::test]
fn notification_counts_report_matches_and_total_for_active_search() {
    let mut list = support::make_list();
    let mut terminal = support::notification(1, "Terminal");
    terminal.body = "Build complete".to_string();
    list.seed(
        vec![terminal, support::notification(2, "Browser")],
        vec![support::notification(3, "Terminal history")],
    );

    let counts = list.notification_counts();
    assert_eq!(counts.matching, 3);
    assert_eq!(counts.total, 3);
    assert!(!counts.filter_active);

    assert!(list.set_filter_query("terminal"));
    let counts = list.notification_counts();
    assert_eq!(counts.matching, 2);
    assert_eq!(counts.total, 3);
    assert!(counts.filter_active);

    assert!(list.set_filter_query("missing"));
    let counts = list.notification_counts();
    assert_eq!(counts.matching, 0);
    assert_eq!(counts.total, 3);
    assert!(counts.filter_active);
}

#[test]
fn ignorable_group_chars_cover_controls_and_zero_width_marks() {
    assert!(is_ignorable_group_char('\n'));
    assert!(is_ignorable_group_char('\u{200B}'));
    assert!(is_ignorable_group_char('\u{FEFF}'));
    assert!(!is_ignorable_group_char('a'));
}
