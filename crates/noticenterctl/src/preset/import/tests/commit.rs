use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;
use crate::preset::archive::BundleFile;
use crate::preset::import::commit::commit_import_plan;
use crate::preset::import::plan::build_import_plan;

fn bundle_file(relative_path: &str, contents: &str) -> BundleFile {
    BundleFile {
        relative_path: PathBuf::from(relative_path),
        contents: contents.as_bytes().to_vec(),
        mode: 0o644,
    }
}

#[test]
fn commit_import_plan_writes_files_runs_css_check_and_returns_backup() {
    let import_root = TempDirGuard::new("commit-success");
    import_root.write("config.toml", "[panel]\nwidth = 320\n");

    let plan = build_import_plan(
        &import_root.path,
        vec![
            bundle_file("config.toml", "[panel]\nwidth = 444\n"),
            bundle_file("theme/base.css", ".panel { color: red; }\n"),
        ],
        &[],
    )
    .expect("build plan");
    let css_calls = AtomicUsize::new(0);

    let (backup_dir, css_result) = commit_import_plan(&import_root.path, &plan, || {
        css_calls.fetch_add(1, Ordering::Relaxed);
        Ok(())
    })
    .expect("commit import");

    assert_eq!(css_calls.load(Ordering::Relaxed), 1);
    assert!(css_result.is_ok());
    assert_eq!(
        fs::read_to_string(import_root.path.join("config.toml")).expect("read imported config"),
        "[panel]\nwidth = 444\n"
    );
    assert_eq!(
        fs::read_to_string(import_root.path.join("theme/base.css")).expect("read imported css"),
        ".panel { color: red; }\n"
    );
    let backup_dir = backup_dir.expect("overwritten config should create backup");
    assert_eq!(
        fs::read_to_string(backup_dir.join("config.toml")).expect("read backup config"),
        "[panel]\nwidth = 320\n"
    );
}

#[test]
fn commit_import_plan_rolls_back_when_imported_config_cannot_load() {
    let import_root = TempDirGuard::new("commit-invalid-config");
    import_root.write("config.toml", "[panel]\nwidth = 320\n");

    let plan = build_import_plan(
        &import_root.path,
        vec![bundle_file("config.toml", "[panel\nwidth = broken\n")],
        &[],
    )
    .expect("build plan");
    let css_calls = AtomicUsize::new(0);

    let error = commit_import_plan(&import_root.path, &plan, || {
        css_calls.fetch_add(1, Ordering::Relaxed);
        Ok(())
    })
    .expect_err("invalid imported config should fail");

    assert!(error.to_string().contains("load imported config.toml"));
    assert_eq!(css_calls.load(Ordering::Relaxed), 0);
    assert_eq!(
        fs::read_to_string(import_root.path.join("config.toml")).expect("read restored config"),
        "[panel]\nwidth = 320\n"
    );
}

#[test]
fn commit_import_plan_rolls_back_when_imported_config_points_outside_root() {
    let import_root = TempDirGuard::new("commit-outside-theme");
    import_root.write("config.toml", "[panel]\nwidth = 320\n");
    let outside_theme = import_root.path.with_file_name("outside-theme.css");

    let plan = build_import_plan(
        &import_root.path,
        vec![bundle_file(
            "config.toml",
            &format!(
                "[theme]\nbase_css = {:?}\n",
                outside_theme.display().to_string()
            ),
        )],
        &[],
    )
    .expect("build plan");
    let css_calls = AtomicUsize::new(0);

    let error = commit_import_plan(&import_root.path, &plan, || {
        css_calls.fetch_add(1, Ordering::Relaxed);
        Ok(())
    })
    .expect_err("outside theme path should fail");

    assert!(error
        .to_string()
        .contains("tries to leave the UnixNotis config directory"));
    assert_eq!(css_calls.load(Ordering::Relaxed), 0);
    assert_eq!(
        fs::read_to_string(import_root.path.join("config.toml")).expect("read restored config"),
        "[panel]\nwidth = 320\n"
    );
    assert!(!outside_theme.exists());
}
