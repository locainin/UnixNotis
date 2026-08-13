use super::truncate_utf8_bytes;

#[test]
fn truncation_keeps_values_that_fit_the_byte_budget() {
    assert_eq!(truncate_utf8_bytes("plain", 5), "plain");
    assert_eq!(truncate_utf8_bytes("🙂", 4), "🙂");
}

#[test]
fn truncation_returns_empty_text_for_a_zero_byte_budget() {
    assert_eq!(truncate_utf8_bytes("text", 0), "");
}

#[test]
fn truncation_stops_before_a_partial_multibyte_character() {
    assert_eq!(truncate_utf8_bytes("abc🙂def", 5), "abc");
    assert_eq!(truncate_utf8_bytes("éé", 3), "é");
}

#[test]
fn truncation_accepts_every_boundary_inside_a_four_byte_character() {
    for limit in 1..4 {
        assert_eq!(truncate_utf8_bytes("🙂tail", limit), "");
    }
}
