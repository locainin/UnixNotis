use std::path::PathBuf;

use super::{IconDecodeRequest, IconResolution};
use crate::ui::icons::cache::IconKey;

#[test]
fn asynchronous_resolution_preserves_decode_geometry_and_path() {
    let key = IconKey::Path {
        path: PathBuf::from("icon.png"),
        size: 20,
        scale: 2,
    };
    let resolution = IconResolution::Async {
        request: IconDecodeRequest {
            key,
            path: PathBuf::from("icon.png"),
            size: 20,
            scale: 2,
        },
    };

    let IconResolution::Async { request } = resolution else {
        panic!("expected asynchronous icon resolution");
    };
    assert_eq!(request.path, PathBuf::from("icon.png"));
    assert_eq!(request.size, 20);
    assert_eq!(request.scale, 2);
}
