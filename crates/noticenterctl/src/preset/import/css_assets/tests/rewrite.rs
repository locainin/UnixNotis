use std::path::Path;

use super::super::harden_imported_css_assets;
use super::super::rewrite::validate_rewritten_css_size;
use super::support::bundle_file;
use crate::preset::archive::MAX_PRESET_FILE_BYTES;

#[test]
fn hardening_keeps_included_css_imports_as_stylesheets() {
    let mut files = vec![
        bundle_file(
            "base.css",
            br#"@im\70ort u\72l("themes/colors.css");"#.to_vec(),
        ),
        bundle_file("themes/colors.css", b".card { color: red; }".to_vec()),
    ];

    harden_imported_css_assets(Path::new("/tmp/config"), &mut files, &[])
        .expect("validate local CSS import");

    assert_eq!(files.len(), 2);
    assert!(files[0]
        .contents
        .windows(17)
        .any(|bytes| bytes == b"themes/colors.css"));
}

#[test]
fn hardening_rejects_missing_images_and_ambiguous_payload_escapes() {
    let mut missing = vec![bundle_file(
        "base.css",
        b".card { background: url(missing.png); }".to_vec(),
    )];
    let mut ambiguous = vec![bundle_file(
        "base.css",
        br#".card { background: url("\2f tmp/image.png"); }"#.to_vec(),
    )];

    let missing_error = harden_imported_css_assets(Path::new("/tmp/config"), &mut missing, &[])
        .expect_err("reject missing image");
    let ambiguous_error = harden_imported_css_assets(Path::new("/tmp/config"), &mut ambiguous, &[])
        .expect_err("reject ambiguous URL payload");

    assert!(missing_error.to_string().contains("not included"));
    assert!(ambiguous_error.to_string().contains("ambiguous escaped"));
}

#[test]
fn hardening_rejects_invalid_local_and_embedded_import_targets() {
    let cases = [
        (
            vec![bundle_file(
                "base.css",
                b"@import \"missing.css\";".to_vec(),
            )],
            "not included",
        ),
        (
            vec![
                bundle_file("base.css", b"@import \"theme.txt\";".to_vec()),
                bundle_file("theme.txt", b".card { color: red; }".to_vec()),
            ],
            ".css extension",
        ),
        (
            vec![bundle_file(
                "base.css",
                b"@import \"data:image/png;base64,AAAA\";".to_vec(),
            )],
            "does not accept data image",
        ),
        (
            vec![bundle_file("base.css", b"@import var(--theme);".to_vec())],
            "ambiguous CSS import",
        ),
    ];

    for (mut files, expected) in cases {
        let error = harden_imported_css_assets(Path::new("/tmp/config"), &mut files, &[])
            .expect_err("reject invalid CSS import target");

        assert!(
            error.to_string().contains(expected),
            "unexpected error for {expected}: {error:#}"
        );
    }
}

#[test]
fn hardening_rejects_imported_stylesheets_that_are_not_utf8() {
    let mut files = vec![
        bundle_file("base.css", b"@import \"theme.css\";".to_vec()),
        bundle_file("theme.css", vec![0xff, 0xfe]),
    ];

    let error = harden_imported_css_assets(Path::new("/tmp/config"), &mut files, &[])
        .expect_err("reject non-UTF-8 imported stylesheet");

    assert!(error.to_string().contains("not valid UTF-8"));
}

#[test]
fn rewritten_css_limit_accepts_exact_boundary_and_rejects_one_over() {
    validate_rewritten_css_size(MAX_PRESET_FILE_BYTES, Path::new("base.css"))
        .expect("accept exact rewritten CSS limit");

    assert!(validate_rewritten_css_size(
        MAX_PRESET_FILE_BYTES.saturating_add(1),
        Path::new("base.css"),
    )
    .is_err());
}
