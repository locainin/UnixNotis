use std::fs;

pub(super) fn temp_config_dir(label: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("unixnotis-reset-{label}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("create reset fixture");
    path
}
