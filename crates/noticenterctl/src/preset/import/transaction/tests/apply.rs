use super::*;
use crate::preset::archive::BundleFile;
use crate::preset::import::apply::{apply_import_plan, rollback_import_transaction};
use crate::preset::import::plan::build_import_plan;

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
