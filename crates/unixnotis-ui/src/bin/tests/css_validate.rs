use std::error::Error;
use std::fmt::Write as _;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::{parse_css_text, path_report};

type TestResult = Result<(), Box<dyn Error + Send + Sync>>;

static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

#[gtk::test]
fn css_validator_accepts_parseable_css_and_rejects_invalid_css() -> TestResult {
    assert_eq!(parse_css_text(".panel { color: #ffffff; }"), 0);
    assert!(parse_css_text(".panel { color: ;") > 0);
    Ok(())
}

#[gtk::test]
fn path_protocol_accepts_percent_encoded_asset_urls() -> TestResult {
    let root = temp_root("encoded-imports");
    let assets = root.join("assets");
    std::fs::create_dir_all(&assets)?;
    let cases = [
        ("icon%20one.svg", "icon one.svg"),
        ("icon%23one.svg", "icon#one.svg"),
        ("icon%25one.svg", "icon%one.svg"),
        ("icon%29one.svg", "icon)one.svg"),
        ("icon%22one.svg", "icon\"one.svg"),
    ];
    let mut stylesheet = String::new();
    for (index, (encoded_name, decoded_name)) in cases.into_iter().enumerate() {
        std::fs::write(
            assets.join(decoded_name),
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1\" height=\"1\"><rect width=\"1\" height=\"1\" fill=\"red\"/></svg>",
        )?;
        writeln!(
            stylesheet,
            ".encoded-{index}-plain {{ background-image: url(assets/{encoded_name}); }}"
        )?;
        writeln!(
            stylesheet,
            ".encoded-{index}-quoted {{ background-image: url(\"assets/{encoded_name}\"); }}"
        )?;
    }
    let stylesheet_path = root.join("base.css");
    std::fs::write(&stylesheet_path, stylesheet)?;

    let report = path_report(&stylesheet_path);

    if report.available {
        assert!(report.diagnostics.is_empty());
    }
    std::fs::remove_dir_all(root)?;
    Ok(())
}

#[gtk::test]
fn path_protocol_accepts_css_escaped_url_and_import_names() -> TestResult {
    let root = temp_root("escaped-reference-tokens");
    let assets = root.join("assets");
    std::fs::create_dir_all(&assets)?;
    std::fs::write(
        assets.join("icon.svg"),
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1\" height=\"1\"/>",
    )?;
    std::fs::write(root.join("colors.css"), ".imported { color: red; }")?;
    let stylesheet = root.join("base.css");
    std::fs::write(
        &stylesheet,
        concat!(
            "@im\\70ort \"colors.css\";\n",
            ".short { background-image: u\\72l(\"assets/icon.svg\"); }\n",
            ".six { background-image: U\\000052L(assets/icon.svg); }\n",
        ),
    )?;

    let report = path_report(&stylesheet);

    if report.available {
        assert!(report.diagnostics.is_empty());
    }
    std::fs::remove_dir_all(root)?;
    Ok(())
}

#[gtk::test]
fn path_protocol_returns_bounded_structured_diagnostics() -> TestResult {
    let root = temp_root("diagnostic-cap");
    std::fs::create_dir_all(&root)?;
    let stylesheet = root.join("many-errors.css");
    let mut css = String::new();
    for index in 0..12 {
        writeln!(css, ".broken-{index} {{ color: ; }}")?;
    }
    std::fs::write(&stylesheet, css)?;

    let report = path_report(&stylesheet);

    if report.available {
        assert!(!report.diagnostics.is_empty());
        assert!(report.diagnostics.len() <= 4);
        assert!(report.truncated);
        assert_eq!(report.diagnostics[0].line, 1);
    }
    std::fs::remove_dir_all(root)?;
    Ok(())
}

fn temp_root(name: &str) -> std::path::PathBuf {
    let serial = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "unixnotis-css-validator-{name}-{}-{serial}",
        std::process::id()
    ))
}
