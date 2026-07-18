use std::path::Path;

use super::super::model::ImportedCssReference;
use super::super::reference::classify_imported_css_reference;

const PNG_DATA_URL: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=";

#[test]
fn reference_classifier_resolves_percent_encoded_bundle_paths() {
    let target = classify_imported_css_reference(
        Path::new("/tmp/config"),
        Path::new("themes/widgets.css"),
        "../assets/icon%20one.png",
        1024,
    )
    .expect("classify relative asset");

    assert!(matches!(
        target,
        ImportedCssReference::Bundled(path) if path == Path::new("assets/icon one.png")
    ));
}

#[test]
fn reference_classifier_decodes_supported_data_images_under_the_byte_limit() {
    let target = classify_imported_css_reference(
        Path::new("/tmp/config"),
        Path::new("widgets.css"),
        PNG_DATA_URL,
        1024,
    )
    .expect("decode data image");
    let decoded_len = match &target {
        ImportedCssReference::Data { contents, .. } => contents.len() as u64,
        other => panic!("unexpected data image classification: {other:?}"),
    };

    assert!(matches!(
        target,
        ImportedCssReference::Data { path_hint, contents }
            if path_hint == Path::new("inline.png") && contents.starts_with(b"\x89PNG")
    ));

    classify_imported_css_reference(
        Path::new("/tmp/config"),
        Path::new("widgets.css"),
        PNG_DATA_URL,
        decoded_len,
    )
    .expect("accept data image at exact decoded-byte limit");
}

#[test]
fn reference_classifier_rejects_oversized_and_unsupported_data_images() {
    let oversized = classify_imported_css_reference(
        Path::new("/tmp/config"),
        Path::new("widgets.css"),
        PNG_DATA_URL,
        4,
    )
    .expect_err("reject oversized data image");
    let unsupported = classify_imported_css_reference(
        Path::new("/tmp/config"),
        Path::new("widgets.css"),
        "data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///ywAAAAAAQABAAACAUwAOw==",
        1024,
    )
    .expect_err("reject unsupported data image");

    assert!(oversized.to_string().contains("exceeds"));
    assert!(unsupported.to_string().contains("unsupported media type"));
}

#[test]
fn reference_classifier_keeps_absolute_uris_outside_the_bundle_class() {
    for value in [
        "file:///tmp/image.png",
        "https://example.invalid/image.png",
        "custom:asset-name",
        "/tmp/image.png",
        "/tmp/config/assets/image.png",
        "~/assets/image.png",
    ] {
        let target = classify_imported_css_reference(
            Path::new("/tmp/config"),
            Path::new("widgets.css"),
            value,
            1024,
        )
        .expect("classify external reference");
        assert!(matches!(target, ImportedCssReference::External));
    }
}

#[test]
fn reference_classifier_rejects_empty_null_and_encoded_query_or_fragment_values() {
    for value in ["", "assets/icon\0name.png"] {
        assert!(classify_imported_css_reference(
            Path::new("/tmp/config"),
            Path::new("widgets.css"),
            value,
            1024,
        )
        .is_err());
    }
    for value in [
        "assets/icon.png?size=1",
        "assets/icon.png#fragment",
        "assets/icon%20one.png?size=1",
        "assets/icon%20one.png#fragment",
    ] {
        let error = classify_imported_css_reference(
            Path::new("/tmp/config"),
            Path::new("widgets.css"),
            value,
            1024,
        )
        .expect_err("reject query or fragment on portable relative URL");

        assert!(error.to_string().contains("queries or fragments"));
    }
}

#[test]
fn reference_classifier_preserves_invalid_percent_bytes_as_literal_path_text() {
    let target = classify_imported_css_reference(
        Path::new("/tmp/config"),
        Path::new("themes/widgets.css"),
        "../assets/icon%ZZ.png",
        1024,
    )
    .expect("classify literal invalid percent bytes");

    assert!(matches!(
        target,
        ImportedCssReference::Bundled(path) if path == Path::new("assets/icon%ZZ.png")
    ));
}
