use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;
use crate::preset::archive::BundleFile;
use crate::preset::import::apply::fail_backup_write_after_for_test;
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
fn apply_failure_on_later_file_rolls_back_earlier_publication() {
    let import_root = TempDirGuard::new("later-apply-failure");
    import_root.write("config.toml", "[panel]\nwidth = 320\n");
    let plan = build_import_plan(
        &import_root.path,
        vec![
            bundle_file("config.toml", "[panel]\nwidth = 444\n"),
            bundle_file("theme/base.css", ".new { color: red; }\n"),
        ],
        &[],
    )
    .expect("build plan");
    fs::write(import_root.path.join("theme"), "blocks directory creation")
        .expect("create blocking file");

    commit_import_plan(&import_root.path, &plan, || Ok(()))
        .expect_err("later publication should fail");

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

#[test]
fn commit_import_plan_rolls_back_when_imported_command_points_outside_root() {
    let import_root = TempDirGuard::new("commit-outside-command");
    import_root.write("config.toml", "[panel]\nwidth = 320\n");
    let outside_command = import_root.path.with_file_name("outside-command.sh");

    let plan = build_import_plan(
        &import_root.path,
        vec![
            bundle_file(
                "config.toml",
                &format!(
                    "[[widgets.stats]]\nlabel = \"Probe\"\ncmd = {:?}\n",
                    outside_command.display().to_string()
                ),
            ),
            bundle_file("scripts/probe.sh", "#!/bin/sh\necho should-not-stay\n"),
        ],
        &[],
    )
    .expect("build plan");
    let css_calls = AtomicUsize::new(0);

    let error = commit_import_plan(&import_root.path, &plan, || {
        css_calls.fetch_add(1, Ordering::Relaxed);
        Ok(())
    })
    .expect_err("outside command path should fail");

    assert!(error.to_string().contains("preset import blocked"));
    assert_eq!(css_calls.load(Ordering::Relaxed), 0);
    assert_eq!(
        fs::read_to_string(import_root.path.join("config.toml")).expect("read restored config"),
        "[panel]\nwidth = 320\n"
    );
    assert!(!import_root.path.join("scripts/probe.sh").exists());
    assert!(!outside_command.exists());
}

#[test]
fn commit_import_plan_cleans_partial_backup_and_rolls_back_when_backup_write_fails() {
    let import_root = TempDirGuard::new("commit-backup-failure");
    import_root.write("config.toml", "[panel]\nwidth = 320\n");
    import_root.write("theme/base.css", ".old { color: blue; }\n");

    let plan = build_import_plan(
        &import_root.path,
        vec![
            bundle_file("config.toml", "[panel]\nwidth = 444\n"),
            bundle_file("theme/base.css", ".new { color: red; }\n"),
        ],
        &[],
    )
    .expect("build plan");
    let _failure = fail_backup_write_after_for_test(1);
    let css_calls = AtomicUsize::new(0);

    let error = commit_import_plan(&import_root.path, &plan, || {
        css_calls.fetch_add(1, Ordering::Relaxed);
        Ok(())
    })
    .expect_err("backup failure should rollback");

    assert!(error.to_string().contains("forced backup write failure"));
    assert_eq!(css_calls.load(Ordering::Relaxed), 1);
    assert_eq!(
        fs::read_to_string(import_root.path.join("config.toml")).expect("read restored config"),
        "[panel]\nwidth = 320\n"
    );
    assert_eq!(
        fs::read_to_string(import_root.path.join("theme/base.css")).expect("read restored css"),
        ".old { color: blue; }\n"
    );
    let backup_dirs = fs::read_dir(&import_root.path)
        .expect("read import root")
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().starts_with("Backup-"))
        .count();
    assert_eq!(backup_dirs, 0);
}

#[test]
fn commit_import_plan_keeps_import_committed_when_css_check_fails() {
    let import_root = TempDirGuard::new("commit-css-failure");
    import_root.write("config.toml", "[panel]\nwidth = 320\n");

    let plan = build_import_plan(
        &import_root.path,
        vec![bundle_file("config.toml", "[panel]\nwidth = 444\n")],
        &[],
    )
    .expect("build plan");

    let (backup_dir, css_result) = commit_import_plan(&import_root.path, &plan, || {
        Err(anyhow!("css-check failed for test"))
    })
    .expect("import should commit before reporting css-check failure");

    assert!(backup_dir.is_some());
    assert!(css_result
        .expect_err("css-check failure should be returned")
        .to_string()
        .contains("css-check failed for test"));
    assert_eq!(
        fs::read_to_string(import_root.path.join("config.toml")).expect("read committed config"),
        "[panel]\nwidth = 444\n"
    );
}
