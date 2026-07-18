use super::*;
use crate::preset::archive::BundleFile;
use crate::preset::import::transaction::plan::build_import_plan;

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

#[test]
fn import_plan_rejects_payload_file_ancestor_conflicts() {
    let root = TempDirGuard::new("plan-file-ancestor-conflict");
    let files = vec![
        BundleFile {
            relative_path: PathBuf::from("config.toml/child"),
            contents: b"child".to_vec(),
            mode: 0o644,
        },
        BundleFile {
            relative_path: PathBuf::from("config.toml"),
            contents: Vec::new(),
            mode: 0o644,
        },
    ];

    let error = build_import_plan(&root.path, files, &[])
        .expect_err("a payload file cannot also be another payload file's parent");

    assert!(error
        .to_string()
        .contains("file paths that cannot both exist"));
}

#[test]
fn import_plan_allows_sibling_paths_with_shared_text_prefixes() {
    let root = TempDirGuard::new("plan-sibling-prefixes");
    let files = vec![
        BundleFile {
            relative_path: PathBuf::from("assets/theme"),
            contents: b"first".to_vec(),
            mode: 0o644,
        },
        BundleFile {
            relative_path: PathBuf::from("assets/theme-extra"),
            contents: b"second".to_vec(),
            mode: 0o644,
        },
    ];

    let plan = build_import_plan(&root.path, files, &[]).expect("siblings should remain valid");

    assert_eq!(plan.items.len(), 2);
}
