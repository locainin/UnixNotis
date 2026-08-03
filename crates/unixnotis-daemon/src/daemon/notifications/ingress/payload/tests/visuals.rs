use super::super::visuals::{
    downsample_avatar, local_avatar_path, valid_percent_escapes, MAX_DECODE_DIMENSION,
};
use super::*;
#[test]
fn associated_sender_role_accepts_inline_reply_and_message_categories() {
    let attribution = unixnotis_core::NotificationAttribution::associated(
        "Messages",
        "Messages",
        "org.example.Messages",
        "messages",
        IdentityAssurance::SystemAssociated,
        InteractionPolicies::NATIVE_COMPATIBILITY,
        unixnotis_core::AttributionReason::ExactUserExecutable,
        "associated executable",
        "recognized:system-app:org.example.Messages:sender".to_string(),
    );
    let index = super::super::super::super::identity::DesktopIdentityIndex::default();

    assert_eq!(
        sender_visual_role(
            &attribution,
            &index,
            &HashMap::new(),
            &["inline-reply".to_string(), "Reply".to_string()],
            "",
        ),
        SenderVisualRole::ConversationAvatar
    );

    let mut hints = HashMap::new();
    hints.insert(
        "category".to_string(),
        string_to_owned_value("im.received").expect("category value"),
    );
    assert_eq!(
        sender_visual_role(&attribution, &index, &hints, &[], ""),
        SenderVisualRole::ConversationAvatar
    );

    let mut exact = HashMap::new();
    exact.insert(
        "category".to_string(),
        string_to_owned_value("im").expect("exact category value"),
    );
    assert_eq!(
        sender_visual_role(&attribution, &index, &exact, &[], ""),
        SenderVisualRole::ConversationAvatar
    );

    let mut unrelated = HashMap::new();
    unrelated.insert(
        "category".to_string(),
        string_to_owned_value("other").expect("unrelated category value"),
    );
    assert_eq!(
        sender_visual_role(&attribution, &index, &unrelated, &[], ""),
        SenderVisualRole::None
    );
    assert_eq!(
        sender_visual_role(&attribution, &index, &HashMap::new(), &[], ""),
        SenderVisualRole::None
    );
}

#[test]
fn associated_noncommunication_path_is_a_small_application_visual() {
    let attribution = unixnotis_core::NotificationAttribution::associated(
        "Example player",
        "Example player",
        "org.example.Player",
        "example-player",
        IdentityAssurance::SystemAssociated,
        InteractionPolicies::NATIVE_COMPATIBILITY,
        unixnotis_core::AttributionReason::ExactSystemExecutable,
        "associated executable",
        "associated:system-app:org.example.Player:sender".to_string(),
    );
    let role = sender_visual_role(
        &attribution,
        &super::super::super::super::identity::DesktopIdentityIndex::default(),
        &HashMap::new(),
        &[],
        "/tmp/application-icon.png",
    );

    assert_eq!(role, SenderVisualRole::ApplicationProvidedIcon);
}

#[test]
fn portal_association_cannot_start_host_avatar_materialization() {
    let attribution = unixnotis_core::NotificationAttribution::associated(
        "Portal app",
        "Portal app",
        "org.example.PortalApp",
        "portal-app",
        IdentityAssurance::PortalAssociated,
        InteractionPolicies::CONFIRM_ACTIONS,
        AttributionReason::PortalAppIdAssociation,
        "portal supplied app id",
        "recognized:portal:org.example.PortalApp".to_string(),
    );
    assert!(!may_materialize_application_icon(&attribution));
    assert_eq!(
        sender_visual_role(
            &attribution,
            &super::super::super::super::identity::DesktopIdentityIndex::default(),
            &HashMap::new(),
            &["inline-reply".to_string(), "Reply".to_string()],
            "",
        ),
        SenderVisualRole::None
    );
}

#[test]
fn associated_noncommunication_icon_is_retained_as_a_decorative_visual() {
    let icon = unixnotis_core::ImageData {
        width: 1,
        height: 1,
        rowstride: 4,
        has_alpha: true,
        bits_per_sample: 8,
        channels: 4,
        data: vec![1, 2, 3, 255],
    };
    let notification = build_notification(NotificationInput {
        app_name: "Example player".to_string(),
        app_icon: "example-player".to_string(),
        summary: "Track".to_string(),
        body: "Artist".to_string(),
        actions: Vec::new(),
        hints: HashMap::new(),
        image_data: None,
        sender_visual: Some(icon),
        sender_visual_role: SenderVisualRole::ApplicationProvidedIcon,
        sender: SenderMetadata::default(),
        attribution: unixnotis_core::NotificationAttribution::associated(
            "Example player",
            "Example player",
            "org.example.Player",
            "example-player",
            IdentityAssurance::SystemAssociated,
            InteractionPolicies::NATIVE_COMPATIBILITY,
            AttributionReason::ExactSystemExecutable,
            "protected local association",
            "associated:system-app:org.example.Player".to_string(),
        ),
        attribution_diagnostics: unixnotis_core::AttributionDiagnostics::default(),
        inline_reply_policy: unixnotis_core::InlineReplyPolicy::Deny,
        expire_timeout: 0,
    });

    assert_eq!(
        notification.image.sender_visual_role,
        unixnotis_core::NotificationVisualRole::ApplicationProvidedIcon
    );
    assert_eq!(notification.image.badge_icon, "example-player");
}

#[test]
fn decorative_visual_materialization_is_independent_of_click_authority() {
    let attribution = unixnotis_core::NotificationAttribution::associated(
        "Example",
        "Example",
        "org.example.App",
        "example",
        IdentityAssurance::SystemAssociated,
        InteractionPolicies::DENY,
        unixnotis_core::AttributionReason::ExactSystemExecutable,
        "associated executable",
        "associated:system-app:org.example.App:sender".to_string(),
    );

    assert!(may_materialize_application_icon(&attribution));
    assert!(attribution.may_materialize_content_image());
    assert_eq!(
        attribution.default_activation_policy(),
        ApplicationActionPolicy::Deny
    );
}

#[test]
fn large_avatar_is_downsampled_to_the_storage_bound() {
    let source = vec![255_u8; 256 * 128 * 4];
    let (width, height, data) = downsample_avatar(256, 128, source, 64).expect("downsample");
    assert_eq!((width, height), (64, 32));
    assert_eq!(data.len(), 64 * 32 * 4);
}

#[test]
fn avatar_downsampling_rejects_zero_dimensions_and_keeps_exact_size_images() {
    assert!(downsample_avatar(0, 1, Vec::new(), 64).is_none());
    assert!(downsample_avatar(1, 0, Vec::new(), 64).is_none());

    let source = vec![7_u8; 64 * 64 * 4];
    let source_ptr = source.as_ptr();
    let (width, height, data) = downsample_avatar(64, 64, source, 64).expect("exact bound");
    assert_eq!((width, height), (64, 64));
    assert_eq!(data.as_ptr(), source_ptr);
}

#[test]
fn avatar_downsampling_maps_horizontal_and_vertical_pixels_by_scale() {
    // Keep the source height unchanged after scaling so the early-return guard
    // must compare both dimensions rather than accepting one matching value
    let mut horizontal = vec![0_u8; 128 * 4];
    for x in 0..128 {
        horizontal[x * 4] = u8::try_from(x).expect("horizontal fixture value");
    }
    let (width, height, data) =
        downsample_avatar(128, 1, horizontal, 64).expect("horizontal downsample");
    assert_eq!((width, height), (64, 1));
    assert_eq!(data[4], 2);

    let mut vertical = vec![0_u8; 64 * 128 * 4];
    for y in 0..128 {
        vertical[y * 64 * 4] = u8::try_from(y).expect("vertical fixture value");
    }
    let (width, height, data) =
        downsample_avatar(64, 128, vertical, 64).expect("vertical downsample");
    assert_eq!((width, height), (32, 64));
    assert_eq!(data[32 * 4], 2);
}

#[cfg(target_os = "linux")]
#[test]
fn fifo_avatar_path_is_rejected_without_opening_a_blocking_reader() {
    let directory = std::env::temp_dir().join(format!(
        "unixnotis-avatar-fifo-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock")
            .as_nanos()
    ));
    std::fs::create_dir(&directory).expect("create temporary directory");
    let path = directory.join("avatar.fifo");
    let path_string = path.to_string_lossy().into_owned();
    let status = std::process::Command::new("mkfifo")
        .arg(&path)
        .status()
        .expect("mkfifo available");
    assert!(status.success());
    assert!(materialize_sender_visual(&path_string, 64).is_none());
    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_dir(directory);
}

#[test]
fn absolute_avatar_path_is_materialized_into_bounded_raster_data() {
    // This is a tiny 1x1 RGBA PNG used only to exercise the real decoder
    let png = [
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f,
        0x15, 0xc4, 0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0xf8,
        0xcf, 0xc0, 0xf0, 0x1f, 0x00, 0x05, 0x00, 0x01, 0xff, 0x89, 0x99, 0x3d, 0x1d, 0x00, 0x00,
        0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
    ];
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("unixnotis-avatar-{suffix}.png"));
    std::fs::write(&path, png).expect("write avatar fixture");

    let avatar = materialize_sender_visual(path.to_str().expect("utf8 fixture path"), 64);
    let _ = std::fs::remove_file(&path);

    let avatar = avatar.expect("valid avatar should decode");
    assert_eq!((avatar.width, avatar.height), (1, 1));
    assert_eq!(avatar.channels, 4);
    assert_eq!(avatar.data.len(), 4);
}

#[test]
fn avatar_size_limits_accept_the_boundary_and_reject_one_byte_over() {
    assert!(avatar_file_size_allowed(MAX_SENDER_VISUAL_BYTES));
    assert!(!avatar_file_size_allowed(MAX_SENDER_VISUAL_BYTES + 1));
    assert!(avatar_buffer_size_allowed(MAX_SENDER_VISUAL_BYTES as usize));
    assert!(!avatar_buffer_size_allowed(
        MAX_SENDER_VISUAL_BYTES as usize + 1
    ));
}

#[test]
fn sender_visual_file_policy_requires_a_regular_file_and_bounded_size() {
    assert!(sender_visual_file_allowed(true, MAX_SENDER_VISUAL_BYTES));
    assert!(!sender_visual_file_allowed(false, MAX_SENDER_VISUAL_BYTES));
    assert!(!sender_visual_file_allowed(
        true,
        MAX_SENDER_VISUAL_BYTES + 1
    ));
}

#[test]
fn sender_visual_decode_dimension_has_a_stable_upper_bound() {
    assert_eq!(bounded_decode_dimension(64), 64);
    assert_eq!(bounded_decode_dimension(512), 512);
    assert_eq!(
        bounded_decode_dimension(MAX_DECODE_DIMENSION),
        MAX_DECODE_DIMENSION
    );
    assert_eq!(bounded_decode_dimension(513), 512);
}

#[test]
fn relative_or_missing_avatar_path_is_rejected() {
    assert!(materialize_sender_visual("avatar.png", 64).is_none());
    assert!(materialize_sender_visual("/path/that/does/not/exist.png", 64).is_none());
}

#[test]
fn local_avatar_uri_decodes_local_file_paths() {
    assert_eq!(
        local_avatar_path("file:///tmp/avatar%20one.png")
            .expect("encoded local path")
            .to_string_lossy(),
        "/tmp/avatar one.png"
    );
    assert_eq!(
        local_avatar_path("file://localhost/tmp/avatar.png")
            .expect("localhost path")
            .to_string_lossy(),
        "/tmp/avatar.png"
    );
}

#[test]
fn local_avatar_uri_rejects_remote_or_ambiguous_paths() {
    for value in [
        "file://example.test/tmp/avatar.png",
        "file:///tmp/avatar.png?download=1",
        "file:///tmp/avatar.png#fragment",
        "file:///tmp/%00avatar.png",
        "file:///tmp/%ZZavatar.png",
    ] {
        assert!(
            local_avatar_path(value).is_none(),
            "unexpectedly accepted {value}"
        );
    }
}

#[test]
fn percent_escape_validation_requires_two_hex_digits() {
    assert!(valid_percent_escapes("%20"));
    assert!(valid_percent_escapes("file:///tmp/avatar%20one.png"));
    assert!(valid_percent_escapes("file:///tmp/avatar%2Fone.png"));
    assert!(!valid_percent_escapes("file:///tmp/avatar%2.png"));
    assert!(!valid_percent_escapes("file:///tmp/avatar%GG.png"));
    assert!(!valid_percent_escapes("file:///tmp/avatar%"));
}
