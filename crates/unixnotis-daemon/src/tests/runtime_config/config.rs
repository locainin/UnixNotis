use crate::test_support::TempRoot;

use super::{load_config, test_support};

#[test]
fn load_config_rejects_missing_custom_config_path() {
    let root = TempRoot::new("runtime-config-missing");
    let mut args = test_support::default_args();
    args.config = Some(root.join("missing.toml"));

    // A caller-specified path is intentional, so missing files should fail
    assert!(load_config(&args).is_err());
}
