use super::*;
use crate::preset::archive::BundleFile;
use crate::preset::import::transaction::apply::{
    apply_import_plan, finalize_import_transaction, rollback_import_transaction,
};
use crate::preset::import::transaction::plan::build_import_plan;

#[test]
fn applied_import_can_restore_the_exact_previous_file() {
    let root = TempDirGuard::new("apply-rollback");
    root.write("config.toml", "before");
    let plan = build_import_plan(
        &root.path,
        vec![BundleFile {
            relative_path: PathBuf::from("config.toml"),
            contents: b"after".to_vec(),
            mode: 0o644,
        }],
        &[],
    )
    .expect("build import plan");

    let transaction = apply_import_plan(&root.path, &plan).expect("apply import");
    assert_eq!(
        fs::read_to_string(root.path.join("config.toml")).expect("read applied file"),
        "after"
    );

    rollback_import_transaction(transaction).expect("rollback import");
    assert_eq!(
        fs::read_to_string(root.path.join("config.toml")).expect("read restored file"),
        "before"
    );
}

#[test]
fn apply_captures_a_file_that_appears_after_planning() {
    let root = TempDirGuard::new("apply-late-existing-file");
    root.write("config.toml", "[panel]\nwidth = 320\n");
    let plan = build_import_plan(
        &root.path,
        vec![BundleFile {
            relative_path: PathBuf::from("theme/base.css"),
            contents: b"imported".to_vec(),
            mode: 0o644,
        }],
        &[],
    )
    .expect("build import plan");
    root.write("theme/base.css", "appeared after planning");

    let transaction = apply_import_plan(&root.path, &plan).expect("apply import");
    assert_eq!(
        fs::read_to_string(root.path.join("theme/base.css")).expect("read imported file"),
        "imported"
    );

    rollback_import_transaction(transaction).expect("rollback import");
    assert_eq!(
        fs::read_to_string(root.path.join("theme/base.css")).expect("read restored late file"),
        "appeared after planning"
    );
}

#[test]
fn transaction_rejects_a_replaced_live_root_before_finalize() {
    let root = TempDirGuard::new("finalize-root-replacement");
    root.write("config.toml", "before");
    let plan = build_import_plan(
        &root.path,
        vec![BundleFile {
            relative_path: PathBuf::from("config.toml"),
            contents: b"after".to_vec(),
            mode: 0o644,
        }],
        &[],
    )
    .expect("build import plan");
    let transaction = apply_import_plan(&root.path, &plan).expect("apply import");
    let moved = root.path.with_extension("moved");
    let _ = fs::remove_dir_all(&moved);
    fs::rename(&root.path, &moved).expect("move imported config root");
    fs::create_dir(&root.path).expect("create replacement config root");

    finalize_import_transaction(transaction)
        .expect_err("finalize must reject a replacement config root");

    assert_eq!(
        fs::read_to_string(moved.join("config.toml")).expect("read rolled-back old root"),
        "before"
    );
    fs::remove_dir_all(&root.path).expect("remove replacement root");
    fs::rename(&moved, &root.path).expect("restore imported config root");
}

#[test]
fn root_drift_check_rolls_back_files_through_the_pinned_descriptor() {
    let root = TempDirGuard::new("apply-root-drift-rollback");
    root.write("config.toml", "before");
    let plan = build_import_plan(
        &root.path,
        vec![BundleFile {
            relative_path: PathBuf::from("config.toml"),
            contents: b"after".to_vec(),
            mode: 0o644,
        }],
        &[],
    )
    .expect("build import plan");
    let transaction = apply_import_plan(&root.path, &plan).expect("apply import");
    let moved = root.path.with_extension("moved");
    let _ = fs::remove_dir_all(&moved);
    fs::rename(&root.path, &moved).expect("move imported config root");
    fs::create_dir(&root.path).expect("create replacement config root");

    transaction
        .ensure_live_root_or_rollback()
        .expect_err("root drift must stop and roll back the import");

    assert_eq!(
        fs::read_to_string(moved.join("config.toml")).expect("read descriptor-root config"),
        "before"
    );
    fs::remove_dir_all(&root.path).expect("remove replacement root");
    fs::rename(&moved, &root.path).expect("restore rolled-back config root");
}
