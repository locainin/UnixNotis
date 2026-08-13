//! Trusted portal attribution regressions

use super::super::*;

#[test]
fn unmediated_flatpak_process_cannot_become_portal_associated() {
    let flatpak_identity = identity(21, 210, 0);
    let mut record = system_record(
        "org.example.FlatpakApp",
        "Flatpak App",
        "/usr/bin/flatpak",
        flatpak_identity,
    );
    record.association_eligible = false;
    record.system_association = false;
    let index = DesktopIdentityIndex::from_records(vec![record], Vec::new());

    let resolution = resolve_with_evidence(
        AppClaim {
            reported_name: "Flatpak App",
            desktop_entry: Some("org.example.FlatpakApp"),
        },
        &sender("/usr/bin/flatpak", flatpak_identity),
        &index,
    );

    assert_ne!(resolution.attribution.status, AttributionStatus::Verified);
    assert_eq!(resolution.inline_reply_policy, InlineReplyPolicy::Deny);
}

#[test]
fn an_empty_app_name_does_not_turn_an_untrusted_relay_into_a_portal() {
    let flatpak_identity = identity(24, 240, 0);
    let relay_identity = identity(25, 250, 0);
    let mut record = system_record(
        "org.example.FlatpakApp",
        "Flatpak App",
        "/usr/bin/flatpak",
        flatpak_identity,
    );
    record.association_eligible = false;
    record.system_association = false;
    let index = DesktopIdentityIndex::from_records(vec![record], Vec::new());

    let resolution = resolve_with_evidence(
        AppClaim {
            reported_name: "",
            desktop_entry: Some("org.example.FlatpakApp"),
        },
        &sender("/usr/lib/untrusted-relay", relay_identity),
        &index,
    );

    assert_ne!(resolution.attribution.status, AttributionStatus::Verified);
    assert_eq!(resolution.inline_reply_policy, InlineReplyPolicy::Deny);
}

#[test]
fn portal_mediated_flatpak_uses_broker_associated_desktop_identity() {
    let flatpak_identity = identity(22, 220, 0);
    let (portal_path, portal_identity) = installed_system_executable();
    let mut record = system_record(
        "org.example.FlatpakApp",
        "Flatpak App",
        "/usr/bin/flatpak",
        flatpak_identity,
    );
    record.association_eligible = false;
    record.system_association = false;
    let index = DesktopIdentityIndex::from_records(vec![record], Vec::new())
        .with_trusted_portal(PathBuf::from(&portal_path), portal_identity);

    let resolution = resolve_with_evidence(
        AppClaim {
            // The GTK portal backend forwards an empty app name and desktop-entry hint
            reported_name: "",
            desktop_entry: Some("org.example.FlatpakApp"),
        },
        &sender(&portal_path, portal_identity),
        &index,
    );

    assert_eq!(resolution.attribution.status, AttributionStatus::Recognized);
    assert_eq!(
        resolution.attribution.assurance,
        unixnotis_core::IdentityAssurance::PortalAssociated
    );
    assert_eq!(resolution.attribution.display_name, "Flatpak App");
    assert_eq!(resolution.inline_reply_policy, InlineReplyPolicy::Deny);
    assert_eq!(
        resolution.attribution.default_activation_policy(),
        unixnotis_core::ApplicationActionPolicy::Confirm
    );
    assert_eq!(
        resolution.attribution.action_button_policy(),
        unixnotis_core::ApplicationActionPolicy::Confirm
    );
}

#[test]
fn trusted_portal_accepts_a_matching_nonempty_application_name() {
    let flatpak_identity = identity(26, 260, 0);
    let (portal_path, portal_identity) = installed_system_executable();
    let mut record = system_record(
        "org.example.FlatpakApp",
        "Flatpak App",
        "/usr/bin/flatpak",
        flatpak_identity,
    );
    record.association_eligible = false;
    record.system_association = false;
    let index = DesktopIdentityIndex::from_records(vec![record], Vec::new())
        .with_trusted_portal(PathBuf::from(&portal_path), portal_identity);

    let resolution = resolve_with_evidence(
        AppClaim {
            reported_name: "Flatpak App",
            desktop_entry: Some("org.example.FlatpakApp"),
        },
        &sender(&portal_path, portal_identity),
        &index,
    );

    assert_eq!(resolution.attribution.status, AttributionStatus::Recognized);
    assert_eq!(resolution.attribution.display_name, "Flatpak App");
    assert_eq!(resolution.inline_reply_policy, InlineReplyPolicy::Deny);
}

#[test]
fn trusted_portal_reports_a_name_that_contradicts_its_verified_application_id() {
    let flatpak_identity = identity(27, 270, 0);
    let (portal_path, portal_identity) = installed_system_executable();
    let mut record = system_record(
        "org.example.FlatpakApp",
        "Flatpak App",
        "/usr/bin/flatpak",
        flatpak_identity,
    );
    record.association_eligible = false;
    record.system_association = false;
    let index = DesktopIdentityIndex::from_records(vec![record], Vec::new())
        .with_trusted_portal(PathBuf::from(&portal_path), portal_identity);

    let resolution = resolve_with_evidence(
        AppClaim {
            reported_name: "Different App",
            desktop_entry: Some("org.example.FlatpakApp"),
        },
        &sender(&portal_path, portal_identity),
        &index,
    );

    assert_eq!(resolution.attribution.status, AttributionStatus::Conflict);
    assert_eq!(resolution.inline_reply_policy, InlineReplyPolicy::Deny);
    assert_eq!(resolution.diagnostics.record_trust, RecordTrust::Portal);
}

#[test]
fn trusted_portal_rejects_a_stale_indexed_inode() {
    let (portal_path, live_identity) = installed_system_executable();
    let stale_identity = FileIdentity {
        inode: live_identity.inode.saturating_add(1),
        ..live_identity
    };
    let index = DesktopIdentityIndex::from_records(Vec::new(), Vec::new())
        .with_trusted_portal(PathBuf::from(&portal_path), stale_identity);

    assert!(index
        .trusted_portal_path(live_identity, std::path::Path::new(&portal_path))
        .is_none());
}

#[test]
fn trusted_portal_rejects_a_live_path_outside_protected_roots() {
    let (portal_path, portal_identity) = installed_system_executable();
    let index = DesktopIdentityIndex::from_records(Vec::new(), Vec::new())
        .with_trusted_portal(PathBuf::from(&portal_path), portal_identity);

    assert!(index
        .trusted_portal_path(
            portal_identity,
            std::path::Path::new("/tmp/xdg-desktop-portal")
        )
        .is_none());
}
