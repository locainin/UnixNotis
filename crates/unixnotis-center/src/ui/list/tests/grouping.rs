use super::*;

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
