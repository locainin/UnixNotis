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

#[test]
fn rewrite_percent_encodes_decoded_file_url_characters_in_quoted_and_unquoted_forms() {
    let cases = [
        ("icon%20one.png", "icon%20one.png"),
        ("icon%23one.png", "icon%23one.png"),
        ("icon%25one.png", "icon%25one.png"),
        ("icon%29one.png", "icon%29one.png"),
        ("icon%22one.png", "icon%22one.png"),
        ("icon%28one.png", "icon%28one.png"),
        ("icon%27one.png", "icon%27one.png"),
    ];

    for (encoded_name, expected_name) in cases {
        let file_url = format!("file:///config/unixnotis/assets/{encoded_name}");
        for (input, expected) in [
            (
                format!(".a {{ background: url({file_url}); }}\n"),
                format!(".a {{ background: url(assets/{expected_name}); }}\n"),
            ),
            (
                format!(".a {{ background: url(\"{file_url}\"); }}\n"),
                format!(".a {{ background: url(\"assets/{expected_name}\"); }}\n"),
            ),
        ] {
            let (rewritten, findings) = rewrite_host_specific_refs_in_text(
                Path::new("/config/unixnotis"),
                Path::new("/config/unixnotis/base.css"),
                &input,
            )
            .expect("rewrite encoded file URL");

            assert_eq!(rewritten, expected, "failed encoded name {encoded_name}");
            assert_eq!(findings.len(), 1, "missing finding for {encoded_name}");
            assert_eq!(findings[0].rewritten_ref, format!("assets/{expected_name}"));
        }
    }
}

#[test]
fn rewrite_preserves_unicode_whitespace_without_invalid_utf8_ranges() {
    let whitespace = [
        '\u{0085}', '\u{00A0}', '\u{1680}', '\u{2000}', '\u{2001}', '\u{2002}', '\u{2003}',
        '\u{2004}', '\u{2005}', '\u{2006}', '\u{2007}', '\u{2008}', '\u{2009}', '\u{200A}',
        '\u{2028}', '\u{2029}', '\u{202F}', '\u{205F}', '\u{3000}',
    ];

    for character in whitespace {
        for css in [
            format!(".a {{ background: url({character}asset.png); }}"),
            format!(".a {{ background: url(asset.png{character}); }}"),
            format!(".a {{ background: url(\"{character}asset.png{character}\"); }}"),
        ] {
            let (rewritten, findings) = rewrite_host_specific_refs_in_text(
                Path::new("/config/unixnotis"),
                Path::new("/config/unixnotis/base.css"),
                &css,
            )
            .expect("rewrite Unicode CSS URL safely");

            assert_eq!(rewritten, css);
            assert!(findings.is_empty());
        }
    }
}
