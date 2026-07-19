use super::notification_capabilities;

#[test]
fn notification_capabilities_without_sound_keeps_static_contract() {
    let caps = notification_capabilities(false);

    assert_eq!(
        caps,
        [
            "actions",
            "inline-reply",
            "body",
            "body-markup",
            "icon-static"
        ]
    );
}

#[test]
fn notification_capabilities_adds_sound_only_when_backend_supports_it() {
    let caps = notification_capabilities(true);

    assert_eq!(
        caps,
        [
            "actions",
            "inline-reply",
            "body",
            "body-markup",
            "icon-static",
            "sound"
        ]
    );
}
