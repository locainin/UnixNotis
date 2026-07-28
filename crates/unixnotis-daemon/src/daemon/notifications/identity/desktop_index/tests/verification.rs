use super::{is_dynamic_or_option, is_protected_payload};
use crate::daemon::notifications::identity::desktop_index::model::{
    FieldCode, LaunchArgument, LiteralArgument,
};

#[test]
fn authority_helpers_distinguish_dynamic_values_options_and_payloads() {
    let dynamic = LaunchArgument::FieldCode(FieldCode::Files);
    let option = LaunchArgument::Literal(LiteralArgument {
        value: b"--fixed".to_vec(),
        file: None,
    });
    let payload = LaunchArgument::Literal(LiteralArgument {
        value: b"/usr/share/example/app.bundle".to_vec(),
        file: Some((
            "/usr/share/example/app.bundle".into(),
            crate::daemon::notifications::identity::FileIdentity {
                device: 1,
                inode: 2,
                uid: 0,
                mode: 0o100_755,
            },
        )),
    });

    assert!(is_dynamic_or_option(&dynamic));
    assert!(is_dynamic_or_option(&option));
    assert!(!is_dynamic_or_option(&payload));
    assert!(!is_protected_payload(&dynamic));
    assert!(is_protected_payload(&payload));
}
