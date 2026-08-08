use super::{quota_principal, QuotaPrincipal, SenderMetadata};

#[test]
fn quota_principal_requires_and_preserves_the_complete_process_lifetime() {
    let complete = SenderMetadata {
        sender_uid: Some(1_000),
        sender_pid: Some(42),
        sender_start_time: Some(77),
        ..SenderMetadata::default()
    };

    assert_eq!(
        quota_principal(&complete),
        Some(QuotaPrincipal::new(1_000, 42, 77))
    );

    for incomplete in [
        SenderMetadata {
            sender_uid: None,
            ..complete.clone()
        },
        SenderMetadata {
            sender_pid: None,
            ..complete.clone()
        },
        SenderMetadata {
            sender_start_time: None,
            ..complete
        },
    ] {
        assert_eq!(quota_principal(&incomplete), None);
    }
}
