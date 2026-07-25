use super::NotificationAttribution;

#[test]
fn matching_sender_identity_keeps_reported_brand_and_marks_it_verified() {
    let (display, attribution) =
        NotificationAttribution::resolve("UnixNotis Center", Some("/usr/bin/unixnotis-center"));

    assert_eq!(display, "UnixNotis Center");
    assert!(attribution.verified);
    assert!(attribution.reported_name.is_empty());
    assert_eq!(attribution.badge_icon, "unixnotis-center");
    assert_eq!(attribution.display_label(&display), "UnixNotis Center");
}

#[test]
fn mismatched_sender_leads_with_executable_and_keeps_claim_secondary() {
    let (display, attribution) =
        NotificationAttribution::resolve("Password Manager", Some("/usr/bin/unknown-client"));

    assert_eq!(display, "unknown-client");
    assert!(!attribution.verified);
    assert_eq!(attribution.reported_name, "Password Manager");
    assert_eq!(attribution.badge_icon, "unknown-client");
    assert_eq!(
        attribution.display_label(&display),
        "unknown-client · unverified claim: Password Manager"
    );
}

#[test]
fn unresolved_sender_keeps_claim_but_uses_warning_badge() {
    let (display, attribution) = NotificationAttribution::resolve("Calendar", None);

    assert_eq!(display, "Calendar");
    assert!(!attribution.verified);
    assert!(attribution.reported_name.is_empty());
    assert_eq!(attribution.badge_icon, "dialog-warning-symbolic");
    assert_eq!(attribution.display_label(&display), "Calendar · unverified");
}

#[test]
fn partial_executable_name_match_does_not_verify_a_brand_claim() {
    let (display, attribution) =
        NotificationAttribution::resolve("Discord", Some("/opt/discord/DiscordCanaryDiscord"));

    assert_eq!(display, "DiscordCanaryDiscord");
    assert!(!attribution.verified);
    assert_eq!(attribution.reported_name, "Discord");
}
