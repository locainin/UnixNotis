use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use rustix::fs::{mkfifoat, Mode, CWD};

use super::super::dependencies::{collect_import_dependency_states, imported_css_paths};
use super::helpers::TempDirGuard;

#[test]
fn dependency_scan_rejects_fifo_import_without_blocking() {
    let root = TempDirGuard::new("dependency-fifo");
    let fifo = root.path().join("outside.css");
    mkfifoat(CWD, &fifo, Mode::RUSR | Mode::WUSR).expect("create FIFO");
    let css_path = root.write(
        "config/base.css",
        &format!("@import \"{}\";", fifo.display()),
    );

    let started = Instant::now();
    let error = collect_import_dependency_states(&css_path)
        .expect_err("a CSS import must not read from a FIFO");

    assert!(started.elapsed() < Duration::from_secs(1));
    assert!(error.to_string().contains("not a regular file"));
}

#[test]
fn dependency_scan_rejects_import_depth_over_limit() {
    let root = TempDirGuard::new("dependency-depth");
    for index in 0..=33 {
        let contents = if index == 33 {
            ".last { color: red; }".to_string()
        } else {
            format!("@import \"{}.css\";", index + 1)
        };
        root.write(&format!("config/{index}.css"), &contents);
    }

    let error = collect_import_dependency_states(&root.path().join("config/0.css"))
        .expect_err("deep CSS import chains must stay bounded");

    assert!(error.to_string().contains("import depth exceeds"));
}

#[test]
fn dependency_scan_accepts_the_exact_import_depth_limit() {
    let root = TempDirGuard::new("dependency-depth-exact");
    for index in 0..=32 {
        let contents = if index == 32 {
            ".last { color: red; }".to_string()
        } else {
            format!("@import \"{}.css\";", index + 1)
        };
        root.write(&format!("config/{index}.css"), &contents);
    }

    let dependencies = collect_import_dependency_states(&root.path().join("config/0.css"))
        .expect("exact CSS import depth must remain valid");

    assert_eq!(dependencies.len(), 32);
}

#[test]
fn dependency_scan_rejects_too_many_import_paths() {
    let root = TempDirGuard::new("dependency-count");
    let imports = (0..=256)
        .map(|index| format!("@import \"missing-{index}.css\";"))
        .collect::<Vec<_>>()
        .join("\n");
    let css_path = root.write("config/base.css", &imports);

    let error = collect_import_dependency_states(&css_path)
        .expect_err("CSS dependency count must stay bounded");

    assert!(error.to_string().contains("imports exceed 256 files"));
}

#[test]
fn dependency_scan_accepts_the_exact_import_file_limit() {
    let root = TempDirGuard::new("dependency-count-exact");
    let imports = (0..256)
        .map(|index| format!("@import \"missing-{index}.css\";"))
        .collect::<Vec<_>>()
        .join("\n");
    let css_path = root.write("config/base.css", &imports);

    let dependencies = collect_import_dependency_states(&css_path).expect("exact dependency count");

    assert_eq!(dependencies.len(), 256);
}

#[test]
fn dependency_scan_rejects_oversized_import_before_reading_it() {
    let root = TempDirGuard::new("dependency-size");
    let imported = root.write("config/large.css", "");
    fs::OpenOptions::new()
        .write(true)
        .open(&imported)
        .expect("open imported CSS")
        .set_len(16_777_217)
        .expect("grow imported CSS");
    let css_path = root.write("config/base.css", "@import \"large.css\";");

    let error = collect_import_dependency_states(&css_path)
        .expect_err("large imported CSS must stay bounded");

    assert!(error.to_string().contains("CSS file exceeds"));
}

#[test]
fn dependency_parser_handles_case_url_forms_and_ignores_false_positives() {
    let root = TempDirGuard::new("dependency-syntax");
    let css_path = root.path().join("config/base.css");
    let contents = "/* @import \"comment.css\"; */\n\
                    .a { content: '@import \"string.css\";'; }\n\
                    @IMPORTANT \"not-an-import.css\";\n\
                    @IMPORT URL(\"upper.css\");\n\
                    @import url('single.css');\n\
                    @import \"quoted.css\";";

    let paths = imported_css_paths(contents, &css_path).expect("parse imports");

    assert_eq!(
        paths,
        ["upper.css", "single.css", "quoted.css"].map(|name| root.path().join("config").join(name))
    );
}

#[test]
fn dependency_parser_tracks_local_targets_and_ignores_remote_schemes() {
    let root = TempDirGuard::new("dependency-targets");
    let css_path = root.path().join("config/base.css");
    let local_file = root.path().join("shared/file.css");
    let localhost_file = root.path().join("shared/localhost.css");
    let contents = format!(
        "@import \"http://example.invalid/a.css\";\n\
         @import \"https://example.invalid/b.css\";\n\
         @import \"data:text/css,body{{}}\";\n\
         @import \"//example.invalid/c.css\";\n\
         @import \"ftp://example.invalid/d.css\";\n\
         @import \"file://{}\";\n\
         @import \"file://localhost{}\";\n\
         @import \"/absolute/local.css\";\n\
         @import \"relative.css\";",
        local_file.display(),
        localhost_file.display()
    );

    let paths = imported_css_paths(&contents, &css_path).expect("classify import targets");

    assert_eq!(
        paths,
        vec![
            local_file,
            localhost_file,
            PathBuf::from("/absolute/local.css"),
            root.path().join("config/relative.css"),
        ]
    );
}
