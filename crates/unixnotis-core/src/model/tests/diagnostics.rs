use zbus::zvariant::{serialized::Context, to_bytes, LE};

use super::{
    AttributionDiagnostics, CommandLineQualityView, LaunchAuthorityView, LaunchVerificationView,
    RecordTrust,
};

#[test]
fn attribution_diagnostics_round_trip_every_evidence_dimension() {
    let diagnostics = AttributionDiagnostics {
        claimed_name: "Example".to_string(),
        claimed_desktop_entry: "org.example.App".to_string(),
        sender_executable: "/opt/example/app".to_string(),
        matched_desktop_id: "org.example.App".to_string(),
        record_trust: RecordTrust::System,
        launch_authority: LaunchAuthorityView::ProtectedPayload,
        command_line_quality: CommandLineQualityView::RewrittenProcessTitle,
        verification: LaunchVerificationView::InsufficientEvidence,
        reason: "unstructured command-line evidence".to_string(),
    };

    let encoded =
        to_bytes(Context::new_dbus(LE, 0), &diagnostics).expect("serialize attribution evidence");
    let decoded = encoded
        .deserialize::<AttributionDiagnostics>()
        .expect("deserialize attribution evidence")
        .0;

    assert_eq!(decoded, diagnostics);
}

#[test]
fn diagnostic_wire_enums_reject_unknown_values() {
    let encoded =
        to_bytes(Context::new_dbus(LE, 0), &u8::MAX).expect("serialize unknown evidence value");

    assert!(encoded.deserialize::<RecordTrust>().is_err());
    assert!(encoded.deserialize::<LaunchAuthorityView>().is_err());
    assert!(encoded.deserialize::<CommandLineQualityView>().is_err());
    assert!(encoded.deserialize::<LaunchVerificationView>().is_err());
}
