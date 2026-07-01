use super::*;

#[test]
fn collect_url_spans_ignores_comment_bodies_and_handles_real_urls_afterward() {
    let css = "/* url(ignored.png) */\n.real { background: url(real.png); }";

    let spans = collect_url_spans(css);

    // Comment text can look like CSS, but it must not trigger asset rewrites
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].value, "real.png");
}

#[test]
fn collect_url_spans_tracks_multiple_urls_without_overlapping_matches() {
    let css = ".a { background: url(one.png); }\n.b { mask: url(\"two.svg\"); }";

    let spans = collect_url_spans(css);

    assert_eq!(spans.len(), 2);
    assert_eq!(spans[0].value, "one.png");
    assert_eq!(spans[1].value, "two.svg");
    assert!(spans[0].value_end < spans[1].value_start);
}

#[test]
fn collect_url_spans_resumes_after_comment_close_on_same_line() {
    let css = "/* url(ignored.png) */ .real { background: URL(real.svg); }";

    let spans = collect_url_spans(css);

    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].value, "real.svg");
}

#[test]
fn collect_url_spans_stops_safely_on_unclosed_url() {
    let css = ".bad { background: url(unclosed.png";

    let spans = collect_url_spans(css);

    // Malformed trailing url(...) syntax should leave the original CSS untouched
    assert!(spans.is_empty());
}
