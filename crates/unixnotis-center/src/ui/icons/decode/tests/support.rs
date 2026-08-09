use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use image::codecs::png::PngEncoder;
use image::{ExtendedColorType, ImageEncoder};

pub(super) fn test_root(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "unixnotis-center-icon-{name}-{}-{stamp}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("create icon test root");
    root
}

pub(super) fn png_bytes(width: u32, height: u32) -> Vec<u8> {
    let pixels = vec![0x7f; usize::try_from(width * height * 4).expect("small PNG size")];
    let mut bytes = Vec::new();
    PngEncoder::new(&mut bytes)
        .write_image(&pixels, width, height, ExtendedColorType::Rgba8)
        .expect("encode PNG");
    bytes
}

pub(super) fn svg_renderer_binary() -> &'static PathBuf {
    static BINARY: OnceLock<PathBuf> = OnceLock::new();

    BINARY.get_or_init(|| {
        if let Some(path) = option_env!("CARGO_BIN_EXE_unixnotis-svg-renderer") {
            return path.into();
        }
        let current_exe = std::env::current_exe().expect("current center test binary");
        let profile_dir = current_exe
            .parent()
            .and_then(|path| path.parent())
            .expect("Cargo profile directory");
        let target_root = profile_dir.parent().expect("Cargo target root");
        let candidate = profile_dir.join(format!(
            "unixnotis-svg-renderer{}",
            std::env::consts::EXE_SUFFIX
        ));
        if fs::metadata(&candidate).is_err() {
            build_svg_renderer(target_root);
        }
        assert!(
            fs::metadata(&candidate).is_ok(),
            "SVG renderer binary is missing at {candidate:?}"
        );
        candidate
    })
}

pub(super) fn renderer_fixture(name: &str) -> PathBuf {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/svg-renderers")
        .join(name);
    assert!(
        fs::metadata(&fixture).is_ok_and(|metadata| metadata.is_file()),
        "SVG renderer fixture is missing at {fixture:?}"
    );
    fixture
}

fn build_svg_renderer(target_root: &std::path::Path) {
    // Unit-test targets do not guarantee that the non-test helper binary was built
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| String::from("cargo"));
    let output = Command::new(cargo)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args(["build", "--bin", "unixnotis-svg-renderer", "--target-dir"])
        .arg(target_root)
        .output()
        .expect("build SVG renderer for center tests");
    assert!(
        output.status.success(),
        "failed to build SVG renderer\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
