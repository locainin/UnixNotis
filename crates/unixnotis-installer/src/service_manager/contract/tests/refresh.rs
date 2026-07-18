use std::path::PathBuf;

use crate::service_manager::S6DatabaseRefresh;

#[test]
fn s6_refresh_paths_share_the_configured_data_root() {
    let refresh =
        S6DatabaseRefresh::new(PathBuf::from("/tmp/s6-data"), PathBuf::from("/tmp/s6-live"));

    assert_eq!(refresh.source_root(), PathBuf::from("/tmp/s6-data/sv"));
    assert_eq!(refresh.rc_root(), PathBuf::from("/tmp/s6-data/rc"));
    assert_eq!(
        refresh.compiled_link(),
        PathBuf::from("/tmp/s6-data/rc/compiled")
    );
    assert_eq!(refresh.live_root(), PathBuf::from("/tmp/s6-live"));
}
