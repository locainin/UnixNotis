use std::fs;
use std::os::unix::fs::symlink;

use super::super::scan::{ScanBudget, ScanLimits};
use super::super::DesktopIdentityIndex;
use crate::test_support::TempRoot;

#[test]
fn scan_rejects_oversized_desktop_files_before_parsing() {
    let root = TempRoot::new("desktop-size-budget");
    let path = root.join("large.desktop");
    fs::write(&path, "x".repeat(65)).expect("oversized desktop fixture");
    let limits = ScanLimits {
        file_bytes: 64,
        ..ScanLimits::default()
    };
    let mut budget = ScanBudget::default();
    let mut index = DesktopIdentityIndex::default();

    index.scan_root(root.path(), false, &limits, &mut budget);

    assert!(index.records.is_empty());
    assert_eq!(budget.skipped_files, 1);
}

#[test]
fn scan_accepts_a_regular_desktop_file_at_the_exact_size_limit() {
    let root = TempRoot::new("desktop-exact-size-budget");
    let contents = "[Desktop Entry]\nType=Application\nName=App\nExec=/usr/bin/true\n";
    fs::write(root.join("exact.desktop"), contents).expect("exact-size desktop fixture");
    let limits = ScanLimits {
        file_bytes: u64::try_from(contents.len()).expect("fixture length fits u64"),
        ..ScanLimits::default()
    };
    let mut budget = ScanBudget::default();
    let mut index = DesktopIdentityIndex::default();

    index.scan_root(root.path(), false, &limits, &mut budget);

    assert_eq!(index.records.len(), 1);
    assert_eq!(budget.skipped_files, 0);
}

#[test]
fn scan_never_follows_a_desktop_file_symlink() {
    let root = TempRoot::new("desktop-symlink");
    let target = root.join("target.txt");
    fs::write(
        &target,
        "[Desktop Entry]\nType=Application\nName=Linked\nExec=/usr/bin/true\n",
    )
    .expect("symlink target fixture");
    symlink(&target, root.join("linked.desktop")).expect("desktop symlink fixture");
    let mut budget = ScanBudget::default();
    let mut index = DesktopIdentityIndex::default();

    index.scan_root(root.path(), false, &ScanLimits::default(), &mut budget);

    assert!(index.records.is_empty());
}

#[test]
fn scan_stops_when_the_global_entry_budget_is_exhausted() {
    let root = TempRoot::new("desktop-entry-budget");
    for name in ["one.desktop", "two.desktop"] {
        fs::write(
            root.join(name),
            "[Desktop Entry]\nType=Application\nName=App\nExec=/usr/bin/true\n",
        )
        .expect("desktop fixture");
    }
    let limits = ScanLimits {
        entries: 1,
        ..ScanLimits::default()
    };
    let mut budget = ScanBudget::default();
    let mut index = DesktopIdentityIndex::default();

    index.scan_root(root.path(), false, &limits, &mut budget);

    assert_eq!(budget.entries, 1);
    assert_eq!(budget.stopped_by, Some("entry budget"));
    assert!(index.records.len() <= 1);
}

#[test]
fn scan_stops_before_crossing_the_directory_depth_budget() {
    let root = TempRoot::new("desktop-depth-budget");
    fs::create_dir_all(root.join("one/two")).expect("nested application directories");
    let limits = ScanLimits {
        depth: 1,
        ..ScanLimits::default()
    };
    let mut budget = ScanBudget::default();
    let mut index = DesktopIdentityIndex::default();

    index.scan_root(root.path(), false, &limits, &mut budget);

    assert_eq!(budget.stopped_by, Some("directory depth"));
}

#[test]
fn scan_stops_when_the_global_directory_budget_is_exhausted() {
    let root = TempRoot::new("desktop-directory-budget");
    fs::create_dir_all(root.join("one")).expect("first application directory");
    fs::create_dir_all(root.join("two")).expect("second application directory");
    let limits = ScanLimits {
        directories: 1,
        ..ScanLimits::default()
    };
    let mut budget = ScanBudget::default();
    let mut index = DesktopIdentityIndex::default();

    index.scan_root(root.path(), false, &limits, &mut budget);

    assert_eq!(budget.directories, 1);
    assert_eq!(budget.stopped_by, Some("directory budget"));
}

#[test]
fn scan_stops_when_the_global_record_budget_is_exhausted() {
    let root = TempRoot::new("desktop-record-budget");
    for name in ["one.desktop", "two.desktop"] {
        fs::write(
            root.join(name),
            "[Desktop Entry]\nType=Application\nName=App\nExec=/usr/bin/true\n",
        )
        .expect("desktop fixture");
    }
    let limits = ScanLimits {
        records: 1,
        ..ScanLimits::default()
    };
    let mut budget = ScanBudget::default();
    let mut index = DesktopIdentityIndex::default();

    index.scan_root(root.path(), false, &limits, &mut budget);

    assert_eq!(index.records.len(), 1);
    assert_eq!(budget.stopped_by, Some("record budget"));
}
