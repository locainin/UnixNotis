use std::cell::Cell;
use std::fs;

use super::helpers::{pause_for_metadata_tick, validate_with_counter, TempDirGuard};

#[test]
fn edited_files_always_revalidate() {
    let root = TempDirGuard::new("edited-file");
    let css_path = root.write("config/base.css", "broken-one");
    let cache_path = root.path().join("cache.json");
    let invocations = Cell::new(0usize);

    validate_with_counter(
        &invocations,
        std::slice::from_ref(&css_path),
        root.path(),
        &cache_path,
    )
    .expect("first parse");

    pause_for_metadata_tick();
    fs::write(&css_path, "broken-two").expect("rewrite file");

    let second = validate_with_counter(&invocations, &[css_path], root.path(), &cache_path)
        .expect("second parse");

    assert_eq!(invocations.get(), 2);
    assert!(second.diagnostics[0].message.contains("broken-two"));
}

#[test]
fn deleting_and_recreating_a_file_misses_the_cache() {
    let root = TempDirGuard::new("delete-recreate");
    let css_path = root.write("config/base.css", "broken-one");
    let cache_path = root.path().join("cache.json");
    let invocations = Cell::new(0usize);

    validate_with_counter(
        &invocations,
        std::slice::from_ref(&css_path),
        root.path(),
        &cache_path,
    )
    .expect("first parse");

    pause_for_metadata_tick();
    fs::remove_file(&css_path).expect("remove file");
    fs::write(&css_path, "broken-two").expect("recreate file");

    let second = validate_with_counter(&invocations, &[css_path], root.path(), &cache_path)
        .expect("second parse");

    assert_eq!(invocations.get(), 2);
    assert!(second.diagnostics[0].message.contains("broken-two"));
}

#[test]
fn replacing_a_path_with_a_new_inode_misses_the_cache() {
    let root = TempDirGuard::new("replace-path");
    let css_path = root.write("config/base.css", "broken-aa");
    let cache_path = root.path().join("cache.json");
    let invocations = Cell::new(0usize);

    validate_with_counter(
        &invocations,
        std::slice::from_ref(&css_path),
        root.path(),
        &cache_path,
    )
    .expect("first parse");

    pause_for_metadata_tick();
    let old_path = root.path().join("config/base.old.css");
    fs::rename(&css_path, &old_path).expect("move old file");
    fs::write(&css_path, "broken-bb").expect("write replacement file");

    let second = validate_with_counter(&invocations, &[css_path], root.path(), &cache_path)
        .expect("second parse");

    assert_eq!(invocations.get(), 2);
    assert!(second.diagnostics[0].message.contains("broken-bb"));
}

#[cfg(unix)]
#[test]
fn symlink_target_changes_miss_the_cache() {
    use std::os::unix::fs::symlink;

    let root = TempDirGuard::new("symlink-retarget");
    let _target_a = root.write("config/shared/base-a.css", "broken-one");
    let _target_b = root.write("config/shared/base-b.css", "broken-two");
    let symlink_path = root.path().join("config/base.css");
    symlink("shared/base-a.css", &symlink_path).expect("create symlink");
    let cache_path = root.path().join("cache.json");
    let invocations = Cell::new(0usize);

    validate_with_counter(
        &invocations,
        std::slice::from_ref(&symlink_path),
        root.path(),
        &cache_path,
    )
    .expect("first parse");

    pause_for_metadata_tick();
    fs::remove_file(&symlink_path).expect("remove symlink");
    symlink("shared/base-b.css", &symlink_path).expect("retarget symlink");

    let second = validate_with_counter(&invocations, &[symlink_path], root.path(), &cache_path)
        .expect("second parse");

    assert_eq!(invocations.get(), 2);
    assert!(second.diagnostics[0].message.contains("broken-two"));
}

#[test]
fn renaming_the_real_path_creates_a_new_cache_key() {
    let root = TempDirGuard::new("path-change");
    let old_path = root.write("config/base-a.css", "broken-one");
    let new_path = root.path().join("config/base-b.css");
    let cache_path = root.path().join("cache.json");
    let invocations = Cell::new(0usize);

    validate_with_counter(
        &invocations,
        std::slice::from_ref(&old_path),
        root.path(),
        &cache_path,
    )
    .expect("first parse");

    pause_for_metadata_tick();
    fs::rename(&old_path, &new_path).expect("rename file");

    let second = validate_with_counter(&invocations, &[new_path], root.path(), &cache_path)
        .expect("second parse");

    assert_eq!(invocations.get(), 2);
    assert!(second.diagnostics[0].message.contains("broken-one"));
}

#[test]
fn mixed_runs_only_reparse_the_changed_file() {
    let root = TempDirGuard::new("mixed-run");
    let first_path = root.write("config/base.css", "broken-one");
    let second_path = root.write("config/panel.css", "broken-two");
    let cache_path = root.path().join("cache.json");
    let invocations = Cell::new(0usize);

    validate_with_counter(
        &invocations,
        &[first_path.clone(), second_path.clone()],
        root.path(),
        &cache_path,
    )
    .expect("first parse");

    pause_for_metadata_tick();
    fs::write(&second_path, "broken-three").expect("rewrite second file");

    let second = validate_with_counter(
        &invocations,
        &[first_path, second_path],
        root.path(),
        &cache_path,
    )
    .expect("second parse");

    assert_eq!(invocations.get(), 3);
    assert_eq!(second.error_count, 2);
    assert!(second
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("broken-one")));
    assert!(second
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("broken-three")));
}

#[cfg(unix)]
#[test]
fn same_size_same_inode_same_mtime_still_misses_when_contents_change() {
    use std::os::unix::fs::MetadataExt;

    let root = TempDirGuard::new("coarse-mtime-edge");
    let css_path = root.write("config/base.css", "aaaaa");
    let cache_path = root.path().join("cache.json");
    let invocations = Cell::new(0usize);

    validate_with_counter(
        &invocations,
        std::slice::from_ref(&css_path),
        root.path(),
        &cache_path,
    )
    .expect("first parse");

    let before = fs::metadata(&css_path).expect("metadata before rewrite");
    let original_mtime = before.modified().expect("mtime before rewrite");

    fs::write(&css_path, "bbbbb").expect("rewrite same-sized file");
    super::helpers::set_file_mtime(&css_path, original_mtime);

    let after = fs::metadata(&css_path).expect("metadata after rewrite");
    assert_eq!(before.len(), after.len());
    assert_eq!(before.ino(), after.ino());
    assert_eq!(before.dev(), after.dev());
    assert_eq!(
        after.modified().expect("mtime after rewrite"),
        original_mtime
    );

    let second = validate_with_counter(&invocations, &[css_path], root.path(), &cache_path)
        .expect("second parse");

    assert_eq!(invocations.get(), 2);
    assert!(second.diagnostics[0].message.contains("bbbbb"));
}
