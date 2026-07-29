use std::fs;
use std::os::unix::fs::symlink;

use super::super::launcher::launcher_binding_is_current;
use super::super::model::{LaunchVerification, VerifiedLaunch};
use super::super::scan::{ScanBudget, ScanLimits};
use super::super::{verify_record_launch, DesktopIdentityIndex};
use crate::daemon::notifications::identity::sender::{CommandLineEvidence, CommandLineQuality};
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

#[test]
fn exhausted_user_budget_does_not_block_system_desktop_records() {
    let root = TempRoot::new("desktop-separate-budgets");
    let user_root = root.join("user");
    let system_root = root.join("system");
    fs::create_dir_all(&user_root).expect("create user application directory");
    fs::create_dir_all(&system_root).expect("create system application directory");
    for name in ["one.desktop", "two.desktop"] {
        fs::write(
            user_root.join(name),
            "[Desktop Entry]\nType=Application\nName=User App\nExec=/usr/bin/true\n",
        )
        .expect("user desktop fixture");
    }
    fs::write(
        system_root.join("system.desktop"),
        "[Desktop Entry]\nType=Application\nName=System App\nExec=/usr/bin/true\n",
    )
    .expect("system desktop fixture");
    let limits = ScanLimits {
        records: 1,
        ..ScanLimits::default()
    };

    let snapshot = DesktopIdentityIndex::build_with_roots(
        vec![(user_root, false), (system_root, true)],
        &limits,
    );

    assert!(snapshot
        .index
        .records
        .iter()
        .any(|record| record.system_origin && record.display_name == "System App"));
    assert_eq!(snapshot.watched_directories.len(), 2);
}

#[test]
fn local_arch_package_launcher_reaches_its_runtime_target() {
    let desktop = std::path::Path::new("/usr/share/applications/signal.desktop");
    if !desktop.exists() {
        return;
    }

    let snapshot = DesktopIdentityIndex::build_snapshot();
    let record = snapshot
        .index
        .records_for_id("signal")
        .into_iter()
        .find(|record| record.system_origin)
        .expect("installed package desktop record");

    assert!(record.system_association);
    assert_eq!(
        record.declared_executable_path.as_deref(),
        Some(std::path::Path::new("/usr/bin/signal-desktop"))
    );
    assert_eq!(
        record.runtime_executable_path.as_deref(),
        Some(std::path::Path::new(
            "/usr/lib/signal-desktop/signal-desktop"
        ))
    );
    let binding = record
        .launch_spec
        .as_ref()
        .and_then(|spec| spec.package_launcher.as_ref())
        .expect("installed package launcher binding");
    assert!(launcher_binding_is_current(binding));
    let mut stale_digest = binding.clone();
    stale_digest.launcher_digest[0] ^= 1;
    assert!(!launcher_binding_is_current(&stale_digest));
    let mut changed_target = binding.clone();
    changed_target.target_path = "/usr/bin/true".into();
    assert!(!launcher_binding_is_current(&changed_target));
    let runtime_identity = record
        .runtime_executable_identity
        .expect("installed runtime identity");
    let command_line = CommandLineEvidence {
        argv: [
            "/usr/lib/signal-desktop/signal-desktop",
            "--password-store=kwallet6",
            "--ozone-platform=x11",
            "--use-tray-icon",
            "--",
        ]
        .into_iter()
        .map(|argument| argument.as_bytes().to_vec())
        .collect(),
        quality: CommandLineQuality::Structured,
    };

    assert_eq!(
        verify_record_launch(record, &snapshot.index, runtime_identity, &command_line),
        LaunchVerification::Verified(VerifiedLaunch::PackageLauncherTarget)
    );
}
