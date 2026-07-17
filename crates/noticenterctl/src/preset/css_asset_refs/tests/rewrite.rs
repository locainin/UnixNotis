use std::path::Path;

use super::rewrite_host_specific_refs_in_text;

#[test]
fn rewrite_keeps_ambiguous_escaped_url_unchanged() {
    let css = ".a { background: url(\"\\2f config/unixnotis/image.png\"); }\n";

    let (rewritten, findings) = rewrite_host_specific_refs_in_text(
        Path::new("/config/unixnotis"),
        Path::new("/config/unixnotis/base.css"),
        css,
    )
    .expect("rewrite CSS");

    assert_eq!(rewritten, css);
    assert!(findings.is_empty());
}
