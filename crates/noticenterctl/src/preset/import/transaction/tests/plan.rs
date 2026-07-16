use super::*;
use crate::preset::archive::BundleFile;
use crate::preset::import::plan::build_import_plan;

#[test]
fn import_plan_counts_created_overwritten_and_excluded_files() {
    let root = TempDirGuard::new("plan-counts");
    root.write("existing.css", "old");
    let files = vec![
        BundleFile {
            relative_path: PathBuf::from("existing.css"),
            contents: b"new".to_vec(),
            mode: 0o644,
        },
        BundleFile {
            relative_path: PathBuf::from("new.css"),
            contents: b"new".to_vec(),
            mode: 0o644,
        },
        BundleFile {
            relative_path: PathBuf::from("ignored.css"),
            contents: b"ignored".to_vec(),
            mode: 0o644,
        },
    ];

    let plan = build_import_plan(&root.path, files, &[PathBuf::from("ignored.css")])
        .expect("build import plan");

    assert_eq!(plan.items.len(), 2);
    assert_eq!(plan.created, 1);
    assert_eq!(plan.overwritten, 1);
    assert_eq!(plan.excluded, 1);
}
