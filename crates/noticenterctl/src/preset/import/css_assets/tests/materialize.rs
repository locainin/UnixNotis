use std::path::Path;

use super::super::harden_imported_css_assets;
use super::super::materialize::{
    validate_materialized_png_size, CssAssetMaterializer, MaterializationLimits,
};
use super::super::model::IncludedBundleFiles;
use super::support::{bundle_file, PNG_BYTES};
use crate::preset::archive::MAX_PRESET_FILE_BYTES;

#[test]
fn hardening_materializes_escaped_local_image_references_as_validated_pngs() {
    let mut files = vec![
        bundle_file(
            "themes/widgets.css",
            br#".card { background: u\72l("../assets/icon.png"); }"#.to_vec(),
        ),
        bundle_file("assets/icon.png", PNG_BYTES.to_vec()),
    ];

    harden_imported_css_assets(Path::new("/tmp/config"), &mut files, &[])
        .expect("harden local CSS image");

    let css = files
        .iter()
        .find(|file| file.relative_path == Path::new("themes/widgets.css"))
        .expect("rewritten stylesheet");
    let css = std::str::from_utf8(&css.contents).expect("UTF-8 stylesheet");
    assert!(css.contains("../assets/.validated-css/"));
    assert!(!css.contains("../assets/icon.png"));
    assert!(files.iter().any(|file| {
        file.relative_path.starts_with("assets/.validated-css")
            && file.contents.starts_with(b"\x89PNG")
            && file.mode == 0o644
    }));
}

#[test]
fn materializer_enforces_generated_file_and_byte_limits_before_retaining_output() {
    let png = bundle_file("assets/one.png", PNG_BYTES.to_vec());
    let svg = bundle_file(
        "assets/two.svg",
        br##"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1">
            <rect width="1" height="1" fill="#123456"/>
        </svg>"##
            .to_vec(),
    );
    let available = IncludedBundleFiles::from([
        (png.relative_path.clone(), &png),
        (svg.relative_path.clone(), &svg),
    ]);
    let mut file_limited = CssAssetMaterializer::with_limits(
        Path::new("/tmp/config"),
        MaterializationLimits {
            max_files: 1,
            max_bytes: u64::MAX,
        },
    );

    file_limited
        .materialize_reference(&available, Path::new("base.css"), "assets/one.png")
        .expect("materialize first image within file limit");
    let file_error = file_limited
        .materialize_reference(&available, Path::new("base.css"), "assets/two.svg")
        .expect_err("reject second generated image");

    assert!(file_error.to_string().contains("generated-file limit"));

    let mut byte_limited = CssAssetMaterializer::with_limits(
        Path::new("/tmp/config"),
        MaterializationLimits {
            max_files: usize::MAX,
            max_bytes: 1,
        },
    );
    let byte_error = byte_limited
        .materialize_reference(&available, Path::new("base.css"), "assets/one.png")
        .expect_err("reject generated image above byte limit");

    assert!(byte_error.to_string().contains("generated-byte limit"));

    let mut baseline = CssAssetMaterializer::with_limits(
        Path::new("/tmp/config"),
        MaterializationLimits {
            max_files: usize::MAX,
            max_bytes: u64::MAX,
        },
    );
    baseline
        .materialize_reference(&available, Path::new("base.css"), "assets/one.png")
        .expect("measure generated image size");
    let exact_bytes = baseline
        .into_generated()
        .values()
        .map(Vec::len)
        .sum::<usize>() as u64;
    let mut exact_byte_limit = CssAssetMaterializer::with_limits(
        Path::new("/tmp/config"),
        MaterializationLimits {
            max_files: usize::MAX,
            max_bytes: exact_bytes,
        },
    );

    exact_byte_limit
        .materialize_reference(&available, Path::new("base.css"), "assets/one.png")
        .expect("accept generated image at exact byte limit");
}

#[test]
fn materialized_png_limit_accepts_exact_boundary_and_rejects_one_over() {
    validate_materialized_png_size(MAX_PRESET_FILE_BYTES)
        .expect("accept exact materialized image limit");

    assert!(validate_materialized_png_size(MAX_PRESET_FILE_BYTES.saturating_add(1)).is_err());
}
